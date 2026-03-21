use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::{Path, PathBuf};
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
    let output = git(project_root, &["config", "--local", "--get", key]);
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}

fn sibling_dir(project_root: &Path, name: &str) -> PathBuf {
    let path = project_root
        .parent()
        .expect("project should have a parent directory")
        .join(name);
    fs::create_dir_all(&path).expect("failed to create sibling directory");
    path
}

fn git_config_set(project_root: &Path, key: &str, value: &str) {
    let output = git(project_root, &["config", "--local", key, value]);
    assert!(
        output.status.success(),
        "git config failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_config_set_file(config_path: &Path, key: &str, value: &str) {
    let output = Command::new("git")
        .args(["config", "--file"])
        .arg(config_path)
        .arg(key)
        .arg(value)
        .output()
        .unwrap_or_else(|err| panic!("failed to run git config --file {:?}: {err}", config_path));
    assert!(
        output.status.success(),
        "git config --file failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
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
fn test_hooks_new_default_non_interactive() {
    let project = ProjectBuilder::new("hooks-new-default").build();

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("default")
        .run()
        .success();

    output.assert_snapshot_matches("integration/snapshots/hooks/test_hooks_new_default.stdout.txt");
    output.assert_file_snapshot_matches(
        ".githooks/pre-commit",
        "integration/snapshots/hooks/test_hooks_new_default.pre-commit.txt",
    );
}

#[cfg(unix)]
#[test]
fn test_hooks_new_interactive_defaults_to_default() {
    use expectrl::Eof;
    use std::time::Duration;

    let project = ProjectBuilder::new("hooks-new-interactive-default").build();
    let mut session = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(10)));

    session.expect("Hooks template:");
    session.send_line("", "failed to select default hooks template");
    session.expect("Created default hooks scaffold in .githooks");
    session.expect(Eof);

    session.assert_file_snapshot_matches(
        ".githooks/pre-commit",
        "integration/snapshots/hooks/test_hooks_new_default.pre-commit.txt",
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
        .arg("default")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_existing_pre_commit.stderr.txt",
        );
}

#[test]
fn test_hooks_new_fails_when_local_hooks_are_already_configured() {
    let project = ProjectBuilder::new("hooks-new-existing-local-hooks").build();
    init_git_repo(project.path());
    git_config_set(project.path(), "core.hooksPath", "custom-hooks");

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("default")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_existing_local_hooks.stderr.txt",
        );
}

#[test]
fn test_hooks_new_uses_auto_detected_project_root_from_nested_directory() {
    let project = ProjectBuilder::new("hooks-new-nested-auto-detect").build();
    let nested_dir = project.path().join("nested/deeper");
    fs::create_dir_all(&nested_dir).expect("failed to create nested directory");

    let output = project
        .acton()
        .current_dir(&nested_dir)
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

    assert!(
        !nested_dir.join(".githooks").exists(),
        ".githooks must not be created in the process working directory"
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
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_status_not_installed.stdout.txt",
        );
}

#[test]
fn test_hooks_install_fails_when_local_hooks_are_already_configured() {
    let project = ProjectBuilder::new("hooks-install-existing-local-hooks")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());
    git_config_set(project.path(), "core.hooksPath", "custom-hooks");

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("install")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_install_existing_local_hooks.stderr.txt",
        );
}

#[test]
fn test_hooks_commands_use_project_root_flag_outside_project_root() {
    let project = ProjectBuilder::new("hooks-project-root-flag")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());

    let outside_dir = sibling_dir(project.path(), "hooks-project-root-flag-outside");
    let project_root = project.path().to_string_lossy().to_string();

    project
        .acton()
        .current_dir(&outside_dir)
        .arg("--project-root")
        .arg(&project_root)
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
        .current_dir(&outside_dir)
        .arg("--project-root")
        .arg(&project_root)
        .arg("hooks")
        .arg("status")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_status.stdout.txt");

    project
        .acton()
        .current_dir(&outside_dir)
        .arg("--project-root")
        .arg(&project_root)
        .arg("hooks")
        .arg("uninstall")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_uninstall.stdout.txt");

    assert_eq!(git_config_get(project.path(), "core.hooksPath"), None);

    project
        .acton()
        .current_dir(&outside_dir)
        .arg("--project-root")
        .arg(&project_root)
        .arg("hooks")
        .arg("status")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_status_not_installed.stdout.txt",
        );
}

#[test]
fn test_hooks_status_reports_mismatch() {
    let project = ProjectBuilder::new("hooks-status-mismatch").build();
    init_git_repo(project.path());
    git_config_set(project.path(), "core.hooksPath", "custom-hooks");

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_status_not_installed.stdout.txt",
        );
}

#[test]
fn test_hooks_status_ignores_global_hooks_path() {
    let project = ProjectBuilder::new("hooks-status-global-only").build();
    init_git_repo(project.path());

    let home = tempfile::TempDir::new().expect("failed to create temp HOME");
    let global_config = home.path().join(".gitconfig");
    git_config_set_file(&global_config, "core.hooksPath", ".githooks");
    let global_config = global_config.to_string_lossy().to_string();

    project
        .acton()
        .env("GIT_CONFIG_GLOBAL", &global_config)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_status_not_installed.stdout.txt",
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
        .arg("default")
        .run()
        .success();

    let mode = fs::metadata(project.path().join(".githooks/pre-commit"))
        .expect("pre-commit metadata must exist")
        .permissions()
        .mode();

    assert_eq!(mode & 0o111, 0o111, "pre-commit must be executable");
}
