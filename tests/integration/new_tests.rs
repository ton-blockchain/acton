use crate::common::acton_exe;
use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use serde_json::Value as JsonValue;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
const ACTON_SHIM: &str = r#"#!/bin/sh
set -eu
exec "$ACTON_BIN" "$@"
"#;

const LOCALNET_TEST_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

#[cfg(unix)]
fn make_executable(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[cfg(unix)]
fn setup_real_npm_toolchain(project_root: &Path) -> (String, PathBuf) {
    let bin_dir = project_root.join("bin");
    let cache_dir = project_root.join(".npm-cache");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(&cache_dir).unwrap();

    let acton_path = bin_dir.join("acton");

    fs::write(&acton_path, ACTON_SHIM).unwrap();
    make_executable(&acton_path);

    let path_env = format!(
        "{}:{}",
        bin_dir.display(),
        env::var("PATH").unwrap_or_default()
    );

    (path_env, cache_dir)
}

#[cfg(unix)]
fn setup_fake_git_stage_failure_toolchain(project_root: &Path) -> String {
    const GIT_STAGE_FAILURE_SHIM: &str = r#"#!/bin/sh
set -eu

case "${1:-}" in
  --version)
    printf '%s\n' 'git version 2.42.0'
    exit 0
    ;;
  config)
    if [ "${2:-}" = "--get" ] && [ "${3:-}" = "user.name" ]; then
      printf '%s\n' 'Test User'
      exit 0
    fi
    exit 1
    ;;
  init)
    /bin/mkdir -p .git
    exit 0
    ;;
  add)
    printf '%s\n' 'simulated git add failure' >&2
    exit 1
    ;;
  *)
    printf 'unexpected fake git invocation: %s\n' "$*" >&2
    exit 1
    ;;
esac
"#;

    let bin_dir = project_root.join("fake-git-bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let git_path = bin_dir.join("git");
    fs::write(&git_path, GIT_STAGE_FAILURE_SHIM).unwrap();
    make_executable(&git_path);

    bin_dir.display().to_string()
}

#[cfg(unix)]
fn setup_fake_git_init_failure_toolchain(project_root: &Path) -> String {
    const GIT_INIT_FAILURE_SHIM: &str = r#"#!/bin/sh
set -eu

case "${1:-}" in
  --version)
    printf '%s\n' 'git version 2.42.0'
    exit 0
    ;;
  config)
    if [ "${2:-}" = "--get" ] && [ "${3:-}" = "user.name" ]; then
      printf '%s\n' 'Test User'
      exit 0
    fi
    exit 1
    ;;
  init)
    printf '%s\n' 'simulated git init failure' >&2
    exit 1
    ;;
  *)
    printf 'unexpected fake git invocation: %s\n' "$*" >&2
    exit 1
    ;;
esac
"#;

    let bin_dir = project_root.join("fake-git-init-failure-bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let git_path = bin_dir.join("git");
    fs::write(&git_path, GIT_INIT_FAILURE_SHIM).unwrap();
    make_executable(&git_path);

    bin_dir.display().to_string()
}

#[cfg(unix)]
fn setup_fake_git_without_user_name_toolchain(project_root: &Path) -> String {
    const GIT_NO_USER_NAME_SHIM: &str = r#"#!/bin/sh
set -eu

case "${1:-}" in
  --version)
    printf '%s\n' 'git version 2.42.0'
    exit 0
    ;;
  config)
    if [ "${2:-}" = "--get" ] && [ "${3:-}" = "user.name" ]; then
      exit 1
    fi
    exit 1
    ;;
  init)
    /bin/mkdir -p .git
    exit 0
    ;;
  add)
    exit 0
    ;;
  *)
    printf 'unexpected fake git invocation: %s\n' "$*" >&2
    exit 1
    ;;
esac
"#;

    let bin_dir = project_root.join("fake-git-no-user-bin");
    fs::create_dir_all(&bin_dir).unwrap();

    let git_path = bin_dir.join("git");
    fs::write(&git_path, GIT_NO_USER_NAME_SHIM).unwrap();
    make_executable(&git_path);

    bin_dir.display().to_string()
}

#[cfg(unix)]
fn run_npm_command(
    project_dir: &Path,
    path_env: &str,
    cache_dir: &Path,
    args: &[&str],
) -> std::process::Output {
    Command::new("npm")
        .args(args)
        .current_dir(project_dir)
        .env("PATH", path_env)
        .env("ACTON_BIN", acton_exe())
        .env("NPM_CONFIG_CACHE", cache_dir)
        .env("NPM_CONFIG_AUDIT", "false")
        .env("NPM_CONFIG_FUND", "false")
        .env("NPM_CONFIG_FETCH_RETRIES", "0")
        .env("NPM_CONFIG_FETCH_TIMEOUT", "5000")
        .env("NPM_CONFIG_PROGRESS", "false")
        .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
        .env("NPM_CONFIG_PREFER_OFFLINE", "true")
        .output()
        .unwrap()
}

#[cfg(unix)]
fn is_npm_available() -> bool {
    Command::new("npm")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn npm_failure_looks_environment_specific(output: &std::process::Output) -> bool {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    [
        "EAI_AGAIN",
        "ENOTFOUND",
        "ECONNREFUSED",
        "ECONNRESET",
        "ETIMEDOUT",
        "fetch failed",
        "getaddrinfo",
        "network request",
        "Exit handler never called!",
        "cb() never called!",
    ]
    .iter()
    .any(|pattern| combined.contains(pattern))
}

fn git_config_get(project_root: &Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--local", "--get", key])
        .current_dir(project_root)
        .output()
        .unwrap();

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        None
    }
}

fn write_localnet_wallet_config(project_dir: &Path, wallet_name: &str) {
    let wallets_toml = format!(
        r#"[wallets."{wallet_name}"]
kind = "v4r2"
workchain = 0
keys = {{ mnemonic = "{LOCALNET_TEST_MNEMONIC}" }}
"#
    );
    fs::write(project_dir.join("wallets.toml"), wallets_toml)
        .expect("Failed to write localnet wallets.toml");
}

fn append_localnet_network(project_dir: &Path, base_url: &str) {
    let acton_toml_path = project_dir.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("Failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r#"

[networks.localnet]
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
"#
    ));
    fs::write(&acton_toml_path, acton_toml).expect("Failed to write Acton.toml with localnet");
}

fn assert_new_project_localnet_deploy_snapshot(
    fixture_name: &str,
    template: &str,
    app: bool,
    wallet_name: &str,
    deploy_script_path: &str,
    snapshot_path: &str,
) {
    let project = ProjectBuilder::new(fixture_name)
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("foobar");

    create_project_from_template(&project, &project_dir, template, app);

    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success();

    write_localnet_wallet_config(&project_dir, wallet_name);

    let node = project
        .litenode()
        .current_dir(&project_dir)
        .args(["--accounts", wallet_name])
        .start();
    append_localnet_network(&project_dir, &node.base_url());

    project
        .acton()
        .script(deploy_script_path)
        .current_dir(&project_dir)
        .broadcast()
        .verify_network("localnet")
        .run()
        .success()
        .assert_snapshot_matches(snapshot_path);

    node.stop();
}

fn create_project_from_template(project: &Project, project_dir: &Path, template: &str, app: bool) {
    let mut cmd = project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg(template)
        .arg("--license")
        .arg("MIT");

    if app {
        cmd = cmd.arg("--app");
    }

    cmd.run().success();
}

#[test]
fn test_new_empty_project_non_interactive() {
    let project = ProjectBuilder::new("new-empty")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Project name: test-project")
        .assert_contains("Template: empty")
        .assert_contains("License: MIT");

    let acton_toml = project.path().join("foobar/Acton.toml");
    assert!(acton_toml.exists());

    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "test-project""#));
    assert!(content.contains(r#"description = "test description""#));
    assert!(content.contains(r#"license = "MIT""#));

    assert!(project.path().join("foobar/contracts").exists());
    assert!(project.path().join("foobar/tests").exists());
    assert!(project.path().join("foobar/LICENSE").exists());
    assert!(project.path().join("foobar/.gitignore").exists());
    assert!(project.path().join("foobar/.editorconfig").exists());
    assert!(!project.path().join("foobar/AGENTS.md").exists());
}

#[test]
fn test_new_counter_project_non_interactive() {
    let project = ProjectBuilder::new("new-counter")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("counter-project")
        .arg("--description")
        .arg("counter description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("Apache-2.0")
        .run()
        .success();

    output.assert_contains("Template: counter");

    let acton_toml = project.path().join("foobar/Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "counter-project""#));

    assert!(
        project
            .path()
            .join("foobar/contracts/counter.tolk")
            .exists()
    );
    assert!(!project.path().join("foobar/package.json").exists());
    assert!(!project.path().join("foobar/app").exists());
    assert!(content.contains(r"[contracts.counter]"));
}

#[test]
fn test_new_counter_project_with_app_flag() {
    let project = ProjectBuilder::new("new-counter-app")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("Counter App Project")
        .arg("--description")
        .arg("counter description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_with_app_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/test_new_counter_project_with_app_flag.acton.toml.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/test_new_counter_project_with_app_flag.package.json.gen",
        );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("app/src/App.tsx").exists());
    assert!(project_dir.join("wrappers/Counter.ts").exists());
    assert!(project_dir.join("contracts/src/counter.tolk").exists());
    assert!(
        project_dir
            .join("contracts/tests/counter.test.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/tests/wrappers/Counter.tolk")
            .exists()
    );
    assert!(project_dir.join(".prettierrc").exists());
}

#[test]
fn test_new_empty_project_with_hooks_flag() {
    let project = ProjectBuilder::new("new-empty-hooks")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("hooks-project")
        .arg("--description")
        .arg("hooks description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--hooks")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Template: empty")
        .assert_contains("Git hooks: installed")
        .assert_file_snapshot_matches(
            "foobar/.githooks/pre-commit",
            "integration/snapshots/hooks/test_hooks_new_default.pre-commit.txt",
        );

    let project_dir = project.path().join("foobar");
    assert_eq!(
        git_config_get(&project_dir, "core.hooksPath").as_deref(),
        Some(".githooks")
    );
}

#[test]
fn test_new_empty_project_with_agents_flag() {
    let project = ProjectBuilder::new("new-empty-agents")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("agents-project")
        .arg("--description")
        .arg("agents description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--agents")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Template: empty")
        .assert_contains("AGENTS.md: included")
        .assert_file_snapshot_matches(
            "foobar/AGENTS.md",
            "integration/snapshots/test_new_empty_project_with_agents_flag.agents.md.gen",
        );

    assert!(project.path().join("foobar/AGENTS.md").exists());
}

#[test]
fn test_new_counter_project_rejects_app_value_syntax() {
    let project = ProjectBuilder::new("new-counter-app-false")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("counter-project")
        .arg("--description")
        .arg("counter description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .arg("--app=false")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_counter_project_rejects_app_value_syntax.stderr.txt",
        );
}

#[test]
fn test_new_project_rejects_hooks_value_syntax() {
    let project = ProjectBuilder::new("new-hooks-false")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("hooks-project")
        .arg("--description")
        .arg("hooks description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--hooks=false")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_project_rejects_hooks_value_syntax.stderr.txt",
        );
}

#[test]
fn test_new_project_rejects_agents_value_syntax() {
    let project = ProjectBuilder::new("new-agents-false")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("agents-project")
        .arg("--description")
        .arg("agents description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--agents=false")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_project_rejects_agents_value_syntax.stderr.txt",
        );
}

#[test]
fn test_new_empty_project_rejects_app_flag() {
    let project = ProjectBuilder::new("new-empty-app-unsupported")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_empty_project_rejects_app_flag.stderr.txt",
        );
}

#[test]
fn test_new_hooks_flag_requires_git() {
    let project = ProjectBuilder::new("new-hooks-requires-git")
        .without_acton_toml()
        .build();

    project
        .acton()
        .env("PATH", "")
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("hooks-project")
        .arg("--description")
        .arg("hooks description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--hooks")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_hooks_flag_requires_git.stderr.txt",
        );
}

#[test]
fn test_new_invalid_template() {
    let project = ProjectBuilder::new("new-invalid-template")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--template")
        .arg("unknown-template")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_invalid_template.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn test_new_empty_project_prompts_for_hooks() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-empty-hooks-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-hooks")
        .arg("--description")
        .arg("interactive hooks description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Install the default Git hooks?");
    session.send_line("y", "failed to confirm Git hooks");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-hooks");
    session.expect("Description: interactive hooks description");
    session.expect("Template: empty");
    session.expect("Git hooks: installed");
    session.expect("License: MIT");
    session.expect("Created Acton.toml with project configuration");
    session.expect("acton build");
    session.expect("acton test");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/.githooks/pre-commit",
        "integration/snapshots/hooks/test_hooks_new_default.pre-commit.txt",
    );

    let project_dir = project.path().join("foobar");
    assert_eq!(
        git_config_get(&project_dir, "core.hooksPath").as_deref(),
        Some(".githooks")
    );
    assert!(!project_dir.join("AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_empty_project_full_interactive_flow_without_flags() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-empty-full-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Project name:");
    session.send_line("interactive-empty", "failed to enter project name");
    session.expect("Description:");
    session.send_line(
        "interactive empty description",
        "failed to enter project description",
    );
    session.expect("Template:");
    session.send_line("", "failed to accept default template");
    session.expect("Install the default Git hooks?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("License:");
    session.send_line("", "failed to accept default license");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-empty");
    session.expect("Description: interactive empty description");
    session.expect("Template: empty");
    session.expect("License: MIT");
    session.expect("acton build");
    session.expect("acton test");
    session.expect(Eof);

    let project_dir = project.path().join("foobar");
    let acton_toml = fs::read_to_string(project_dir.join("Acton.toml")).unwrap();
    assert!(acton_toml.contains(r#"name = "interactive-empty""#));
    assert!(acton_toml.contains(r#"description = "interactive empty description""#));
    assert!(acton_toml.contains(r#"license = "MIT""#));
    assert!(project_dir.join("LICENSE").exists());
}

#[cfg(unix)]
#[test]
fn test_new_empty_project_interactive_prompts_accept_default_name_and_description() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-empty-default-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Project name:");
    session.send_line("", "failed to accept default project name");
    session.expect("Description:");
    session.send_line("", "failed to accept default description");
    session.expect("Template:");
    session.send_line("", "failed to accept default template");
    session.expect("Install the default Git hooks?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("License:");
    session.send_line("", "failed to accept default license");
    session.expect("Created new Acton project");
    session.expect("Project name: foobar");
    session.expect("Description: A TON blockchain project");
    session.expect("Template: empty");
    session.expect("License: MIT");
    session.expect(Eof);

    let project_dir = project.path().join("foobar");
    let acton_toml = fs::read_to_string(project_dir.join("Acton.toml")).unwrap();
    assert!(acton_toml.contains(r#"name = "foobar""#));
    assert!(acton_toml.contains(r#"description = "A TON blockchain project""#));
    assert!(acton_toml.contains(r#"license = "MIT""#));
}

#[cfg(unix)]
#[test]
fn test_new_counter_project_can_be_selected_interactively() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-counter-template-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-selected-counter")
        .arg("--description")
        .arg("interactive selected counter description")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Template:");
    session
        .send("\u{1b}[B")
        .expect("failed to navigate to counter template");
    session.send_line("", "failed to select counter template");
    session.expect("Include the TypeScript app scaffold?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Install the default Git hooks?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-selected-counter");
    session.expect("Description: interactive selected counter description");
    session.expect("Template: counter");
    session.expect("License: MIT");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/test_new_counter_project_can_be_selected_interactively.acton.toml.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("contracts/counter.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
    assert!(!project_dir.join("AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_counter_project_prompts_for_app_when_supported() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-counter-app-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-counter")
        .arg("--description")
        .arg("interactive description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include the TypeScript app scaffold?");
    session.send_line("y", "failed to confirm TypeScript app scaffold");
    session.expect("Install the default Git hooks?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-counter");
    session.expect("Description: interactive description");
    session.expect("Template: counter");
    session.expect("TypeScript app: included");
    session.expect("License: MIT");
    session.expect("Created Acton.toml with project configuration");
    session.expect("acton build");
    session.expect("npm ci");
    session.expect("npm run dev");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/test_new_counter_project_prompts_for_app_when_supported.acton.toml.gen",
    );
    session.assert_file_snapshot_matches(
        "foobar/package.json",
        "integration/snapshots/test_new_counter_project_prompts_for_app_when_supported.package.json.gen",
    );

    assert!(project.path().join("foobar/package.json").exists());
    assert!(project.path().join("foobar/app/src/App.tsx").exists());
    assert!(!project.path().join("foobar/AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_counter_project_interactive_decline_keeps_standard_layout() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-counter-app-interactive-decline")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-counter")
        .arg("--description")
        .arg("interactive description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include the TypeScript app scaffold?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Install the default Git hooks?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-counter");
    session.expect("Description: interactive description");
    session.expect("Template: counter");
    session.expect("License: MIT");
    session.expect("Created Acton.toml with project configuration");
    session.expect("acton build");
    session.expect("acton test");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/test_new_counter_project_interactive_decline_keeps_standard_layout.acton.toml.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("contracts/counter.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
    assert!(!project_dir.join("AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_empty_project_prompts_for_agents() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-empty-agents-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-agents")
        .arg("--description")
        .arg("interactive agents description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Install the default Git hooks?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("y", "failed to confirm AGENTS.md guidance");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-agents");
    session.expect("Description: interactive agents description");
    session.expect("Template: empty");
    session.expect("AGENTS.md: included");
    session.expect("License: MIT");
    session.expect("Created Acton.toml with project configuration");
    session.expect("acton build");
    session.expect("acton test");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/AGENTS.md",
        "integration/snapshots/test_new_empty_project_with_agents_flag.agents.md.gen",
    );
}

#[cfg(unix)]
#[test]
fn test_new_empty_project_accepts_other_license_interactively() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-empty-other-license")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .env("PATH", "")
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("other-license-project")
        .arg("--description")
        .arg("other license description")
        .arg("--template")
        .arg("empty")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
    session.expect("License:");
    for _ in 0..6 {
        session
            .send("\u{1b}[B")
            .expect("failed to navigate to Other license option");
    }
    session.send_line("", "failed to select Other license option");
    session.expect("Enter license:");
    session.send_line("Custom-Proprietary", "failed to enter custom license");
    session.expect("Created new Acton project");
    session.expect("Project name: other-license-project");
    session.expect("Description: other license description");
    session.expect("Template: empty");
    session.expect("License: Custom-Proprietary");
    session.expect(Eof);

    let project_dir = project.path().join("foobar");
    let acton_toml = fs::read_to_string(project_dir.join("Acton.toml")).unwrap();
    assert!(acton_toml.contains(r#"license = "Custom-Proprietary""#));
    assert!(!project_dir.join("LICENSE").exists());
}

#[cfg(unix)]
#[test]
fn test_new_counter_app_project_supports_npm_scripts() {
    if !is_npm_available() {
        eprintln!("Skipping real npm integration test: npm is not available in PATH");
        return;
    }

    let project = ProjectBuilder::new("new-counter-app-npm")
        .without_acton_toml()
        .build();

    let project_dir = project.path().join("foobar");

    let output = project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("counter-app-project")
        .arg("--description")
        .arg("counter description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_app_project_supports_npm_scripts.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/test_new_counter_app_project_supports_npm_scripts.acton.toml.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/test_new_counter_app_project_supports_npm_scripts.package.json.gen",
        );

    let package_lock = fs::read_to_string(project_dir.join("package-lock.json")).unwrap();
    let package_lock_json: JsonValue = serde_json::from_str(&package_lock).unwrap();
    assert!(package_lock.contains(r#""name": "counter-app-project""#));
    assert!(!package_lock.contains("counter-dapp"));
    assert!(
        !package_lock_json["packages"]
            .as_object()
            .unwrap()
            .contains_key("app")
    );

    let (path_env, cache_dir) = setup_real_npm_toolchain(&project_dir);

    let install_output = run_npm_command(&project_dir, &path_env, &cache_dir, &["ci"]);
    if !install_output.status.success() && npm_failure_looks_environment_specific(&install_output) {
        eprintln!(
            "Skipping real npm integration test due to environment-specific npm failure:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&install_output.stdout),
            String::from_utf8_lossy(&install_output.stderr)
        );
        return;
    }
    assert!(
        install_output.status.success(),
        "npm ci failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&install_output.stdout),
        String::from_utf8_lossy(&install_output.stderr)
    );

    let build_output = run_npm_command(&project_dir, &path_env, &cache_dir, &["run", "build"]);
    assert!(
        build_output.status.success(),
        "npm run build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let test_output = run_npm_command(&project_dir, &path_env, &cache_dir, &["run", "test"]);
    assert!(
        test_output.status.success(),
        "npm run test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&test_output.stdout),
        String::from_utf8_lossy(&test_output.stderr)
    );

    let typecheck_output =
        run_npm_command(&project_dir, &path_env, &cache_dir, &["run", "typecheck"]);
    assert!(
        typecheck_output.status.success(),
        "npm run typecheck failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&typecheck_output.stdout),
        String::from_utf8_lossy(&typecheck_output.stderr)
    );

    let fmt_output = run_npm_command(&project_dir, &path_env, &cache_dir, &["run", "fmt:check"]);
    assert!(
        fmt_output.status.success(),
        "npm run fmt:check failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&fmt_output.stdout),
        String::from_utf8_lossy(&fmt_output.stderr)
    );

    project
        .acton()
        .script("contracts/scripts/deploy.tolk")
        .current_dir(&project_dir)
        .run()
        .success();

    project
        .acton()
        .check()
        .current_dir(&project_dir)
        .run()
        .success();

    project
        .acton()
        .fmt()
        .arg("--check")
        .current_dir(&project_dir)
        .run()
        .success();

    assert!(project_dir.join("build/counter.json").exists());
    assert!(project_dir.join("dist/index.html").exists());
}

#[test]
fn test_new_empty_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-empty-localnet-deploy",
        "empty",
        false,
        "deployer",
        "scripts/deploy.tolk",
        "integration/snapshots/test_new_empty_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_counter_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-counter-localnet-deploy",
        "counter",
        false,
        "deployer",
        "scripts/deploy.tolk",
        "integration/snapshots/test_new_counter_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_counter_app_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-counter-app-localnet-deploy",
        "counter",
        true,
        "deployer",
        "contracts/scripts/deploy.tolk",
        "integration/snapshots/test_new_counter_app_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_jetton_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-jetton-localnet-deploy",
        "jetton",
        false,
        "deployer",
        "scripts/deploy.tolk",
        "integration/snapshots/test_new_jetton_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_empty_project_in_existed_directory() {
    let project = ProjectBuilder::new("foobar")
        .contract("foo", "")
        .without_acton_toml()
        .build();

    let dir = project.path().parent().expect("Should be parent directory");
    let log_dir = project.path().join(".acton/logs").display().to_string();

    let output = project
        .acton()
        .env("ACTON_LOG_DIR", &log_dir)
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_new_empty_project_in_existed_directory.stderr.txt",
    );
}

#[test]
fn test_new_empty_project_in_existed_directory_with_acton_toml() {
    let project = ProjectBuilder::new("foobar").contract("foo", "").build();

    let dir = project.path().parent().expect("Should be parent directory");
    let log_dir = project.path().join(".acton/logs").display().to_string();

    let output = project
        .acton()
        .env("ACTON_LOG_DIR", &log_dir)
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure();

    output
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_empty_project_in_existed_directory_with_acton_toml.stderr.txt",
        );
}

#[test]
fn test_new_empty_project_in_current_directory() {
    let project = ProjectBuilder::new("new-current-directory")
        .without_acton_toml()
        .build();
    let current_dir = project.path().join("current-dir-project");
    fs::create_dir_all(&current_dir).unwrap();

    let output = project
        .acton()
        .current_dir(&current_dir)
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("dot-project")
        .arg("--description")
        .arg("dot description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Project name: dot-project")
        .assert_contains("Description: dot description")
        .assert_contains("Template: empty")
        .assert_contains("License: MIT");

    let acton_toml = current_dir.join("Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "dot-project""#));
    assert!(content.contains(r#"description = "dot description""#));
    assert!(content.contains(r#"license = "MIT""#));
    assert!(current_dir.join("contracts").exists());
    assert!(current_dir.join("tests").exists());
    assert!(current_dir.join("LICENSE").exists());
    assert!(current_dir.join(".git").exists());
}

#[test]
fn test_new_empty_project_in_non_empty_current_directory() {
    let project = ProjectBuilder::new("new-non-empty-current-directory")
        .without_acton_toml()
        .build();
    let current_dir = project.path().join("non-empty-current-dir-project");
    fs::create_dir_all(&current_dir).unwrap();

    let existing_file = current_dir.join("notes.txt");
    fs::write(&existing_file, "keep me").unwrap();

    let output = project
        .acton()
        .current_dir(&current_dir)
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("non-empty-dot-project")
        .arg("--description")
        .arg("non empty dot description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_in_non_empty_current_directory.stdout.txt",
        )
        .assert_contains("Project name: non-empty-dot-project");

    let acton_toml = current_dir.join("Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "non-empty-dot-project""#));
    assert!(content.contains(r#"description = "non empty dot description""#));
    assert!(content.contains(r#"license = "MIT""#));
    assert_eq!(fs::read_to_string(&existing_file).unwrap(), "keep me");
    assert!(current_dir.join("contracts").exists());
    assert!(current_dir.join("tests").exists());
    assert!(current_dir.join("LICENSE").exists());
    assert!(current_dir.join(".git").exists());
}

#[cfg(unix)]
#[test]
fn test_new_project_leaves_partial_scaffold_when_git_add_fails() {
    let project = ProjectBuilder::new("new-git-stage-failure")
        .without_acton_toml()
        .build();
    let fake_path = setup_fake_git_stage_failure_toolchain(project.path());
    let project_dir = project.path().join("partial-project");

    project
        .acton()
        .env("PATH", &fake_path)
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("partial-project")
        .arg("--description")
        .arg("partial description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure()
        .assert_stderr_contains("Failed to add files to git repository");

    let acton_toml = project_dir.join("Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "partial-project""#));
    assert!(content.contains(r#"description = "partial description""#));
    assert!(content.contains(r#"license = "MIT""#));
    assert!(project_dir.join("contracts").exists());
    assert!(project_dir.join("tests").exists());
    assert!(project_dir.join(".gitignore").exists());
    assert!(project_dir.join(".env").exists());
    assert!(project_dir.join(".editorconfig").exists());
    assert!(project_dir.join(".git").exists());
}

#[cfg(unix)]
#[test]
fn test_new_project_fails_when_git_init_fails() {
    let project = ProjectBuilder::new("new-git-init-failure")
        .without_acton_toml()
        .build();
    let fake_path = setup_fake_git_init_failure_toolchain(project.path());
    let project_dir = project.path().join("foobar");
    let log_dir = project.path().join(".logs");

    let output = project
        .acton()
        .env("PATH", &fake_path)
        .env("ACTON_LOG_DIR", log_dir.to_str().unwrap())
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("git-init-failure-project")
        .arg("--description")
        .arg("git init failure description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure();

    output
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_project_fails_when_git_init_fails.stderr.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/test_new_project_fails_when_git_init_fails.acton.toml.gen",
        );
    assert!(project_dir.join("contracts").exists());
    assert!(project_dir.join("tests").exists());
    assert!(project_dir.join(".gitignore").exists());
    assert!(project_dir.join(".env").exists());
    assert!(project_dir.join(".editorconfig").exists());
    assert!(!project_dir.join(".git").exists());
}

#[test]
fn test_new_project_with_git_initialization() {
    let project = ProjectBuilder::new("new-git").without_acton_toml().build();

    project
        .acton()
        .arg("new")
        .arg(
            &project
                .path()
                .join("test-git-project")
                .display()
                .to_string(),
        )
        .arg("--name")
        .arg("git-test-project")
        .arg("--description")
        .arg("git test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_contains("Created new Acton project");

    let project_dir = project.path().join("test-git-project");
    assert!(project_dir.join(".git").exists());
}

#[test]
fn test_new_project_warns_when_git_is_unavailable_but_still_succeeds() {
    let project = ProjectBuilder::new("new-without-git")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .env("PATH", "")
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("no-git-project")
        .arg("--description")
        .arg("no git description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_new_project_warns_when_git_is_unavailable_but_still_succeeds.stdout.txt",
    );

    let project_dir = project.path().join("foobar");
    assert!(!project_dir.join(".git").exists());
    assert!(project_dir.join("Acton.toml").exists());
}

#[test]
fn test_new_project_symlinks_global_libraries() {
    let project = ProjectBuilder::new("new-symlink-libraries")
        .without_acton_toml()
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_libraries_dir = home_path.join(".config").join("acton").join("libraries");
    fs::create_dir_all(&global_libraries_dir).unwrap();
    let global_config = global_libraries_dir.join("global.libraries.toml");
    fs::write(
        &global_config,
        "[libraries.demo]\nhash = \"abcd\"\ncode = \"te6ccgEBAQEAAgAAAA==\"\n",
    )
    .unwrap();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .arg("new")
        .arg(&project.path().join("my-project").display().to_string())
        .arg("--name")
        .arg("symlink-project")
        .arg("--description")
        .arg("test")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    let symlink = project
        .path()
        .join("my-project")
        .join("global.libraries.toml");
    assert!(symlink.exists());
}

#[test]
fn test_new_project_uses_acton_user_when_git_user_name_is_missing() {
    let project = ProjectBuilder::new("new-git-no-user")
        .without_acton_toml()
        .build();
    let fake_path = setup_fake_git_without_user_name_toolchain(project.path());
    let project_dir = project.path().join("foobar");
    let current_year = chrono::Local::now().format("%Y").to_string();

    project
        .acton()
        .env("PATH", &fake_path)
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("fallback-author-project")
        .arg("--description")
        .arg("fallback author description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    let license = fs::read_to_string(project_dir.join("LICENSE")).unwrap();
    assert!(license.contains("MIT License"));
    assert!(license.contains(&format!("Copyright (c) {current_year} Acton User")));
}

#[test]
fn test_new_project_symlinks_global_wallets() {
    let project = ProjectBuilder::new("new-symlink")
        .without_acton_toml()
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_wallets_dir = home_path.join(".config").join("acton").join("wallets");
    fs::create_dir_all(&global_wallets_dir).unwrap();
    let global_config = global_wallets_dir.join("global.wallets.toml");
    fs::write(
        &global_config,
        "[wallets.global]\nkind=\"v5r1\"\nkeys={mnemonic=\"word1\"}",
    )
    .unwrap();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .arg("new")
        .arg(&project.path().join("my-project").display().to_string())
        .arg("--name")
        .arg("symlink-project")
        .arg("--description")
        .arg("test")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    let symlink = project
        .path()
        .join("my-project")
        .join("global.wallets.toml");
    assert!(symlink.exists());
}

#[test]
fn test_new_empty_project_full_flow() {
    let project = ProjectBuilder::new("new-empty-full")
        .without_acton_toml()
        .build();

    let dir = project.path();
    let project_dir = project.path().join("foobar");

    // 1. Create project
    project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_check.stderr.txt",
        );

    // 6. Run formatter
    project
        .acton()
        .current_dir(&project_dir)
        .fmt()
        .arg("--check")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_fmt.stdout.txt",
        );
}

#[test]
fn test_new_counter_project_full_flow() {
    let project = ProjectBuilder::new("new-counter-full")
        .without_acton_toml()
        .build();

    let dir = project.path();
    let project_dir = project.path().join("foobar");

    // 1. Create project
    project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_check.stderr.txt",
        );

    // 6. Run formatter
    project
        .acton()
        .current_dir(&project_dir)
        .fmt()
        .arg("--check")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_fmt.stdout.txt",
        );
}

#[test]
fn test_new_jetton_project_full_flow() {
    let project = ProjectBuilder::new("new-jetton-full")
        .without_acton_toml()
        .build();

    let dir = project.path();
    let project_dir = project.path().join("foobar");

    // 1. Create project
    project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("jetton")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_check.stderr.txt",
        );

    // 6. Run formatter
    project
        .acton()
        .current_dir(&project_dir)
        .fmt()
        .arg("--check")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_fmt.stdout.txt",
        );
}

#[test]
fn test_new_empty_project_with_dot_env() {
    let project = ProjectBuilder::new("new-dot-env")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Project name: test-project")
        .assert_contains("Template: empty")
        .assert_contains("License: MIT");

    let acton_toml = project.path().join("foobar/Acton.toml");
    assert!(acton_toml.exists());

    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "test-project""#));
    assert!(content.contains(r#"description = "test description""#));
    assert!(content.contains(r#"license = "MIT""#));

    assert!(project.path().join("foobar/contracts").exists());
    assert!(project.path().join("foobar/tests").exists());
    assert!(project.path().join("foobar/LICENSE").exists());
    assert!(project.path().join("foobar/.gitignore").exists());
    assert!(project.path().join("foobar/.env").exists());
    assert!(project.path().join("foobar/.editorconfig").exists());
}

#[test]
fn test_new_empty_project_writes_editorconfig_with_tolk_rules() {
    let project = ProjectBuilder::new("new-editorconfig")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_file_snapshot_matches(
            "foobar/.editorconfig",
            "integration/snapshots/test_new_empty_project_editorconfig.gen",
        );
}
