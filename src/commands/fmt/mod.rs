use crate::config::ActonConfig;
use anyhow::{Context, Result};
use globset::{Glob, GlobSetBuilder};
use owo_colors::OwoColorize;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn fmt_cmd(paths: Vec<String>, check: bool) -> Result<()> {
    let config = ActonConfig::load().ok();
    let fmt_settings = config.as_ref().and_then(|c| c.fmt.as_ref());

    let width = fmt_settings.and_then(|s| s.width).unwrap_or(100);
    let ignore_patterns = fmt_settings.and_then(|s| s.ignore.as_ref());

    let mut ignore_builder = GlobSetBuilder::new();
    if let Some(ignores) = ignore_patterns {
        for pattern in ignores {
            ignore_builder.add(Glob::new(pattern)?);
        }
    }
    let ignore_set = ignore_builder.build()?;

    let mut files_to_format = Vec::new();

    let search_paths = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.into_iter().map(PathBuf::from).collect()
    };

    for path in search_paths {
        if path.is_file() {
            if path.extension().is_some_and(|ext| ext == "tolk") {
                files_to_format.push(path);
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "tolk") {
                    let relative_path = path.strip_prefix("./").unwrap_or(path);

                    if !ignore_set.is_match(relative_path) {
                        files_to_format.push(relative_path.to_path_buf());
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
        let content = fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?;

        match tolkfmt::format_source(&content, width) {
            Ok(formatted) => {
                if content != formatted {
                    if check {
                        unformatted_files.push(file_path.clone());

                        let diff = TextDiff::from_lines(&content, &formatted);
                        println!("Diff in {}:", file_path.display().bold());

                        for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
                            for change in hunk.iter_changes() {
                                let (sign, style) = match change.tag() {
                                    ChangeTag::Delete => ("-", owo_colors::Style::new().red()),
                                    ChangeTag::Insert => ("+", owo_colors::Style::new().green()),
                                    ChangeTag::Equal => (" ", owo_colors::Style::new().dimmed()),
                                };
                                print!("{}{}", style.style(sign), style.style(change.value()));
                            }
                        }
                        println!();
                    } else {
                        fs::write(&file_path, formatted)
                            .with_context(|| format!("Failed to write {}", file_path.display()))?;
                        formatted_count += 1;
                        println!("{} {}", "Formatted".green(), file_path.display());
                    }
                }
            }
            Err(err) => {
                eprintln!("{} {}: {}", "Error".red(), file_path.display(), err);
                error_count += 1;
            }
        }
    }

    if check {
        if !unformatted_files.is_empty() {
            anyhow::bail!("Files are not formatted");
        } else if error_count > 0 {
            anyhow::bail!(
                "Formatting check failed due to syntax errors in {} files",
                error_count
            );
        } else {
            println!("{}", "All files are properly formatted".green());
        }
    } else {
        if formatted_count > 0 {
            println!("\n{} {} files formatted", "Done:".green(), formatted_count);
        } else if error_count == 0 {
            println!("{}", "All files are already formatted".green());
        }

        if error_count > 0 {
            anyhow::bail!(
                "Failed to format {} files due to syntax errors",
                error_count
            );
        }
    }

    Ok(())
}
