use acton_config::color::{ColorMode, OwoColorize, color_mode};
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::{Context, Result};
use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use globset::{Glob, GlobSetBuilder};
use path_absolutize::Absolutize;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::Point;
use walkdir::WalkDir;

pub fn fmt_cmd(paths: Vec<String>, check: bool) -> Result<()> {
    let config = ActonConfig::load()
        .map_err(|e| {
            eprintln!("  {} Failed to load Acton.toml: {e:#}", "⚠".yellow().bold());
        })
        .ok();
    let fmt_settings = config.as_ref().and_then(|c| c.fmt.as_ref());
    let project_root = configured_project_root();
    let current_dir = std::env::current_dir().context("Failed to determine current directory")?;

    let width = fmt_settings.and_then(|s| s.width).unwrap_or(100);
    let ignore_patterns = fmt_settings.and_then(|s| s.ignore.as_ref());
    let separate_import_groups = fmt_settings
        .and_then(|s| s.separate_import_groups)
        .unwrap_or(false);

    let mut ignore_builder = GlobSetBuilder::new();
    for p in [
        "**/node_modules/**",
        "**/.git/**",
        "**/target/**",
        "**/.acton/**",
        "**/.codex/**",
        "**/.claude/**",
    ] {
        ignore_builder.add(Glob::new(p)?);
    }
    if let Some(ignores) = ignore_patterns {
        for pattern in ignores {
            ignore_builder.add(Glob::new(pattern)?);
        }
    }
    let ignore_set = ignore_builder.build()?;

    let mut files_to_format = Vec::new();

    let has_explicit_paths = !paths.is_empty();
    let search_paths = if paths.is_empty() {
        vec![project_root.to_path_buf()]
    } else {
        paths.into_iter().map(PathBuf::from).collect()
    };

    for path in search_paths {
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "tolk") {
                files_to_format.push(path);
            } else if has_explicit_paths {
                anyhow::bail!(
                    "Cannot format {}: expected a .tolk file",
                    path.display().to_string().yellow()
                );
            }
        } else if path.is_dir() {
            let iter = WalkDir::new(&path)
                .into_iter()
                .filter_entry(|entry| {
                    if !entry.file_type().is_dir() {
                        return true;
                    }
                    let p = entry.path();
                    let relative = relative_to_project_root(p, project_root);
                    !ignore_set.is_match(p) && !ignore_set.is_match(relative)
                })
                .filter_map(std::result::Result::ok);

            for entry in iter {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "tolk") && path.is_file() {
                    let relative_path = relative_to_project_root(path, project_root);
                    if !ignore_set.is_match(path) && !ignore_set.is_match(&relative_path) {
                        files_to_format.push(path.to_path_buf());
                    }
                }
            }
        } else {
            anyhow::bail!("Path {} does not exist", path.display());
        }
    }

    if files_to_format.is_empty() {
        println!("{}", "No Tolk files found to format".yellow());
        return Ok(());
    }

    let mut unformatted_files = Vec::new();
    let mut formatted_count = 0;
    let mut error_count = 0;

    for file_path in files_to_format {
        let display_path = path_for_display(&file_path, &current_dir);
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?;

        match tolk_fmt::format_source(
            &content,
            tolk_fmt::FormatOptions {
                width,
                separate_import_groups,
            },
        ) {
            Ok(formatted) => {
                if content != formatted {
                    if check {
                        unformatted_files.push(file_path.clone());

                        let diff = TextDiff::from_lines(&content, &formatted);
                        println!("Diff in {}:", display_path.display().bold());

                        for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
                            for change in hunk.iter_changes() {
                                let (sign, value) = match change.tag() {
                                    ChangeTag::Delete => {
                                        ("-".red().to_string(), change.value().red().to_string())
                                    }
                                    ChangeTag::Insert => (
                                        "+".green().to_string(),
                                        change.value().green().to_string(),
                                    ),
                                    ChangeTag::Equal => (
                                        " ".dimmed().to_string(),
                                        change.value().dimmed().to_string(),
                                    ),
                                };
                                print!("{sign}{value}");
                            }
                        }
                        println!();
                    } else {
                        fs::write(&file_path, formatted)
                            .with_context(|| format!("Failed to write {}", file_path.display()))?;
                        formatted_count += 1;
                        println!("{} {}", "Formatted".green(), display_path.display());
                    }
                }
            }
            Err(err) => {
                if emit_parse_errors_if_any(&display_path, &content)? {
                    error_count += 1;
                    continue;
                }
                eprintln!("{} {}: {}", "Error".red(), display_path.display(), err);
                error_count += 1;
            }
        }
    }

    if check {
        if !unformatted_files.is_empty() {
            anyhow::bail!("Files are not formatted");
        } else if error_count > 0 {
            if error_count == 1 {
                anyhow::bail!("Formatting check failed due to syntax error in 1 file");
            }
            anyhow::bail!("Formatting check failed due to syntax errors in {error_count} files");
        }
        println!("{}", "All files are properly formatted".green());
    } else {
        if formatted_count > 0 {
            println!("\n{} {} files formatted", "Done:".green(), formatted_count);
        } else if error_count == 0 {
            println!("{}", "All files are already formatted".green());
        }

        if error_count > 0 {
            if error_count == 1 {
                anyhow::bail!("Failed to format 1 file due to syntax error");
            }
            anyhow::bail!("Failed to format {error_count} files due to syntax errors");
        }
    }

    Ok(())
}

fn emit_parse_errors_if_any(file_path: &Path, source: &str) -> Result<bool> {
    let source_file = tolk_syntax::parse(source)
        .with_context(|| format!("Failed to parse {}", file_path.display()))?;
    if !source_file.has_errors() {
        // another kind of error
        return Ok(false);
    }

    let mut files = SimpleFiles::new();
    let file_id = files.add(file_path.display().to_string(), source.to_owned());

    let writer = StandardStream::stderr(match color_mode() {
        ColorMode::Auto => ColorChoice::Auto,
        ColorMode::Always => ColorChoice::Always,
        ColorMode::Never => ColorChoice::Never,
    });

    let mut config = term::Config::default();
    let mut styles = term::Styles::default();
    styles.header_error.set_intense(false);
    config.styles = styles;
    config.chars = term::Chars::ascii();

    for parse_error in source_file.errors() {
        let start = byte_offset_from_point(&parse_error.span.start, source).min(source.len());
        let mut end = byte_offset_from_point(&parse_error.span.end, source).min(source.len());
        if end < start {
            end = start;
        }

        let diagnostic = Diagnostic::new(Severity::Error)
            .with_code("C001")
            .with_message(parse_error.message)
            .with_labels(vec![Label::primary(file_id, start..end)]);
        term::emit(&mut writer.lock(), &config, &files, &diagnostic)?;
    }

    Ok(true)
}

fn byte_offset_from_point(point: &Point, source: &str) -> usize {
    let lines = source.lines().collect::<Vec<_>>();
    let mut offset = 0;

    for i in 0..point.row {
        if i < lines.len() {
            offset += lines[i].len() + 1; // +1 for newline
        }
    }

    if point.row < lines.len() {
        offset += point.column;
    }

    offset
}

fn relative_to_project_root(path: &Path, project_root: &Path) -> PathBuf {
    if let Some(relative) = pathdiff::diff_paths(path, project_root) {
        return relative;
    }

    if let Ok(canonical_path) = dunce::canonicalize(path)
        && let Ok(canonical_project_root) = dunce::canonicalize(project_root)
        && let Some(relative) = pathdiff::diff_paths(canonical_path, canonical_project_root)
    {
        return relative;
    }

    path.to_path_buf()
}

fn path_for_display(path: &Path, current_dir: &Path) -> PathBuf {
    let normalized_current_dir = current_dir
        .absolutize()
        .map_or_else(|_| current_dir.to_path_buf(), std::borrow::Cow::into_owned);

    let normalized_path = path
        .absolutize_from(&normalized_current_dir)
        .map_or_else(|_| path.to_path_buf(), std::borrow::Cow::into_owned);

    pathdiff::diff_paths(&normalized_path, &normalized_current_dir).unwrap_or(normalized_path)
}
