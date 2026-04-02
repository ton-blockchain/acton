use acton_config::color::OwoColorize;
use rustc_hash::FxHashSet;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use tolk_linter::diagnostic::{Applicability, Diagnostic};
use tolk_resolver::FileDb;

pub(super) fn apply_fixes(file_db: &FileDb, diagnostics: &[Diagnostic]) -> anyhow::Result<()> {
    let mut fixes_by_file: BTreeMap<PathBuf, Vec<(usize, usize, String)>> = BTreeMap::new();
    let mut total_diags_by_file: HashMap<PathBuf, usize> = HashMap::new();
    let mut fixed_diags_by_file: HashMap<PathBuf, usize> = HashMap::new();

    let mut applied_edits = FxHashSet::default();

    for diag in diagnostics {
        let file_info = file_db
            .get_by_id(diag.file_id)
            .ok_or_else(|| anyhow::anyhow!("File info not found for file_id {}", diag.file_id))?;

        let file_path = &file_info.index().path;

        *total_diags_by_file.entry(file_path.clone()).or_default() += 1;

        if diag.fixes.is_empty() {
            continue;
        }

        // For now, apply only the first fix for each diagnostic
        let fix = &diag.fixes[0];
        if fix.applicability != Applicability::Auto {
            continue;
        }

        *fixed_diags_by_file.entry(file_path.clone()).or_default() += 1;

        for edit in &fix.edits {
            if !applied_edits.insert(edit) {
                // don't apply duplicate edits (for example for case checker)
                continue;
            }

            let edit_file_id = edit.file_id;
            let edit_file_info = file_db.get_by_id(edit_file_id).ok_or_else(|| {
                anyhow::anyhow!("File info not found for edit file_id {edit_file_id}")
            })?;
            let edit_file_path = edit_file_info.index().path.clone();

            fixes_by_file.entry(edit_file_path).or_default().push((
                edit.span.start(),
                edit.span.end(),
                edit.replacement.clone(),
            ));
        }
    }

    let current_dir = std::env::current_dir().unwrap_or_default();

    for (file_path, mut fixes) in fixes_by_file {
        // sort fixes by start position in reverse order (to avoid offset issues when multiple fixes)
        fixes.sort_by(|(a_start, _, _), (b_start, _, _)| b_start.cmp(a_start));

        let content = fs::read_to_string(&file_path)?;
        let mut new_content = content;
        let mut applied_fixes = 0;

        for (start, end, replacement) in fixes {
            if start <= new_content.len()
                && end <= new_content.len()
                && start <= end
                && new_content.is_char_boundary(start)
                && new_content.is_char_boundary(end)
            {
                new_content.replace_range(start..end, &replacement);
                applied_fixes += 1;
            }
        }

        if applied_fixes > 0 {
            fs::write(&file_path, new_content)?;

            let relative_path = pathdiff::diff_paths(&file_path, &current_dir)
                .unwrap_or_else(|| PathBuf::from(&file_path));

            let total_issues = *total_diags_by_file.get(&file_path).unwrap_or(&0);
            let fixed_issues = *fixed_diags_by_file.get(&file_path).unwrap_or(&0);

            if total_issues == 0 {
                println!(
                    "     {} {} {} to {}",
                    "Applied".green().bold(),
                    applied_fixes,
                    if applied_fixes == 1 { "fix" } else { "fixes" },
                    relative_path.display().cyan(),
                );
            } else if fixed_issues == total_issues {
                println!(
                    "       {} all issues in {}",
                    "Fixed".green().bold(),
                    relative_path.display().cyan()
                );
            } else {
                let remaining = total_issues - fixed_issues;
                println!(
                    "     {} {} {} to {}, {} {} remaining",
                    "Applied".green().bold(),
                    fixed_issues,
                    if fixed_issues == 1 { "fix" } else { "fixes" },
                    relative_path.display().cyan(),
                    remaining,
                    if remaining == 1 { "issue" } else { "issues" }
                );
            }
        }
    }

    Ok(())
}

pub(super) fn filter_fixed_diagnostics(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| {
            !d.fixes
                .iter()
                .any(|f| f.applicability == Applicability::Auto)
        })
        .cloned()
        .collect()
}
