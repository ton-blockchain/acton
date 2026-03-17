use acton_config::color::OwoColorize;
use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use walkdir::WalkDir;

mod linter;
mod stdlib;

const DEFAULT_STDLIB_OUT: &str = "docs/content/docs/standard_library";
const DEFAULT_TOLK_STDLIB_OUT: &str = "docs/content/docs/tolk_standard_library";
const DEFAULT_LINTER_OUT: &str = "docs/content/docs/linting/rules";
const TOLK_STDLIB_SRC: &str = "crates/tolkc/assets/tolk-stdlib";
const GITHUB_SOURCE_BASE: &str = "https://github.com/i582/acton/blob/master";

#[derive(Debug, Clone)]
struct DocgenOutputPaths {
    stdlib_out_dir: PathBuf,
    tolk_stdlib_out_dir: PathBuf,
    linter_out_dir: PathBuf,
}

pub fn docgen_cmd(output: Option<String>, check: bool) -> Result<()> {
    let output_paths = resolve_output_paths(output);

    if check {
        let temp_out = TempDir::new()?;
        let generated_out = resolve_output_paths(Some(
            temp_out
                .path()
                .join("standard_library")
                .display()
                .to_string(),
        ));
        generate_docs(&generated_out)?;

        let changed_files = print_docgen_diff(&output_paths, &generated_out)?;
        if changed_files > 0 {
            anyhow::bail!("Documentation is out of date. Run `acton docgen` to regenerate.")
        }

        return Ok(());
    }

    generate_docs(&output_paths)
}

fn resolve_output_paths(output: Option<String>) -> DocgenOutputPaths {
    let stdlib_output = output.unwrap_or_else(|| DEFAULT_STDLIB_OUT.to_string());
    let stdlib_out_dir = PathBuf::from(&stdlib_output);
    let tolk_stdlib_out_dir = stdlib_out_dir
        .parent()
        .map(|parent| parent.join("tolk_standard_library"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TOLK_STDLIB_OUT));
    let linter_out_dir = stdlib_out_dir
        .parent()
        .map(|parent| parent.join("linting").join("rules"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LINTER_OUT));

    DocgenOutputPaths {
        stdlib_out_dir,
        tolk_stdlib_out_dir,
        linter_out_dir,
    }
}

fn generate_docs(output_paths: &DocgenOutputPaths) -> Result<()> {
    stdlib::generate_stdlib_docs(
        Path::new("lib"),
        Path::new(TOLK_STDLIB_SRC),
        &output_paths.stdlib_out_dir,
        &output_paths.tolk_stdlib_out_dir,
    )?;
    linter::generate_linter_docs(&output_paths.linter_out_dir)?;
    Ok(())
}

fn print_docgen_diff(current: &DocgenOutputPaths, generated: &DocgenOutputPaths) -> Result<usize> {
    let mut changed_files = 0usize;

    changed_files += print_dir_diff(&current.stdlib_out_dir, &generated.stdlib_out_dir)?;
    changed_files += print_dir_diff(&current.tolk_stdlib_out_dir, &generated.tolk_stdlib_out_dir)?;
    changed_files += print_dir_diff(&current.linter_out_dir, &generated.linter_out_dir)?;

    Ok(changed_files)
}

fn print_dir_diff(current_dir: &Path, generated_dir: &Path) -> Result<usize> {
    let current_files = read_text_tree(current_dir)?;
    let generated_files = read_text_tree(generated_dir)?;

    let mut keys: BTreeSet<PathBuf> = current_files.keys().cloned().collect();
    keys.extend(generated_files.keys().cloned());

    let mut changed_files = 0usize;

    for relative_path in keys {
        let current_content = current_files.get(&relative_path);
        let generated_content = generated_files.get(&relative_path);

        if current_content == generated_content {
            continue;
        }

        changed_files += 1;

        let current_text = current_content.map_or("", String::as_str);
        let generated_text = generated_content.map_or("", String::as_str);
        let target = current_dir.join(&relative_path);
        eprintln!("Diff in {}:", target.display().to_string().bold());

        let diff = TextDiff::from_lines(current_text, generated_text);
        for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
            for change in hunk.iter_changes() {
                let (sign, value) = match change.tag() {
                    ChangeTag::Delete => ("-".red().to_string(), change.value().red().to_string()),
                    ChangeTag::Insert => {
                        ("+".green().to_string(), change.value().green().to_string())
                    }
                    ChangeTag::Equal => (
                        " ".dimmed().to_string(),
                        change.value().dimmed().to_string(),
                    ),
                };
                eprint!("{sign}{value}");
            }
        }
        eprintln!();
    }

    Ok(changed_files)
}

fn read_text_tree(root: &Path) -> Result<BTreeMap<PathBuf, String>> {
    let mut files = BTreeMap::new();
    if !root.exists() {
        return Ok(files);
    }

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let relative = path.strip_prefix(root)?.to_path_buf();
        let content = fs::read_to_string(path)?;
        files.insert(relative, content);
    }

    Ok(files)
}
