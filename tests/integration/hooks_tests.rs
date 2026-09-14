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
        .unwrap_or_else(|err| panic!("failed to run git {args:?}: {err}"))
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

fn git_config_add(project_root: &Path, key: &str, value: &str) {
    let output = git(project_root, &["config", "--local", "--add", key, value]);
    assert!(
        output.status.success(),
        "git config --add failed: {}",
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
        .unwrap_or_else(|err| panic!("failed to run git config --file {config_path:?}: {err}"));
    assert!(
        output.status.success(),
        "git config --file failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_hooks_new_empty_non_interactive() {
    let project = ProjectBuilder::new("hooks-new-empty").build();
    init_git_repo(project.path());

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
        ".githooks/pre-push",
        "integration/snapshots/hooks/test_hooks_new_empty.hook.txt",
    );
}

#[test]
fn test_hooks_new_default_non_interactive() {
    let project = ProjectBuilder::new("hooks-new-default").build();
    init_git_repo(project.path());

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .run()
        .success();

    output.assert_snapshot_matches("integration/snapshots/hooks/test_hooks_new_default.stdout.txt");
    output.assert_file_snapshot_matches(
        ".githooks/pre-push",
        "integration/snapshots/hooks/test_hooks_new_default.hook.txt",
    );
}

#[test]
fn test_hooks_new_creates_only_selected_hook() {
    use std::fmt::Write as _;

    let mut report = String::new();
    for hook in ["pre-push", "pre-commit"] {
        for template in ["default", "empty"] {
            let project = ProjectBuilder::new(&format!("hooks-{hook}-{template}")).build();
            init_git_repo(project.path());
            let output = project
                .acton()
                .current_dir(project.path())
                .arg("hooks")
                .arg("new")
                .arg("--hook")
                .arg(hook)
                .arg("--template")
                .arg(template)
                .run()
                .success();

            output.assert_file_snapshot_matches(
                &format!(".githooks/{hook}"),
                &format!("integration/snapshots/hooks/test_hooks_new_{template}.hook.txt"),
            );
            let mut files = fs::read_dir(project.path().join(".githooks"))
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            files.sort();
            writeln!(
                report,
                "{hook}, {template}: {files:?}\n{}",
                output.get_stdout()
            )
            .unwrap();
        }
    }

    crate::common::assertion().eq(
        report.trim_end(),
        snapbox::file!["snapshots/hooks/test_hooks_new_selected_hook.txt"],
    );
}

#[test]
fn test_hooks_new_rejects_invalid_hook() {
    let project = ProjectBuilder::new("hooks-invalid-hook").build();
    project
        .acton()
        .arg("hooks")
        .arg("new")
        .arg("--hook")
        .arg("post-commit")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_invalid_hook.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn test_default_hook_requires_acton_and_propagates_failures() {
    use std::fmt::Write as _;
    use std::os::unix::fs::PermissionsExt;

    let mut report = String::new();
    for hook in ["pre-push", "pre-commit"] {
        // Regression for #1065: missing tools must explain the Git environment without skipping checks.
        let project = ProjectBuilder::new(&format!("hooks-execution-{hook}"))
            .raw_file(
                "hook bin/acton",
                r#"#!/bin/sh
printf '%s\n' "$*"
case "$1" in
    check) exit "$ACTON_HOOK_TEST_CHECK_EXIT" ;;
    fmt) exit "$ACTON_HOOK_TEST_FMT_EXIT" ;;
    *) exit 99 ;;
esac
"#,
            )
            .build();
        init_git_repo(project.path());
        project
            .acton()
            .current_dir(project.path())
            .arg("hooks")
            .arg("new")
            .arg("--hook")
            .arg(hook)
            .arg("--template")
            .arg("default")
            .run()
            .success();

        let bin_dir = project.path().join("hook bin");
        fs::set_permissions(bin_dir.join("acton"), fs::Permissions::from_mode(0o755))
            .expect("fake Acton must be executable");

        for (scenario, missing_acton, check_exit, fmt_exit) in [
            ("missing Acton", true, 0, 0),
            ("check failed", false, 41, 0),
            ("fmt failed", false, 0, 42),
            ("success", false, 0, 0),
        ] {
            let output = Command::new(project.path().join(".githooks").join(hook))
                .current_dir(project.path())
                .env(
                    "PATH",
                    if missing_acton {
                        project.path().join("missing-bin")
                    } else {
                        bin_dir.clone()
                    },
                )
                .env("ACTON_HOOK_TEST_CHECK_EXIT", check_exit.to_string())
                .env("ACTON_HOOK_TEST_FMT_EXIT", fmt_exit.to_string())
                .output()
                .expect("generated hook must run");
            writeln!(
                report,
                "{hook}: {scenario}\nexit: {}\nstdout:\n{}stderr:\n{}",
                output.status.code().expect("hook must exit normally"),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            )
            .expect("hook report must be writable");
        }
    }

    crate::common::assertion().eq(
        report.trim_end(),
        snapbox::file!["snapshots/hooks/test_default_hook_execution.txt"],
    );
}

#[cfg(unix)]
#[test]
fn test_hooks_new_interactive_defaults_to_default() {
    use expectrl::Eof;
    use std::time::Duration;

    let project = ProjectBuilder::new("hooks-new-interactive-default").build();
    init_git_repo(project.path());
    let mut session = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(10)));

    session.expect("Git hooks:");
    session.send_line("", "failed to select default pre-push hook");
    session.expect("Hooks template:");
    session.send_line("", "failed to select default hooks template");
    session.expect("Created default pre-push hook in .githooks");
    session.expect(Eof);

    session.assert_file_snapshot_matches(
        ".githooks/pre-push",
        "integration/snapshots/hooks/test_hooks_new_default.hook.txt",
    );
}

#[test]
fn test_hooks_new_fails_when_githooks_exists() {
    let project = ProjectBuilder::new("hooks-new-existing")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
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
fn test_hooks_new_preserves_existing_pre_push() {
    let project = ProjectBuilder::new("hooks-existing-pre-push")
        .raw_file(".githooks/pre-push", "#!/bin/sh\necho custom checks\n")
        .build();
    init_git_repo(project.path());

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--hook")
        .arg("pre-commit")
        .run()
        .failure();
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/hooks/test_hooks_new_existing_pre_push.stderr.txt",
    );
    output.assert_file_snapshot_matches(
        ".githooks/pre-push",
        "integration/snapshots/hooks/test_hooks_new_existing_pre_push.hook.txt",
    );
}

#[test]
fn test_hooks_new_fails_when_githooks_directory_exists_without_pre_commit() {
    let project = ProjectBuilder::new("hooks-new-existing-dir")
        .raw_file(".githooks/post-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("default")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_existing_githooks_dir.stderr.txt",
        );
}

#[test]
fn test_hooks_new_fails_when_local_hooks_are_already_configured() {
    let project = ProjectBuilder::new("hooks-new-existing-local-hooks").build();
    init_git_repo(project.path());
    git_config_set(project.path(), "core.hooksPath", "custom-hooks");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
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
fn test_hooks_new_fails_without_local_git_directory() {
    let project = ProjectBuilder::new("hooks-new-missing-local-git").build();

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("default")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_missing_local_git_dir.stderr.txt",
        );
}

#[test]
fn test_hooks_new_fails_when_parent_git_repo_exists_without_local_git_directory() {
    let project = ProjectBuilder::new("hooks-new-parent-existing-local-hooks").build();
    let outer_repo = project
        .path()
        .parent()
        .expect("project should have a parent directory");
    init_git_repo(outer_repo);
    git_config_set(outer_repo, "core.hooksPath", "custom-hooks");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("default")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_new_missing_local_git_dir.stderr.txt",
        );
}

#[test]
fn test_hooks_new_uses_auto_detected_project_root_from_nested_directory() {
    let project = ProjectBuilder::new("hooks-new-nested-auto-detect").build();
    init_git_repo(project.path());
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
        ".githooks/pre-push",
        "integration/snapshots/hooks/test_hooks_new_empty.hook.txt",
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
fn test_hooks_install_fails_when_githooks_is_missing() {
    let project = ProjectBuilder::new("hooks-install-missing-githooks").build();
    init_git_repo(project.path());

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("install")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_install_missing_githooks.stderr.txt",
        );
}

#[test]
fn test_hooks_install_fails_without_local_git_directory() {
    let project = ProjectBuilder::new("hooks-install-missing-local-git")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("install")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_manage_missing_local_git_dir.stderr.txt",
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
        .env("ACTON_LOG_DIR", ".acton/logs")
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
fn test_hooks_install_reports_equivalent_local_hooks_path_as_installed() {
    let project = ProjectBuilder::new("hooks-install-equivalent-local-hooks")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());
    git_config_set(project.path(), "core.hooksPath", "./.githooks");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("install")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_install_existing_equivalent_local_hooks.stderr.txt",
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
fn test_hooks_status_fails_without_local_git_directory() {
    let project = ProjectBuilder::new("hooks-status-missing-local-git").build();

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_manage_missing_local_git_dir.stderr.txt",
        );
}

#[test]
fn test_hooks_status_reports_equivalent_local_hooks_path_as_installed() {
    let project = ProjectBuilder::new("hooks-status-equivalent-local-hooks")
        .raw_file(".githooks/pre-commit", "#!/bin/sh\n")
        .build();
    init_git_repo(project.path());
    git_config_set(project.path(), "core.hooksPath", "./.githooks");

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("status")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_status.stdout.txt");
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

#[test]
fn test_hooks_uninstall_succeeds_when_hooks_are_not_installed() {
    let project = ProjectBuilder::new("hooks-uninstall-not-installed").build();
    init_git_repo(project.path());

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("uninstall")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_uninstall_not_installed.stdout.txt",
        );

    assert_eq!(git_config_get(project.path(), "core.hooksPath"), None);
}

#[test]
fn test_hooks_uninstall_removes_all_local_hooks_path_values() {
    let project = ProjectBuilder::new("hooks-uninstall-multiple-values").build();
    init_git_repo(project.path());
    git_config_add(project.path(), "core.hooksPath", ".githooks");
    git_config_add(project.path(), "core.hooksPath", "other-hooks");

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("uninstall")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/hooks/test_hooks_uninstall.stdout.txt");

    assert_eq!(git_config_get(project.path(), "core.hooksPath"), None);
}

#[test]
fn test_hooks_uninstall_fails_without_local_git_directory() {
    let project = ProjectBuilder::new("hooks-uninstall-missing-local-git").build();

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .current_dir(project.path())
        .arg("hooks")
        .arg("uninstall")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/hooks/test_hooks_manage_missing_local_git_dir.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn test_hooks_new_marks_pre_push_executable() {
    use std::os::unix::fs::PermissionsExt;

    let project = ProjectBuilder::new("hooks-new-executable").build();
    init_git_repo(project.path());

    project
        .acton()
        .current_dir(project.path())
        .arg("hooks")
        .arg("new")
        .arg("--template")
        .arg("default")
        .run()
        .success();

    let mode = fs::metadata(project.path().join(".githooks/pre-push"))
        .expect("pre-push metadata must exist")
        .permissions()
        .mode();

    assert_eq!(mode & 0o111, 0o111, "pre-push must be executable");
}
