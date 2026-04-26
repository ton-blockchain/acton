use crate::common::{acton_exe, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MUTATION_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {
    assert (in.valueCoins > 0) throw 5;
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun addOne(x: int): int {
    return x + 1;
}
";

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test always pass`() {
    expect(1).toEqual(1);
}
"#;

const DEPENDENT_MUTATION_CONTRACT: &str = r#"
import "../gen/dependency.code.tolk"

fun onInternalMessage(in: InMessage) {
    assert (in.valueCoins > 0) throw 5;
    val code = dependencyCompiledCode();
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const BROKEN_DEPENDENCY_MUTATION_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {
    THIS IS A SYNTAX ERROR
}

fun onBouncedMessage(_: InMessageBounced) {}
";

const COMPILE_ERROR_MUTATION_CONTRACT: &str = r"
get fun mustFail(): int {
    throw 5;
}

get fun addOne(x: int): int {
    return x + 1;
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const NO_MUTATION_POINTS_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const MUTATION_CONTRACT_ARITHMETIC_CHANGED: &str = r"
fun onInternalMessage(in: InMessage) {
    assert (in.valueCoins > 0) throw 5;
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun addOne(x: int): int {
    return x + 2;
}
";

const CUSTOM_MUTATION_RULES_JSON: &str = r#"[
  {
    "name": "replace_plus_with_multiply_custom",
    "description": "Replace + with *",
    "explanation": "Custom arithmetic mutation loaded from JSON.",
    "level": "major",
    "group": "arithmetic",
    "matcher": {
      "type": "query",
      "query": "(binary_operator operator_name: \"+\" @op)",
      "capture": "op"
    },
    "edit": {
      "type": "replace",
      "replacement": "*"
    }
  }
]"#;

const CUSTOM_MUTATION_RULES_OVERRIDE_JSON: &str = r#"[
  {
    "name": "replace_plus_with_minus",
    "description": "Replace + with -",
    "explanation": "Custom override mutation loaded from JSON.",
    "level": "major",
    "group": "arithmetic",
    "matcher": {
      "type": "query",
      "query": "(binary_operator operator_name: \"+\" @op)",
      "capture": "op"
    },
    "edit": {
      "type": "replace",
      "replacement": "-"
    }
  }
]"#;

fn mutation_project(name: &str) -> Project {
    ProjectBuilder::new(name)
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
}

fn git(project_root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git {args:?}: {err}"))
}

fn git_ok(project_root: &Path, args: &[&str], context: &str) {
    let output = git(project_root, args);
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_git_repo(project_root: &Path) {
    git_ok(project_root, &["init", "-q"], "git init");
    git_ok(
        project_root,
        &["branch", "-M", "main"],
        "git branch -M main",
    );
    git_ok(
        project_root,
        &["config", "user.email", "acton-tests@example.com"],
        "git config user.email",
    );
    git_ok(
        project_root,
        &["config", "user.name", "Acton Tests"],
        "git config user.name",
    );
}

fn commit_all(project_root: &Path, message: &str) {
    git_ok(project_root, &["add", "."], "git add");
    git_ok(project_root, &["commit", "-qm", message], "git commit");
}

fn checkout_new_branch(project_root: &Path, branch: &str) {
    git_ok(
        project_root,
        &["checkout", "-qb", branch],
        "git checkout -b",
    );
}

fn set_upstream(project_root: &Path, target: &str) {
    git_ok(
        project_root,
        &["branch", "--set-upstream-to", target],
        "git branch --set-upstream-to",
    );
}

fn write_simple_contract(project: &Project, source: &str) {
    fs::write(project.path().join("contracts/simple.tolk"), source)
        .expect("failed to update simple contract");
}

fn mutation_session_path(project: &Project, session_id: &str) -> std::path::PathBuf {
    project
        .path()
        .join("build")
        .join("mutation-sessions")
        .join(format!("{session_id}.jsonl"))
}

fn read_jsonl_events(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .unwrap_or_else(|err| panic!("failed to parse jsonl line '{line}': {err}"))
        })
        .collect()
}

fn many_asserts_contract(assert_count: usize) -> String {
    let mut asserts = String::new();
    for _ in 0..assert_count {
        asserts.push_str("    assert (in.valueCoins > 0) throw 5;\n");
    }

    format!(
        r"
fun onInternalMessage(in: InMessage) {{
{asserts}
}}

fun onBouncedMessage(_: InMessageBounced) {{}}
"
    )
}

fn spawn_mutation_process(project: &Project, args: &[&str]) -> Child {
    Command::new(acton_exe())
        .current_dir(project.path())
        .arg("test")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("failed to spawn mutation process: {err}"))
}

fn wait_for_event_count(progress_path: &Path, minimum_lines: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let line_count = fs::read_to_string(progress_path).ok().map_or(0, |content| {
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        });
        if line_count >= minimum_lines {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {} lines in {}",
            minimum_lines,
            progress_path.display()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn send_interrupt(child: &Child) {
    let status = Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status()
        .expect("failed to send SIGINT to child");
    assert!(status.success(), "kill -INT failed with status {status}");
}

#[cfg(not(unix))]
fn send_interrupt(child: &mut Child) {
    child.kill().expect("failed to kill child process");
}

#[test]
fn mutate_requires_mutate_contract() {
    mutation_project("j-mutate-requires-contract")
        .acton()
        .test()
        .arg("--mutate")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_requires_mutate_contract.stderr.txt",
        );
}

#[test]
fn mutate_fails_for_unknown_contract() {
    mutation_project("j-mutate-unknown-contract")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("missing")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_fails_for_unknown_contract.stderr.txt",
        );
}

#[test]
fn mutate_reports_summary() {
    mutation_project("j-mutate-summary")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_reports_summary.stdout.txt",
        );
}

#[test]
fn mutate_disable_rules_filter_mutants() {
    mutation_project("j-mutate-disable-rule")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-disable-rules")
        .arg("remove_assert")
        .arg("--mutation-disable-rules")
        .arg("replace_plus_with_minus")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_disable_rule_filters_mutants.stdout.txt",
        );
}

#[test]
fn mutate_diff_ref_requires_ref() {
    mutation_project("j-mutate-diff-ref-requires-ref")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("ref")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_ref_requires_ref.stderr.txt",
        );
}

#[test]
fn mutate_diff_ref_without_mode_is_rejected() {
    mutation_project("j-mutate-diff-ref-without-mode")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff-ref")
        .arg("HEAD")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_ref_without_mode_is_rejected.stderr.txt",
        );
}

#[test]
fn mutate_diff_worktree_rejects_ref() {
    mutation_project("j-mutate-diff-worktree-rejects-ref")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("worktree")
        .arg("--mutation-diff-ref")
        .arg("HEAD")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_worktree_rejects_ref.stderr.txt",
        );
}

#[test]
fn mutate_diff_worktree_filters_mutants() {
    let project = mutation_project("j-mutate-diff-worktree");
    init_git_repo(project.path());
    commit_all(project.path(), "initial");
    write_simple_contract(&project, MUTATION_CONTRACT_ARITHMETIC_CHANGED);

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("worktree")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_worktree_filters_mutants.stdout.txt",
        );
}

#[test]
fn mutate_diff_branch_requires_upstream_or_ref() {
    let project = mutation_project("j-mutate-diff-branch-missing-upstream");
    init_git_repo(project.path());
    commit_all(project.path(), "initial");
    checkout_new_branch(project.path(), "feature/no-upstream");
    write_simple_contract(&project, MUTATION_CONTRACT_ARITHMETIC_CHANGED);

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("branch")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_branch_requires_upstream_or_ref.stderr.txt",
        );
}

#[test]
fn mutate_diff_ref_filters_mutants() {
    let project = mutation_project("j-mutate-diff-ref");
    init_git_repo(project.path());
    commit_all(project.path(), "initial");
    write_simple_contract(&project, MUTATION_CONTRACT_ARITHMETIC_CHANGED);
    commit_all(project.path(), "change arithmetic");

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("ref")
        .arg("--mutation-diff-ref")
        .arg("HEAD~1")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_ref_filters_mutants.stdout.txt",
        );
}

#[test]
fn mutate_diff_branch_filters_mutants() {
    let project = mutation_project("j-mutate-diff-branch");
    init_git_repo(project.path());
    commit_all(project.path(), "initial");
    checkout_new_branch(project.path(), "feature/mutation-diff");
    set_upstream(project.path(), "main");
    write_simple_contract(&project, MUTATION_CONTRACT_ARITHMETIC_CHANGED);
    commit_all(project.path(), "change arithmetic");

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("branch")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_branch_filters_mutants.stdout.txt",
        );
}

#[test]
fn mutate_diff_branch_with_explicit_ref_filters_mutants() {
    let project = mutation_project("j-mutate-diff-branch-explicit-ref");
    init_git_repo(project.path());
    commit_all(project.path(), "initial");
    checkout_new_branch(project.path(), "feature/mutation-diff-explicit-ref");
    write_simple_contract(&project, MUTATION_CONTRACT_ARITHMETIC_CHANGED);
    commit_all(project.path(), "change arithmetic");

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-diff")
        .arg("branch")
        .arg("--mutation-diff-ref")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_diff_branch_with_explicit_ref_filters_mutants.stdout.txt",
        );
}

#[test]
fn mutate_levels_filter_mutants_from_cli() {
    mutation_project("j-mutate-levels-cli")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-levels")
        .arg("critical,major")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_levels_filter_mutants_from_cli.stdout.txt",
        );
}

#[test]
fn mutate_id_filters_specific_mutant_from_cli() {
    mutation_project("j-mutate-id-cli")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-id")
        .arg("2")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_id_filters_specific_mutant_from_cli.stdout.txt",
        );
}

#[test]
fn mutate_id_rejects_unknown_mutant() {
    mutation_project("j-mutate-id-missing")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-id")
        .arg("10")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_id_rejects_unknown_mutant.stderr.txt",
        );
}

#[test]
fn mutate_id_accepts_comma_separated_list() {
    mutation_project("j-mutate-id-list")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-id")
        .arg("1,3")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_id_accepts_comma_separated_list.stdout.txt",
        );
}

#[test]
fn mutate_id_zero_is_rejected() {
    mutation_project("j-mutate-id-zero")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-id")
        .arg("0")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_id_zero_is_rejected.stderr.txt",
        );
}

#[test]
fn mutate_workers_zero_is_rejected() {
    mutation_project("j-mutate-workers-zero")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-workers")
        .arg("0")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_workers_zero_is_rejected.stderr.txt",
        );
}

#[test]
fn mutate_workers_requires_numeric_value() {
    mutation_project("j-mutate-workers-invalid")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-workers")
        .arg("abc")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_workers_requires_numeric_value.stderr.txt",
        );
}

#[test]
fn mutate_id_must_match_current_filters() {
    mutation_project("j-mutate-id-with-filters")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-levels")
        .arg("critical")
        .arg("--mutation-id")
        .arg("2")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_id_must_match_current_filters.stderr.txt",
        );
}

#[test]
fn mutate_custom_rules_file_via_cli() {
    ProjectBuilder::new("j-mutate-custom-rules-cli")
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file("mutation-rules.json", CUSTOM_MUTATION_RULES_JSON)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-rules-file")
        .arg("mutation-rules.json")
        .arg("--mutation-disable-rules")
        .arg("remove_assert")
        .arg("--mutation-disable-rules")
        .arg("replace_plus_with_minus")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_via_cli.stdout.txt",
        );
}

#[test]
fn mutate_custom_rules_file_overrides_builtin_rule_by_name() {
    ProjectBuilder::new("j-mutate-custom-rules-override")
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file("mutation-rules.json", CUSTOM_MUTATION_RULES_OVERRIDE_JSON)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-rules-file")
        .arg("mutation-rules.json")
        .arg("--mutation-disable-rules")
        .arg("remove_assert")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_overrides_builtin_rule_by_name.stdout.txt",
        );
}

#[test]
fn mutate_custom_rules_file_from_config() {
    ProjectBuilder::new("j-mutate-custom-rules-config")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file("mutation-rules.json", CUSTOM_MUTATION_RULES_JSON)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-custom-rules-config"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
rules-file = "mutation-rules.json"
disable-rules = ["remove_assert", "replace_plus_with_minus"]
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_from_config.stdout.txt",
        );
}

#[test]
fn mutate_custom_rules_file_cli_overrides_config() {
    ProjectBuilder::new("j-mutate-custom-rules-cli-overrides-config")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file("mutation-rules.json", CUSTOM_MUTATION_RULES_JSON)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-custom-rules-cli-overrides-config"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
rules-file = "missing-rules.json"
disable-rules = ["remove_assert", "replace_plus_with_minus"]
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-rules-file")
        .arg("mutation-rules.json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_cli_overrides_config.stdout.txt",
        );
}

#[test]
fn mutate_custom_rules_file_rejects_invalid_json() {
    ProjectBuilder::new("j-mutate-custom-rules-invalid-json")
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file("mutation-rules.json", "{ invalid json")
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-rules-file")
        .arg("mutation-rules.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_rejects_invalid_json.stderr.txt",
        );
}

#[test]
fn mutate_custom_rules_file_rejects_missing_file() {
    ProjectBuilder::new("j-mutate-custom-rules-missing-file")
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-rules-file")
        .arg("missing-rules.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_rejects_missing_file.stderr.txt",
        );
}

#[test]
fn mutate_custom_rules_file_rejects_duplicate_rule_names() {
    ProjectBuilder::new("j-mutate-custom-rules-duplicate-names")
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "mutation-rules.json",
            r#"[
  {
    "name": "dup_rule",
    "description": "Replace + with *",
    "explanation": "First duplicate rule.",
    "level": "major",
    "group": "arithmetic",
    "matcher": {
      "type": "query",
      "query": "(binary_operator operator_name: \"+\" @op)",
      "capture": "op"
    },
    "edit": {
      "type": "replace",
      "replacement": "*"
    }
  },
  {
    "name": "dup_rule",
    "description": "Replace + with -",
    "explanation": "Second duplicate rule.",
    "level": "major",
    "group": "arithmetic",
    "matcher": {
      "type": "query",
      "query": "(binary_operator operator_name: \"+\" @op)",
      "capture": "op"
    },
    "edit": {
      "type": "replace",
      "replacement": "-"
    }
  }
]"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-rules-file")
        .arg("mutation-rules.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_custom_rules_file_rejects_duplicate_rule_names.stderr.txt",
        );
}

#[test]
fn mutate_session_writes_progress_jsonl() {
    let project = mutation_project("j-mutate-session-jsonl");
    let session_id = "session-jsonl";

    let output = project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1,2")
        .run()
        .success();
    output.assert_snapshot_matches(
        "integration/snapshots/test-runner/test_runner_mutate/mutate_session_writes_progress_jsonl.stdout.txt",
    );

    let progress_path = mutation_session_path(&project, session_id);
    assert!(
        progress_path.is_file(),
        "expected mutation session progress file at {}",
        progress_path.display()
    );

    let events = read_jsonl_events(&progress_path);
    assert_eq!(
        events
            .first()
            .and_then(|event| event.get("event"))
            .and_then(Value::as_str),
        Some("session_started")
    );
    assert_eq!(
        events
            .first()
            .and_then(|event| event.get("session_id"))
            .and_then(Value::as_str),
        Some(session_id)
    );
    assert_eq!(
        events
            .last()
            .and_then(|event| event.get("event"))
            .and_then(Value::as_str),
        Some("session_finished")
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("mutation_completed"))
            .count(),
        2
    );
}

#[test]
fn mutate_session_resume_skips_completed_mutants() {
    let project = mutation_project("j-mutate-session-resume");
    let session_id = "session-resume";
    let progress_path = mutation_session_path(&project, session_id);

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1,2")
        .run()
        .success();

    let original_lines = fs::read_to_string(&progress_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", progress_path.display()))
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let session_started_line = original_lines
        .iter()
        .find(|line| line.contains("\"event\":\"session_started\""))
        .expect("missing session_started line");
    let first_completed_line = original_lines
        .iter()
        .find(|line| line.contains("\"event\":\"mutation_completed\""))
        .expect("missing mutation_completed line");
    let completed_event = serde_json::from_str::<Value>(first_completed_line)
        .expect("failed to parse first mutation_completed event");
    let completed_id = completed_event
        .get("record")
        .and_then(|record| record.get("id"))
        .and_then(Value::as_u64)
        .expect("missing completed mutation ID") as usize;
    let remaining_id = match completed_id {
        1 => 2,
        2 => 1,
        other => panic!("unexpected completed mutation ID {other}"),
    };
    fs::write(
        &progress_path,
        format!("{session_started_line}\n{first_completed_line}\n"),
    )
    .unwrap_or_else(|err| panic!("failed to rewrite {}: {err}", progress_path.display()));

    let output = project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1,2")
        .run()
        .success();
    let stdout = output.get_normalized_stdout();
    assert!(
        stdout.contains(&format!("Mutation {remaining_id}/3")),
        "expected resumed run to execute remaining mutation {remaining_id}, got:\n{stdout}"
    );
    assert!(
        !stdout.contains(&format!("Mutation {completed_id}/3")),
        "expected resumed run to skip completed mutation {completed_id}, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Total mutants        2"),
        "expected resumed summary in stdout, got:\n{stdout}"
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test-runner/test_runner_mutate/mutate_session_resume_skips_completed_mutants.stderr.txt",
    );

    let resumed_events = read_jsonl_events(&progress_path);
    assert_eq!(
        resumed_events
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("session_started"))
            .count(),
        1
    );
    assert_eq!(
        resumed_events
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("mutation_completed"))
            .count(),
        2
    );
    assert_eq!(
        resumed_events
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("session_finished"))
            .count(),
        1
    );
}

#[test]
fn mutate_session_rejects_selection_mismatch() {
    let project = mutation_project("j-mutate-session-mismatch");
    let session_id = "session-mismatch";

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .success();

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("2")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_session_rejects_selection_mismatch.stderr.txt",
        );
}

#[test]
fn mutate_session_header_shows_id_without_progress_path() {
    mutation_project("j-mutate-session-header")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg("session-header")
        .arg("--mutation-id")
        .arg("1")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_session_header_shows_id_without_progress_path.stdout.txt",
        );
}

#[test]
fn mutate_finished_session_is_idempotent() {
    let project = mutation_project("j-mutate-session-idempotent");
    let session_id = "session-idempotent";

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1,2")
        .run()
        .success();

    let output = project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1,2")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_finished_session_is_idempotent.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_finished_session_is_idempotent.stderr.txt",
        );

    let events = read_jsonl_events(&mutation_session_path(&project, session_id));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("session_finished"))
            .count(),
        1
    );
}

#[test]
fn mutate_session_rejects_contract_mismatch() {
    let project = ProjectBuilder::new("j-mutate-session-contract-mismatch")
        .contract("simple", MUTATION_CONTRACT)
        .contract("other", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build();
    let session_id = "session-contract-mismatch";

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .success();

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("other")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_session_rejects_contract_mismatch.stderr.txt",
        );
}

#[test]
fn mutate_session_rejects_duplicate_completed_entries() {
    let project = mutation_project("j-mutate-session-duplicate-completed");
    let session_id = "session-duplicate-completed";
    let progress_path = mutation_session_path(&project, session_id);

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .success();

    let original = fs::read_to_string(&progress_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", progress_path.display()));
    let duplicate_line = original
        .lines()
        .find(|line| line.contains("\"event\":\"mutation_completed\""))
        .expect("missing mutation_completed line");
    fs::write(&progress_path, format!("{original}{duplicate_line}\n"))
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", progress_path.display()));

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_session_rejects_duplicate_completed_entries.stderr.txt",
        );
}

#[test]
fn mutate_session_rejects_foreign_event_session_id() {
    let project = mutation_project("j-mutate-session-foreign-event");
    let session_id = "session-foreign-event";
    let progress_path = mutation_session_path(&project, session_id);

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .success();

    let corrupted = fs::read_to_string(&progress_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", progress_path.display()))
        .replacen(session_id, "foreign-session", 1);
    fs::write(&progress_path, corrupted)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", progress_path.display()));

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-session-id")
        .arg(session_id)
        .arg("--mutation-id")
        .arg("1")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_session_rejects_foreign_event_session_id.stderr.txt",
        );
}

#[test]
#[cfg(unix)]
fn mutate_session_resume_after_ctrl_c() {
    let contract = many_asserts_contract(80);
    let project = ProjectBuilder::new("j-mutate-session-interrupt-resume")
        .contract("main", &contract)
        .test_file("mutation", PASSING_TEST)
        .build();
    let session_id = "session-interrupt-resume";
    let progress_path = mutation_session_path(&project, session_id);

    let child = spawn_mutation_process(
        &project,
        &[
            "--mutate",
            "--mutate-contract",
            "main",
            "--mutation-session-id",
            session_id,
        ],
    );

    wait_for_event_count(&progress_path, 2);
    send_interrupt(&child);
    let output = child
        .wait_with_output()
        .expect("failed to wait for interrupted mutation process");

    assert!(
        !output.status.success(),
        "expected interrupted mutation process to fail"
    );

    let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    assert!(
        stdout.contains("Interrupted by Ctrl+C."),
        "expected interrupt message in stdout, got:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "Mutation session session-interrupt-resume was left unfinished and can be resumed."
        ),
        "expected session resume hint in stdout, got:\n{stdout}"
    );
    assert!(
        stdout.contains(
            "acton test --mutate --mutate-contract main --mutation-session-id session-interrupt-resume"
        ),
        "expected resume command in stdout, got:\n{stdout}"
    );

    let before_resume = read_jsonl_events(&progress_path);
    let completed_before_resume = before_resume
        .iter()
        .filter(|event| event.get("event").and_then(Value::as_str) == Some("mutation_completed"))
        .count();
    assert!(
        completed_before_resume >= 1,
        "expected at least one completed mutation before interruption"
    );
    assert_eq!(
        before_resume
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("session_finished"))
            .count(),
        0
    );

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .arg("--mutation-session-id")
        .arg(session_id)
        .run()
        .success();

    let after_resume = read_jsonl_events(&progress_path);
    let selected_ids_len = after_resume
        .iter()
        .find(|event| event.get("event").and_then(Value::as_str) == Some("session_started"))
        .and_then(|event| event.get("selected_ids"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .expect("missing selected_ids in session_started event");
    let completed_after_resume = after_resume
        .iter()
        .filter(|event| event.get("event").and_then(Value::as_str) == Some("mutation_completed"))
        .count();
    assert_eq!(completed_after_resume, selected_ids_len);
    assert_eq!(
        after_resume
            .iter()
            .filter(|event| event.get("event").and_then(Value::as_str) == Some("session_finished"))
            .count(),
        1
    );
}

#[test]
fn mutate_minimum_percent_via_cli_fails_when_score_is_too_low() {
    mutation_project("j-mutate-minimum-percent-cli")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--mutation-minimum-percent")
        .arg("100")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_minimum_percent_via_cli.stdout.txt",
        );
}

#[test]
fn mutate_uses_mutation_diff_from_config() {
    let project = ProjectBuilder::new("j-mutate-config-diff-worktree")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-diff-worktree"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
diff = "worktree"
"#,
        )
        .build();
    init_git_repo(project.path());
    commit_all(project.path(), "initial");
    write_simple_contract(&project, MUTATION_CONTRACT_ARITHMETIC_CHANGED);

    project
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_uses_mutation_diff_from_config.stdout.txt",
        );
}

#[test]
fn mutate_uses_disable_rules_from_config() {
    ProjectBuilder::new("j-mutate-config-disable-rules")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-disable-rules"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
disable-rules = ["remove_assert", "replace_plus_with_minus", "replace_greater_than_with_greater_or_equal"]
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_uses_disable_rules_from_config.stdout.txt",
        );
}

#[test]
fn mutate_uses_minimum_percent_from_config() {
    ProjectBuilder::new("j-mutate-config-minimum-percent")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-minimum-percent"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
minimum-percent = 100
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_uses_minimum_percent_from_config.stdout.txt",
        );
}

#[test]
fn mutate_uses_mutation_levels_from_config() {
    ProjectBuilder::new("j-mutate-config-mutation-levels")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-mutation-levels"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
mutation-levels = ["critical"]
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_uses_mutation_levels_from_config.stdout.txt",
        );
}

#[test]
fn mutate_rejects_invalid_minimum_percent_from_config() {
    ProjectBuilder::new("j-mutate-config-invalid-minimum-percent")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-invalid-minimum-percent"
description = "A test project"
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
minimum-percent = 101
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_rejects_invalid_minimum_percent_from_config.stderr.txt",
        );
}

#[test]
fn mutate_contract_with_dependencies() {
    ProjectBuilder::new("j-mutate-contract-with-dependencies")
        .contract("dependency", MUTATION_CONTRACT)
        .contract_with_deps("main", DEPENDENT_MUTATION_CONTRACT, vec!["dependency"])
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_contract_with_dependencies.stdout.txt",
        );
}

#[test]
fn mutate_contract_with_library_ref_dependency() {
    ProjectBuilder::new("j-mutate-contract-with-library-ref-dependency")
        .contract("dependency", MUTATION_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            DEPENDENT_MUTATION_CONTRACT,
            vec![("dependency", Some("library_ref"), None, None)],
        )
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_contract_with_library_ref_dependency.stdout.txt",
        );
}

#[test]
fn mutate_contract_with_dependencies_and_clear_cache() {
    ProjectBuilder::new("j-mutate-contract-with-dependencies-clear-cache")
        .contract("dependency", MUTATION_CONTRACT)
        .contract_with_deps("main", DEPENDENT_MUTATION_CONTRACT, vec!["dependency"])
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .clear_cache()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_contract_with_dependencies_and_clear_cache.stdout.txt",
        );
}

#[test]
fn mutate_reports_dependency_build_failure() {
    ProjectBuilder::new("j-mutate-dependency-build-failure")
        .contract("dependency", BROKEN_DEPENDENCY_MUTATION_CONTRACT)
        .contract_with_deps("main", DEPENDENT_MUTATION_CONTRACT, vec!["dependency"])
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_reports_dependency_build_failure.stderr.txt",
        );
}

#[test]
fn mutate_compile_errors_are_excluded_from_score() {
    ProjectBuilder::new("j-mutate-compile-errors-excluded-from-score")
        .contract("main", COMPILE_ERROR_MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_compile_errors_are_excluded_from_score.stdout.txt",
        );
}

#[test]
fn mutate_reports_no_mutation_points() {
    ProjectBuilder::new("j-mutate-no-mutation-points")
        .contract("main", NO_MUTATION_POINTS_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_reports_no_mutation_points.stdout.txt",
        );
}

#[test]
#[ignore = "benchmark scenario for local perf tracking"]
fn mutate_benchmark_large_mutant_set() {
    use std::time::Instant;
    let contract = many_asserts_contract(120);

    let start = Instant::now();
    let output = ProjectBuilder::new("j-mutate-benchmark-large-mutant-set")
        .contract("main", &contract)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success();
    let elapsed = start.elapsed();

    let stdout = output.get_normalized_stdout();
    let mutant_count = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("Mutants:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .expect("mutation output must contain mutant count");

    assert!(
        mutant_count >= 100,
        "expected at least 100 mutants in benchmark scenario, got {mutant_count}\n{stdout}"
    );

    if let Some(max_ms) = std::env::var("MUTATION_BENCH_MAX_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
    {
        assert!(
            elapsed.as_millis() <= max_ms,
            "mutation benchmark regression: elapsed={}ms exceeds MUTATION_BENCH_MAX_MS={}ms",
            elapsed.as_millis(),
            max_ms
        );
    }

    eprintln!("mutation benchmark: {mutant_count} mutants processed in {elapsed:?}");
}
