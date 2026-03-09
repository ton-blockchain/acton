use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn git(project_root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git {:?}: {err}", args))
}

fn init_git_repo(project_root: &Path) {
    let output = git(project_root, &["init", "-q"]);
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_config_get(project_root: &Path, key: &str) -> Option<String> {
    let output = git(project_root, &["config", "--get", key]);
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}

#[test]
fn test_hooks_new_empty_non_interactive() {
    let project = ProjectBuilder::new("hooks-new-empty").build();

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("empty")
        .run()
        .success();

    output.assert_snapshot_matches("integration/snapshots/hooks/test_hooks_new_empty.stdout.txt");
    output.assert_file_snapshot_matches(
        ".githooks/pre-commit",
        "integration/snapshots/hooks/test_hooks_new_empty.pre-commit.txt",
    );
}

#[test]
fn test_hooks_new_ready_non_interactive() {
    let project = ProjectBuilder::new("hooks-new-ready").build();

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("ready")
        .run()
        .success();

    output.assert_snapshot_matches("integration/snapshots/hooks/test_hooks_new_ready.stdout.txt");
    output.assert_file_snapshot_matches(
        ".githooks/pre-commit",
        "integration/snapshots/hooks/test_hooks_new_ready.pre-commit.txt",
    );
}

#[cfg(unix)]
#[test]
fn test_hooks_new_interactive_defaults_to_empty() {
    use expectrl::Eof;
    use std::time::Duration;

    let project = ProjectBuilder::new("hooks-new-interactive-empty").build();
    let mut session = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(10)));

    session.expect("Hooks template:");
    session.send_line("", "failed to select default empty hooks template");
    session.expect("Created empty hooks scaffold in .githooks");
    session.expect(Eof);

    session.assert_file_snapshot_matches(
        ".githooks/pre-commit",
        "integration/snapshots/hooks/test_hooks_new_empty.pre-commit.txt",
    );
}

#[test]
fn test_hooks_new_fails_when_githooks_exists() {
    let project = ProjectBuilder::new("hooks-new-existing")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("ready")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_existing_dir.stderr.txt",
        );
}

#[test]
fn test_hooks_install_status_uninstall_flow() {
    let project = ProjectBuilder::new("hooks-install-status-uninstall")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("install")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_install.stdout.txt");

    assert_eq!(
        git_config_get(project.path(), "core.hooksPath").as_deref(),
        Some(".githooks")
    );

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_status.stdout.txt");

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("uninstall")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_uninstall.stdout.txt");

    assert_eq!(git_config_get(project.path(), "core.hooksPath"), None);

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_status_unset.stderr.txt",
        );
}

#[test]
fn test_hooks_status_reports_mismatch() {
    let project = ProjectBuilder::new("hooks-status-mismatch").build();
    init_git_repo(project.path());

    let output = git(
        project.path(),
        &["config", "core.hooksPath", "custom-hooks"],
    );
    assert!(
        output.status.success(),
        "git config failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_status_mismatch.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn test_hooks_new_marks_pre_commit_executable() {
    use std::os::unix::fs::PermissionsExt;

    let project = ProjectBuilder::new("hooks-new-executable").build();

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("ready")
        .run()
        .success();

    let mode = fs::metadata(project.path().join(".githooks/pre-commit"))
        .expect("pre-commit metadata must exist")
        .permissions()
        .mode();

    assert_eq!(mode & 0o111, 0o111, "pre-commit must be executable");
}
