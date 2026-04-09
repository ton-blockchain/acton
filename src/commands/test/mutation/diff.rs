use crate::commands::test::mutation::{MutationCandidate, MutationSource};
use acton_config::color::OwoColorize;
use acton_config::test::{MutationDiffMode, TestConfig};
use anyhow::anyhow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Clone, Copy, Debug)]
struct ChangedLineRange {
    start_line: usize,
    end_line: usize,
}

#[derive(Debug)]
enum ChangedLineSelection {
    FullFile,
    Ranges(Vec<ChangedLineRange>),
}

#[derive(Debug)]
pub(crate) struct MutationDiffScope {
    pub(crate) label: String,
    changed_files: BTreeMap<PathBuf, ChangedLineSelection>,
}

impl MutationDiffScope {
    pub(crate) fn changed_source_count(&self, sources: &[MutationSource]) -> usize {
        sources
            .iter()
            .filter(|source| self.changed_files.contains_key(&source.relative_path))
            .count()
    }

    pub(crate) fn matches_candidate(
        &self,
        source: &MutationSource,
        candidate: &MutationCandidate,
    ) -> bool {
        let Some(selection) = self.changed_files.get(&source.relative_path) else {
            return false;
        };

        match selection {
            ChangedLineSelection::FullFile => true,
            ChangedLineSelection::Ranges(ranges) => {
                let start_line = candidate.node.start_position().row + 1;
                let end_line = candidate.node.end_position().row + 1;
                ranges
                    .iter()
                    .any(|range| start_line <= range.end_line && end_line >= range.start_line)
            }
        }
    }
}

fn run_git(project_root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = process::Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(|err| anyhow!("Failed to execute git {args:?}: {err}"))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    };

    anyhow::bail!("git {args:?} failed: {details}");
}

fn git_has_ref(project_root: &Path, reference: &str) -> bool {
    process::Command::new("git")
        .args(["rev-parse", "--verify", reference])
        .current_dir(project_root)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn parse_changed_range(hunk_header: &str) -> anyhow::Result<Option<ChangedLineRange>> {
    let plus_segment = hunk_header
        .split_whitespace()
        .find(|segment| segment.starts_with('+'))
        .ok_or_else(|| anyhow!("Missing added-line segment in git diff hunk: {hunk_header}"))?;

    let raw_range = plus_segment.trim_start_matches('+');
    let (start_raw, count_raw) = raw_range.split_once(',').unwrap_or((raw_range, "1"));
    let start_line = start_raw
        .parse::<usize>()
        .map_err(|err| anyhow!("Invalid start line in git diff hunk '{hunk_header}': {err}"))?;
    let line_count = count_raw
        .parse::<usize>()
        .map_err(|err| anyhow!("Invalid line count in git diff hunk '{hunk_header}': {err}"))?;

    if line_count == 0 {
        return Ok(None);
    }

    Ok(Some(ChangedLineRange {
        start_line,
        end_line: start_line + line_count - 1,
    }))
}

fn collect_changed_files_from_diff(
    diff_output: &str,
) -> anyhow::Result<BTreeMap<PathBuf, ChangedLineSelection>> {
    let mut changed_files = BTreeMap::new();
    let mut current_file: Option<PathBuf> = None;

    for line in diff_output.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            if path == "/dev/null" {
                current_file = None;
                continue;
            }

            let path = PathBuf::from(path);
            changed_files
                .entry(path.clone())
                .or_insert_with(|| ChangedLineSelection::Ranges(Vec::new()));
            current_file = Some(path);
            continue;
        }

        if !line.starts_with("@@") {
            continue;
        }

        let Some(path) = &current_file else {
            continue;
        };

        let Some(range) = parse_changed_range(line)? else {
            continue;
        };

        let entry = changed_files
            .entry(path.clone())
            .or_insert_with(|| ChangedLineSelection::Ranges(Vec::new()));
        if let ChangedLineSelection::Ranges(ranges) = entry {
            ranges.push(range);
        }
    }

    Ok(changed_files)
}

fn append_untracked_files(
    project_root: &Path,
    changed_files: &mut BTreeMap<PathBuf, ChangedLineSelection>,
) -> anyhow::Result<()> {
    let output = run_git(
        project_root,
        &["ls-files", "--others", "--exclude-standard", "--"],
    )?;
    for path in output.lines().filter(|line| !line.trim().is_empty()) {
        changed_files.insert(PathBuf::from(path), ChangedLineSelection::FullFile);
    }
    Ok(())
}

fn resolve_worktree_base(project_root: &Path) -> &'static str {
    if git_has_ref(project_root, "HEAD") {
        "HEAD"
    } else {
        // Empty tree object used by git when diffing an unborn branch.
        "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
    }
}

fn normalized_diff_ref(raw: Option<&String>) -> Option<String> {
    raw.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

pub(crate) fn collect_mutation_diff_scope(
    project_root: &Path,
    config: &TestConfig,
) -> anyhow::Result<Option<MutationDiffScope>> {
    let explicit_ref = normalized_diff_ref(config.mutation_diff_ref.as_ref());
    let Some(mode) = config.mutation_diff else {
        if explicit_ref.is_some() {
            anyhow::bail!(
                "Use {} {} when passing {}",
                "--mutation-diff".yellow(),
                "<worktree|ref|branch>".yellow(),
                "--mutation-diff-ref".yellow()
            );
        }
        return Ok(None);
    };

    if mode == MutationDiffMode::Worktree && explicit_ref.is_some() {
        anyhow::bail!(
            "Do not pass {} {} with {} {}",
            "--mutation-diff-ref".yellow(),
            "<REF>".yellow(),
            "--mutation-diff".yellow(),
            "worktree".yellow()
        );
    }

    let (label, diff_base) = match mode {
        MutationDiffMode::Worktree => (
            "worktree".to_owned(),
            resolve_worktree_base(project_root).to_owned(),
        ),
        MutationDiffMode::Ref => {
            let diff_ref = explicit_ref.ok_or_else(|| {
                anyhow!(
                    "Use {} {} with {} {}",
                    "--mutation-diff-ref".yellow(),
                    "<REF>".yellow(),
                    "--mutation-diff".yellow(),
                    "ref".yellow()
                )
            })?;
            (format!("ref {diff_ref}"), diff_ref)
        }
        MutationDiffMode::Branch => {
            let target = if let Some(diff_ref) = explicit_ref {
                diff_ref
            } else {
                run_git(
                    project_root,
                    &[
                        "rev-parse",
                        "--abbrev-ref",
                        "--symbolic-full-name",
                        "@{upstream}",
                    ],
                )
                .map_err(|_| {
                    anyhow!(
                        "{} {} needs an upstream branch or {} {}",
                        "--mutation-diff".yellow(),
                        "branch".yellow(),
                        "--mutation-diff-ref".yellow(),
                        "<REF>".yellow()
                    )
                })?
            };

            let merge_base = run_git(project_root, &["merge-base", "HEAD", &target])?;
            (format!("branch vs {target}"), merge_base)
        }
    };

    let diff_output = run_git(
        project_root,
        &[
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--unified=0",
            "--relative",
            "--no-prefix",
            "--no-renames",
            &diff_base,
            "--",
        ],
    )?;

    let mut changed_files = collect_changed_files_from_diff(&diff_output)?;
    append_untracked_files(project_root, &mut changed_files)?;

    Ok(Some(MutationDiffScope {
        label,
        changed_files,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn normalized_diff_ref_trims_and_rejects_blank_values() {
        assert_eq!(normalized_diff_ref(None), None);
        assert_eq!(normalized_diff_ref(Some(&String::new())), None);
        assert_eq!(normalized_diff_ref(Some(&"   ".to_owned())), None);
        assert_eq!(
            normalized_diff_ref(Some(&"  origin/main  ".to_owned())),
            Some("origin/main".to_owned())
        );
    }

    #[test]
    fn parse_changed_range_handles_single_line_and_multi_line_hunks() {
        let single = parse_changed_range("@@ -7 +9 @@").expect("single-line hunk");
        assert!(single.is_some());
        let single = single.expect("single range");
        assert_eq!(single.start_line, 9);
        assert_eq!(single.end_line, 9);

        let multi = parse_changed_range("@@ -7,2 +9,3 @@").expect("multi-line hunk");
        assert!(multi.is_some());
        let multi = multi.expect("multi range");
        assert_eq!(multi.start_line, 9);
        assert_eq!(multi.end_line, 11);
    }

    #[test]
    fn parse_changed_range_returns_none_for_zero_line_hunks() {
        let range = parse_changed_range("@@ -7,2 +9,0 @@").expect("zero-line hunk");
        assert!(range.is_none());
    }

    #[test]
    fn parse_changed_range_errors_for_invalid_headers() {
        let missing_plus =
            parse_changed_range("@@ -7,2 @@").expect_err("missing plus segment should fail");
        assert!(
            missing_plus
                .to_string()
                .contains("Missing added-line segment in git diff hunk")
        );

        let invalid_start =
            parse_changed_range("@@ -7 +x,2 @@").expect_err("invalid start line should fail");
        assert!(invalid_start.to_string().contains("Invalid start line"));

        let invalid_count =
            parse_changed_range("@@ -7 +9,x @@").expect_err("invalid line count should fail");
        assert!(invalid_count.to_string().contains("Invalid line count"));
    }

    #[test]
    fn collect_changed_files_from_diff_tracks_ranges_and_added_files() {
        let diff = "\
diff --git a/contracts/simple.tolk b/contracts/simple.tolk
--- contracts/simple.tolk
+++ contracts/simple.tolk
@@ -7 +9 @@
@@ -12,0 +14,2 @@
diff --git a/contracts/new.tolk b/contracts/new.tolk
--- /dev/null
+++ contracts/new.tolk
@@ -0,0 +1,4 @@
";

        let changed = collect_changed_files_from_diff(diff).expect("parse diff");

        let simple = changed
            .get(&PathBuf::from("contracts/simple.tolk"))
            .expect("simple file should exist");
        match simple {
            ChangedLineSelection::FullFile => panic!("simple file should use ranges"),
            ChangedLineSelection::Ranges(ranges) => {
                assert_eq!(ranges.len(), 2);
                assert_eq!(ranges[0].start_line, 9);
                assert_eq!(ranges[0].end_line, 9);
                assert_eq!(ranges[1].start_line, 14);
                assert_eq!(ranges[1].end_line, 15);
            }
        }

        let new_file = changed
            .get(&PathBuf::from("contracts/new.tolk"))
            .expect("new file should exist");
        match new_file {
            ChangedLineSelection::FullFile => {
                panic!("new file should use parsed ranges before untracked promotion")
            }
            ChangedLineSelection::Ranges(ranges) => {
                assert_eq!(ranges.len(), 1);
                assert_eq!(ranges[0].start_line, 1);
                assert_eq!(ranges[0].end_line, 4);
            }
        }
    }

    #[test]
    fn collect_mutation_diff_scope_rejects_diff_ref_without_mode() {
        let project_root = tempdir().expect("tempdir");
        let config = TestConfig {
            mutation_diff: None,
            mutation_diff_ref: Some("HEAD".to_owned()),
            ..Default::default()
        };

        let err = collect_mutation_diff_scope(project_root.path(), &config)
            .expect_err("diff-ref without mode should fail");
        let message = err.to_string();
        assert!(message.contains("Use"));
        assert!(message.contains("--mutation-diff"));
        assert!(message.contains("--mutation-diff-ref"));
        assert!(message.contains("<worktree|ref|branch>"));
    }

    #[test]
    fn collect_mutation_diff_scope_rejects_worktree_with_ref() {
        let project_root = tempdir().expect("tempdir");
        let config = TestConfig {
            mutation_diff: Some(MutationDiffMode::Worktree),
            mutation_diff_ref: Some("HEAD".to_owned()),
            ..Default::default()
        };

        let err = collect_mutation_diff_scope(project_root.path(), &config)
            .expect_err("worktree with ref should fail");
        let message = err.to_string();
        assert!(message.contains("Do not pass"));
        assert!(message.contains("--mutation-diff-ref"));
        assert!(message.contains("--mutation-diff"));
        assert!(message.contains("worktree"));
    }

    #[test]
    fn collect_mutation_diff_scope_requires_git_repository_for_worktree_mode() {
        let project_root = tempdir().expect("tempdir");
        let config = TestConfig {
            mutation_diff: Some(MutationDiffMode::Worktree),
            ..Default::default()
        };

        let err = collect_mutation_diff_scope(project_root.path(), &config)
            .expect_err("worktree diff outside git repo should fail");
        assert!(err.to_string().contains("git [\"diff\""));
        assert!(err.to_string().contains("failed:"));
    }
}
