use crate::common::acton_exe;
use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use serde_json::Value as JsonValue;
use std::env;
use std::fs;
use std::path::Path;
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
fn setup_real_npm_toolchain(project_root: &Path, cache_dir: &Path) -> String {
    let bin_dir = project_root.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::create_dir_all(cache_dir).unwrap();

    let acton_path = bin_dir.join("acton");

    fs::write(&acton_path, ACTON_SHIM).unwrap();
    make_executable(&acton_path);

    format!(
        "{}:{}",
        bin_dir.display(),
        env::var("PATH").unwrap_or_default()
    )
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
        .env("NPM_CONFIG_FETCH_RETRIES", "3")
        .env("NPM_CONFIG_FETCH_RETRY_MINTIMEOUT", "5000")
        .env("NPM_CONFIG_FETCH_RETRY_MAXTIMEOUT", "30000")
        .env("NPM_CONFIG_FETCH_TIMEOUT", "60000")
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
        "EPIPE",
        "EHOSTUNREACH",
        "ENETUNREACH",
        "ENETDOWN",
        "fetch failed",
        "getaddrinfo",
        "network request",
        "network timeout",
        "socket hang up",
        "Bad response from registry",
        "503 Service Unavailable",
        "502 Bad Gateway",
        "504 Gateway",
        "429 Too Many Requests",
        "Failed to execute `npx",
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
    use std::fmt::Write as _;

    let acton_toml_path = project_dir.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("Failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.localnet]
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
"#
    );
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
        .localnet()
        .current_dir(&project_dir)
        .args(["--accounts", wallet_name])
        .start();
    append_localnet_network(&project_dir, &node.base_url());

    project
        .acton()
        .script(deploy_script_path)
        .current_dir(&project_dir)
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
    assert!(content.contains(&format!(
        "[toolchain]\nacton = \"{}\"",
        acton::build_info::PACKAGE_VERSION
    )));
    assert!(!content.contains("-trunk"));
    assert!(content.contains("Check full Acton.toml reference and all available keys"));
    assert!(content.contains("https://ton-blockchain.github.io/acton/docs/acton-toml"));

    assert!(project.path().join("foobar/contracts").exists());
    assert!(project.path().join("foobar/tests").exists());
    assert!(project.path().join("foobar/LICENSE").exists());
    assert!(project.path().join("foobar/.gitignore").exists());
    assert!(project.path().join("foobar/.editorconfig").exists());
    assert!(!project.path().join("foobar/AGENTS.md").exists());
}

#[test]
fn test_new_empty_project_next_steps_quote_path_with_spaces() {
    let project = ProjectBuilder::new("new-path-with-spaces")
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("project with spaces");

    project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("spaces-project")
        .arg("--description")
        .arg("spaces description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_empty_project_next_steps_quote_path_with_spaces.stdout.txt",
        );

    project
        .acton()
        .build()
        .current_dir(&project_dir)
        .run()
        .success();
    project
        .acton()
        .test()
        .current_dir(&project_dir)
        .run()
        .success();
}

#[test]
fn test_new_empty_project_next_steps_escape_single_quote_in_path() {
    let project = ProjectBuilder::new("new-path-with-single-quote")
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("project with 'quote");

    let output = project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg("quote-project")
        .arg("--description")
        .arg("quote description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    let stdout = output.get_stdout();
    assert!(
        stdout.contains("cd '") && stdout.contains("project with '\\''quote'"),
        "expected next-step cd command to POSIX-escape the single quote, got:\n{stdout}"
    );
}

#[test]
fn test_new_existing_directory_hint_quotes_path_with_spaces() {
    let project = ProjectBuilder::new("new-existing-dir-path-with-spaces")
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("existing project with spaces");
    fs::create_dir_all(&project_dir).unwrap();

    project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_existing_directory_hint_quotes_path_with_spaces.stderr.txt",
        );
}

#[test]
fn test_new_existing_directory_hint_escapes_single_quote_in_path() {
    let project = ProjectBuilder::new("new-existing-dir-path-with-quote")
        .without_acton_toml()
        .build();
    let project_dir = project.path().join("existing project with 'quote");
    fs::create_dir_all(&project_dir).unwrap();

    let output = project
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .run()
        .failure();

    let stderr = output.get_stderr();
    assert!(
        stderr.contains("cd '") && stderr.contains("existing project with '\\''quote'"),
        "expected existing-directory cd hint to POSIX-escape the single quote, got:\n{stderr}"
    );
}

#[test]
fn test_new_project_non_interactive_requires_template() {
    let project = ProjectBuilder::new("new-non-interactive-requires-template")
        .without_acton_toml()
        .build();

    let target_dir = project.path().join("foobar");

    let output = project
        .acton()
        .arg("--color")
        .arg("always")
        .arg("new")
        .arg(&target_dir.display().to_string())
        .run()
        .failure();

    output
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_project_non_interactive_requires_template.stderr.txt",
        )
        .assert_stderr_svg_snapshot_matches(
            "integration/snapshots/new/test_new_project_non_interactive_requires_template.stderr.svg",
        );

    assert!(
        !target_dir.exists(),
        "new should not create the target directory before required non-interactive arguments are valid"
    );
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
            .join("foobar/contracts/Counter.tolk")
            .exists()
    );
    assert!(!project.path().join("foobar/package.json").exists());
    assert!(!project.path().join("foobar/app").exists());
    assert!(content.contains(r"[contracts.Counter]"));
}

#[test]
fn test_new_jetton_project_non_interactive() {
    let project = ProjectBuilder::new("new-jetton")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("jetton-project")
        .arg("--description")
        .arg("jetton description")
        .arg("--template")
        .arg("jetton")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output.assert_contains("Template: jetton");

    let project_dir = project.path().join("foobar");
    let acton_toml = project_dir.join("Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "jetton-project""#));
    assert!(content.contains(r"[contracts.JettonMinter]"));
    assert!(content.contains(r"[contracts.JettonWallet]"));
    assert!(content.contains("acton script scripts/deploy.tolk"));
    assert!(content.contains("jetton-mint = \"acton script scripts/mint.tolk\""));
    assert!(content.contains("jetton-transfer = \"acton script scripts/transfer.tolk\""));
    assert!(content.contains("jetton-info = \"acton script scripts/info.tolk\""));
    assert!(content.contains("jetton-change-admin = \"acton script scripts/change-admin.tolk\""));
    assert!(
        content.contains("jetton-change-metadata = \"acton script scripts/change-metadata.tolk\"")
    );
    assert!(content.contains("jetton-claim-admin = \"acton script scripts/claim-admin.tolk\""));

    assert!(project_dir.join("contracts/JettonMinter.tolk").exists());
    assert!(project_dir.join("contracts/JettonWallet.tolk").exists());
    assert!(project_dir.join("contracts/storage.tolk").exists());
    assert!(project_dir.join("contracts/messages.tolk").exists());
    assert!(project_dir.join("contracts/errors.tolk").exists());
    assert!(project_dir.join("wrappers/JettonMinter.gen.tolk").exists());
    assert!(project_dir.join("wrappers/JettonWallet.gen.tolk").exists());
    assert!(project_dir.join("wrappers/utils.tolk").exists());
    assert!(project_dir.join("scripts/deploy.tolk").exists());
    assert!(project_dir.join("scripts/mint.tolk").exists());
    assert!(project_dir.join("scripts/transfer.tolk").exists());
    assert!(project_dir.join("scripts/info.tolk").exists());
    assert!(project_dir.join("scripts/change-admin.tolk").exists());
    assert!(project_dir.join("scripts/change-metadata.tolk").exists());
    assert!(project_dir.join("scripts/claim-admin.tolk").exists());
    assert!(project_dir.join("tests/test-utils.tolk").exists());
    assert!(project_dir.join("tests/state-init.test.tolk").exists());
    assert!(project_dir.join("tests/wallet-behavior.test.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
}

#[test]
fn test_new_nft_project_non_interactive() {
    let project = ProjectBuilder::new("new-nft").without_acton_toml().build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("nft-project")
        .arg("--description")
        .arg("nft description")
        .arg("--template")
        .arg("nft")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output.assert_contains("Template: nft");

    let project_dir = project.path().join("foobar");
    let acton_toml = project_dir.join("Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "nft-project""#));
    assert!(content.contains(r"[contracts.NftCollection]"));
    assert!(content.contains(r"[contracts.NftItem]"));
    assert!(content.contains("acton script scripts/deploy-collection.tolk"));
    assert!(content.contains("acton script scripts/deploy-collection.tolk --net testnet"));

    assert!(project_dir.join("contracts/NftCollection.tolk").exists());
    assert!(project_dir.join("contracts/NftItem.tolk").exists());
    assert!(project_dir.join("wrappers/NftCollection.gen.tolk").exists());
    assert!(project_dir.join("wrappers/NftItem.gen.tolk").exists());
    assert!(project_dir.join("wrappers/utils.tolk").exists());
    assert!(project_dir.join("scripts/deploy-collection.tolk").exists());
    assert!(project_dir.join("scripts/deploy-item.tolk").exists());
    assert!(project_dir.join("scripts/deploy-batch.tolk").exists());
    assert!(project_dir.join("scripts/transfer-item.tolk").exists());
    assert!(project_dir.join("scripts/change-admin.tolk").exists());
    assert!(project_dir.join("tests/nft-collection.test.tolk").exists());
    assert!(project_dir.join("tests/nft-item.test.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
}

#[test]
fn test_new_w5_extension_project_non_interactive() {
    let project = ProjectBuilder::new("new-w5-extension")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("w5-extension-project")
        .arg("--description")
        .arg("w5-extension description")
        .arg("--template")
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output.assert_contains("Template: w5-extension");

    let project_dir = project.path().join("foobar");
    let acton_toml = project_dir.join("Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "w5-extension-project""#));
    assert!(content.contains(r"[contracts.SimpleExtension]"));
    assert!(content.contains(r"[contracts.WalletV5]"));
    assert!(content.contains(r#"src = "contracts/walletv5/WalletV5.tolk""#));
    assert!(content.contains("acton script scripts/deploy.tolk"));
    assert!(content.contains("install-extension = \"acton script scripts/install-extension.tolk"));
    assert!(content.contains("delete-extension = \"acton script scripts/delete-extension.tolk"));

    assert!(project_dir.join("contracts/SimpleExtension.tolk").exists());
    assert!(project_dir.join("contracts/types.tolk").exists());
    assert!(project_dir.join("contracts/errors.tolk").exists());
    assert!(project_dir.join("contracts/w5-types.tolk").exists());
    assert!(
        project_dir
            .join("contracts/walletv5/WalletV5.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/walletv5/messages.tolk")
            .exists()
    );
    assert!(project_dir.join("contracts/walletv5/storage.tolk").exists());
    assert!(
        project_dir
            .join("wrappers/SimpleExtension.gen.tolk")
            .exists()
    );
    assert!(project_dir.join("wrappers/WalletV5.gen.tolk").exists());
    assert!(project_dir.join("wrappers/utils.tolk").exists());
    assert!(
        project_dir
            .join("tests/simple-extension.test.tolk")
            .exists()
    );
    assert!(project_dir.join("scripts/deploy.tolk").exists());
    assert!(project_dir.join("scripts/install-extension.tolk").exists());
    assert!(project_dir.join("scripts/delete-extension.tolk").exists());
    assert!(project_dir.join("scripts/utils/common.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
}

#[test]
fn test_new_w5_plugin_template_alias_still_works() {
    let project = ProjectBuilder::new("new-w5-plugin-alias")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("w5-extension-project")
        .arg("--description")
        .arg("w5 extension description")
        .arg("--template")
        .arg("w5-plugin")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output.assert_contains("Template: w5-extension");
    assert!(
        project
            .path()
            .join("foobar/contracts/SimpleExtension.tolk")
            .exists()
    );
}

#[test]
fn test_new_w5_extension_project_with_agents_flag() {
    let project = ProjectBuilder::new("new-w5-extension-agents")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("w5-extension-project")
        .arg("--description")
        .arg("w5-extension description")
        .arg("--template")
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .arg("--agents")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Template: w5-extension")
        .assert_contains("AGENTS.md: included")
        .assert_file_snapshot_matches(
            "foobar/AGENTS.md",
            "integration/snapshots/new/test_new_w5_extension_project_with_agents_flag.agents.md.gen",
        );

    assert!(project.path().join("foobar/AGENTS.md").exists());
}

#[test]
fn test_new_empty_project_with_app_flag() {
    let project = ProjectBuilder::new("new-empty-app")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("Empty App Project")
        .arg("--description")
        .arg("empty app description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_empty_project_with_app_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/new/test_new_empty_project_with_app_flag.acton.toml.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_empty_project_with_app_flag.package.json.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/README.md",
            "integration/snapshots/new/test_new_empty_project_with_app_flag.readme.md",
        )
        .assert_file_snapshot_matches(
            "foobar/.github/workflows/ci.yml",
            "integration/snapshots/new/test_new_empty_project_with_app_flag.ci.yml",
        );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("app/src/App.tsx").exists());
    assert!(project_dir.join("app/src/styles.css").exists());
    assert!(project_dir.join("components.json").exists());
    assert!(project_dir.join(".prettierignore").exists());
    assert!(project_dir.join("contracts/src/Empty.tolk").exists());
    assert!(
        project_dir
            .join("contracts/tests/contract.test.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/wrappers/Empty.gen.tolk")
            .exists()
    );
    assert!(project_dir.join("wrappers-ts/Empty.gen.ts").exists());
    assert!(!project_dir.join("app/src/app.css").exists());
}

#[test]
fn test_new_empty_app_project_with_agents_flag() {
    let project = ProjectBuilder::new("new-empty-app-agents")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("Empty App Project")
        .arg("--description")
        .arg("empty app description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .arg("--agents")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_empty_app_project_with_agents_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/AGENTS.md",
            "integration/snapshots/new/test_new_empty_app_project_with_agents_flag.agents.md.gen",
        );
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
            "integration/snapshots/new/test_new_counter_project_with_app_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/new/test_new_counter_project_with_app_flag.acton.toml.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_counter_project_with_app_flag.package.json.gen",
        );

    let project_dir = project.path().join("foobar");
    let package_lock = fs::read_to_string(project_dir.join("package-lock.json")).unwrap();
    assert!(package_lock.starts_with(
        r#"{
  "name": "counter-app-project",
  "version": "0.1.0",
  "lockfileVersion": 3,
  "requires": true,
  "packages": {
    "": {
      "name": "counter-app-project",
      "version": "0.1.0",
      "dependencies": {
"#
    ));
    assert!(!package_lock.contains(r#""name": "counter-project""#));
    assert!(project_dir.join("app/src/App.tsx").exists());
    assert!(project_dir.join("wrappers-ts/Counter.gen.ts").exists());
    assert!(!project_dir.join("wrappers-ts/Counter.ts").exists());
    assert!(project_dir.join("contracts/src/Counter.tolk").exists());
    assert!(
        project_dir
            .join("contracts/tests/counter.test.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/wrappers/Counter.gen.tolk")
            .exists()
    );
    assert!(project_dir.join(".prettierrc").exists());
}

#[test]
fn test_new_w5_extension_project_with_app_flag() {
    let project = ProjectBuilder::new("new-w5-extension-app")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("W5 Extension App Project")
        .arg("--description")
        .arg("w5-extension app description")
        .arg("--template")
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_w5_extension_project_with_app_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/new/test_new_w5_extension_project_with_app_flag.acton.toml.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_w5_extension_project_with_app_flag.package.json.gen",
        );

    let project_dir = project.path().join("foobar");
    let package_lock = fs::read_to_string(project_dir.join("package-lock.json")).unwrap();
    assert!(package_lock.contains(r#""name": "w5-extension-app-project""#));
    assert!(!package_lock.contains(r#""name": "simple-extension""#));
    assert!(project_dir.join("app/src/App.tsx").exists());
    assert!(
        project_dir
            .join("wrappers-ts/SimpleExtension.gen.ts")
            .exists()
    );
    assert!(project_dir.join("wrappers-ts/WalletV5.gen.ts").exists());
    assert!(
        project_dir
            .join("contracts/src/SimpleExtension.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/src/walletv5/WalletV5.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/tests/simple-extension.test.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/wrappers/SimpleExtension.gen.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/wrappers/WalletV5.gen.tolk")
            .exists()
    );
    assert!(project_dir.join("contracts/wrappers/utils.tolk").exists());
    assert!(
        project_dir
            .join("contracts/scripts/install-extension.tolk")
            .exists()
    );
    assert!(
        project_dir
            .join("contracts/scripts/utils/common.tolk")
            .exists()
    );
    assert!(project_dir.join(".prettierrc").exists());
}

#[test]
fn test_new_w5_extension_app_project_with_agents_flag() {
    let project = ProjectBuilder::new("new-w5-extension-app-agents")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("W5 Extension App Project")
        .arg("--description")
        .arg("w5-extension app description")
        .arg("--template")
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .arg("--agents")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_w5_extension_app_project_with_agents_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_w5_extension_app_project_with_agents_flag.package.json.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/AGENTS.md",
            "integration/snapshots/new/test_new_w5_extension_app_project_with_agents_flag.agents.md.gen",
        );
}

#[test]
fn test_new_jetton_app_project_with_agents_flag() {
    let project = ProjectBuilder::new("new-jetton-app-agents")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("Jetton App Project")
        .arg("--description")
        .arg("jetton description")
        .arg("--template")
        .arg("jetton")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .arg("--agents")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_jetton_app_project_with_agents_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_jetton_app_project_with_agents_flag.package.json.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/AGENTS.md",
            "integration/snapshots/new/test_new_jetton_app_project_with_agents_flag.agents.md.gen",
        );
}

#[test]
fn test_new_nft_app_project_with_agents_flag() {
    let project = ProjectBuilder::new("new-nft-app-agents")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("NFT App Project")
        .arg("--description")
        .arg("nft description")
        .arg("--template")
        .arg("nft")
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .arg("--agents")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_nft_app_project_with_agents_flag.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_nft_app_project_with_agents_flag.package.json.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/AGENTS.md",
            "integration/snapshots/new/test_new_nft_app_project_with_agents_flag.agents.md.gen",
        );
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
            "integration/snapshots/new/test_new_empty_project_with_agents_flag.agents.md.gen",
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
            "integration/snapshots/new/test_new_counter_project_rejects_app_value_syntax.stderr.txt",
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
            "integration/snapshots/new/test_new_project_rejects_hooks_value_syntax.stderr.txt",
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
            "integration/snapshots/new/test_new_project_rejects_agents_value_syntax.stderr.txt",
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
            "integration/snapshots/new/test_new_hooks_flag_requires_git.stderr.txt",
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
            "integration/snapshots/new/test_new_invalid_template.stderr.txt",
        );
}

#[test]
fn test_new_templates_flag_is_hidden_from_help() {
    let project = ProjectBuilder::new("new-templates-hidden-help")
        .without_acton_toml()
        .build();

    project
        .acton()
        .arg("new")
        .arg("--help")
        .run()
        .success()
        .assert_contains("--template")
        .assert_not_contains("--templates");
}

#[test]
fn test_new_templates_returns_machine_readable_json() {
    let project = ProjectBuilder::new("new-templates-json")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg("--templates")
        .run()
        .success();

    let json: JsonValue =
        serde_json::from_str(&output.get_stdout()).expect("new --templates must return valid JSON");

    assert_eq!(json["schema_version"], 1);
    output.assert_snapshot_matches(
        "integration/snapshots/new/test_new_templates_returns_machine_readable_json.stdout.txt",
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

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("y", "failed to open advanced options");
    session.expect("Set up Git hooks to run checks before each commit?");
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
    session.expect("Template:");
    session.send_line("", "failed to accept default template");
    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("y", "failed to open advanced options");
    session.expect("Description:");
    session.send_line(
        "interactive empty description",
        "failed to enter project description",
    );
    session.expect("License:");
    session.send_line("", "failed to accept default license");
    session.expect("Set up Git hooks to run checks before each commit?");
    session.send_line("", "failed to keep default no-hooks choice");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
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
    session.expect("Template:");
    session.send_line("", "failed to accept default template");
    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
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
    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-selected-counter");
    session.expect("Description: interactive selected counter description");
    session.expect("Template: counter");
    session.expect("License: MIT");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/new/test_new_counter_project_can_be_selected_interactively.acton.toml.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("contracts/Counter.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
    assert!(!project_dir.join("AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_w5_extension_project_can_be_selected_interactively() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-w5-extension-template-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-selected-w5-extension")
        .arg("--description")
        .arg("interactive selected w5 extension description")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Template:");
    for _ in 0..4 {
        session
            .send("\u{1b}[B")
            .expect("failed to navigate to w5-extension template");
    }
    session.send_line("", "failed to select w5-extension template");
    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-selected-w5-extension");
    session.expect("Description: interactive selected w5 extension description");
    session.expect("Template: w5-extension");
    session.expect("License: MIT");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/new/test_new_w5_extension_project_can_be_selected_interactively.acton.toml.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("contracts/SimpleExtension.tolk").exists());
    assert!(
        project_dir
            .join("contracts/walletv5/WalletV5.tolk")
            .exists()
    );
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

    session.expect("Include the TypeScript dApp?");
    session.send_line("y", "failed to confirm TypeScript app scaffold");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
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
        "integration/snapshots/new/test_new_counter_project_prompts_for_app_when_supported.acton.toml.gen",
    );
    session.assert_file_snapshot_matches(
        "foobar/package.json",
        "integration/snapshots/new/test_new_counter_project_prompts_for_app_when_supported.package.json.gen",
    );

    assert!(project.path().join("foobar/package.json").exists());
    assert!(project.path().join("foobar/app/src/App.tsx").exists());
    assert!(!project.path().join("foobar/AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_w5_extension_project_prompts_for_app_when_supported() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-w5-extension-app-interactive")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-w5-extension")
        .arg("--description")
        .arg("interactive w5 extension description")
        .arg("--template")
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include the TypeScript dApp?");
    session.send_line("y", "failed to confirm TypeScript app scaffold");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-w5-extension");
    session.expect("Description: interactive w5 extension description");
    session.expect("Template: w5-extension");
    session.expect("TypeScript app: included");
    session.expect("License: MIT");
    session.expect("Created Acton.toml with project configuration");
    session.expect("acton build");
    session.expect("npm ci");
    session.expect("npm run dev");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/new/test_new_w5_extension_project_prompts_for_app_when_supported.acton.toml.gen",
    );
    session.assert_file_snapshot_matches(
        "foobar/package.json",
        "integration/snapshots/new/test_new_w5_extension_project_prompts_for_app_when_supported.package.json.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("package.json").exists());
    assert!(project_dir.join("app/src/App.tsx").exists());
    assert!(
        project_dir
            .join("wrappers-ts/SimpleExtension.gen.ts")
            .exists()
    );
    assert!(project_dir.join("wrappers-ts/WalletV5.gen.ts").exists());
    assert!(!project_dir.join("AGENTS.md").exists());
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

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
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
        "integration/snapshots/new/test_new_counter_project_interactive_decline_keeps_standard_layout.acton.toml.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("contracts/Counter.tolk").exists());
    assert!(!project_dir.join("package.json").exists());
    assert!(!project_dir.join("app").exists());
    assert!(!project_dir.join("AGENTS.md").exists());
}

#[cfg(unix)]
#[test]
fn test_new_w5_extension_project_interactive_decline_keeps_standard_layout() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-w5-extension-app-interactive-decline")
        .without_acton_toml()
        .build();

    let mut session = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("interactive-w5-extension")
        .arg("--description")
        .arg("interactive w5 extension description")
        .arg("--template")
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Created new Acton project");
    session.expect("Project name: interactive-w5-extension");
    session.expect("Description: interactive w5 extension description");
    session.expect("Template: w5-extension");
    session.expect("License: MIT");
    session.expect("Created Acton.toml with project configuration");
    session.expect("acton build");
    session.expect("acton test");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        "foobar/Acton.toml",
        "integration/snapshots/new/test_new_w5_extension_project_interactive_decline_keeps_standard_layout.acton.toml.gen",
    );

    let project_dir = project.path().join("foobar");
    assert!(project_dir.join("contracts/SimpleExtension.tolk").exists());
    assert!(
        project_dir
            .join("contracts/walletv5/WalletV5.tolk")
            .exists()
    );
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

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("y", "failed to open advanced options");
    session.expect("Set up Git hooks to run checks before each commit?");
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
        "integration/snapshots/new/test_new_empty_project_with_agents_flag.agents.md.gen",
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

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("y", "failed to open advanced options");
    session.expect("License:");
    for _ in 0..6 {
        session
            .send("\u{1b}[B")
            .expect("failed to navigate to Other license option");
    }
    session.send_line("", "failed to select Other license option");
    session.expect("Enter license:");
    session.send_line("Custom-Proprietary", "failed to enter custom license");
    session.expect("Include AGENTS.md guidance for coding agents?");
    session.send_line("", "failed to keep default no-agents choice");
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
            "integration/snapshots/new/test_new_counter_app_project_supports_npm_scripts.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/new/test_new_counter_app_project_supports_npm_scripts.acton.toml.gen",
        )
        .assert_file_snapshot_matches(
            "foobar/package.json",
            "integration/snapshots/new/test_new_counter_app_project_supports_npm_scripts.package.json.gen",
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

    let cache_path = project_dir.join(".npm-cache");
    let cache_dir = cache_path.as_path();
    let path_env = setup_real_npm_toolchain(&project_dir, cache_dir);

    let install_output = run_npm_command(&project_dir, &path_env, cache_dir, &["ci"]);
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

    let build_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "build"]);
    assert!(
        build_output.status.success(),
        "npm run build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let test_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "test"]);
    assert!(
        test_output.status.success(),
        "npm run test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&test_output.stdout),
        String::from_utf8_lossy(&test_output.stderr)
    );

    let typecheck_output =
        run_npm_command(&project_dir, &path_env, cache_dir, &["run", "typecheck"]);
    assert!(
        typecheck_output.status.success(),
        "npm run typecheck failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&typecheck_output.stdout),
        String::from_utf8_lossy(&typecheck_output.stderr)
    );

    let fmt_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "fmt:check"]);
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

    assert!(project_dir.join("build/Counter.json").exists());
    assert!(project_dir.join("dist/index.html").exists());
}

#[cfg(unix)]
fn create_app_project(workspace: &Project, project_dir: &Path, template: &str) {
    let project_name = format!("{template}-app-project");
    workspace
        .acton()
        .arg("new")
        .arg(&project_dir.display().to_string())
        .arg("--name")
        .arg(&project_name)
        .arg("--description")
        .arg("app template check")
        .arg("--template")
        .arg(template)
        .arg("--license")
        .arg("MIT")
        .arg("--app")
        .run()
        .success();
}

#[cfg(unix)]
fn package_uses_eslint(package_json: &JsonValue) -> bool {
    let has_eslint_dependency = ["dependencies", "devDependencies"].iter().any(|section| {
        package_json
            .get(section)
            .and_then(JsonValue::as_object)
            .is_some_and(|deps| {
                deps.keys()
                    .any(|name| name == "eslint" || name.contains("eslint"))
            })
    });

    let has_eslint_script = package_json
        .get("scripts")
        .and_then(JsonValue::as_object)
        .is_some_and(|scripts| {
            scripts.values().any(|script| {
                script
                    .as_str()
                    .is_some_and(|script| script.contains("eslint"))
            })
        });

    has_eslint_dependency || has_eslint_script
}

#[cfg(unix)]
fn assert_app_template_npm_quality_checks(test_name: &str, template: &str, cache_dir: &Path) {
    if !is_npm_available() {
        eprintln!("Skipping npm app template checks: npm is not available in PATH");
        return;
    }

    let workspace = ProjectBuilder::new(test_name).without_acton_toml().build();
    let project_dir = workspace.path().join("generated");
    create_app_project(&workspace, &project_dir, template);

    let package_json: JsonValue =
        serde_json::from_str(&fs::read_to_string(project_dir.join("package.json")).unwrap())
            .unwrap();
    let scripts = package_json["scripts"].as_object().unwrap();
    assert!(
        scripts.contains_key("fmt:check"),
        "{template} app template must expose npm run fmt:check"
    );

    let path_env = setup_real_npm_toolchain(&project_dir, cache_dir);
    let install_output = run_npm_command(&project_dir, &path_env, cache_dir, &["ci"]);
    if !install_output.status.success() && npm_failure_looks_environment_specific(&install_output) {
        eprintln!(
            "Skipping npm app template checks for {template} due to environment-specific npm failure:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&install_output.stdout),
            String::from_utf8_lossy(&install_output.stderr)
        );
        return;
    }
    assert!(
        install_output.status.success(),
        "npm ci failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&install_output.stdout),
        String::from_utf8_lossy(&install_output.stderr)
    );

    assert!(
        scripts.contains_key("lint"),
        "{template} app template must expose npm run lint"
    );
    assert!(
        package_uses_eslint(&package_json),
        "{template} app template must depend on ESLint"
    );
    let lint_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "lint"]);
    assert!(
        lint_output.status.success(),
        "npm run lint failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&lint_output.stdout),
        String::from_utf8_lossy(&lint_output.stderr)
    );

    let build_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "build"]);
    assert!(
        build_output.status.success(),
        "npm run build failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    if template == "empty" {
        assert!(
            !scripts.contains_key("test"),
            "empty app template is also used by acton init --create-dapp and must not require an Acton project"
        );
    } else {
        assert!(
            scripts.contains_key("test"),
            "{template} app template must expose npm run test"
        );
        let test_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "test"]);
        assert!(
            test_output.status.success(),
            "npm run test failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&test_output.stdout),
            String::from_utf8_lossy(&test_output.stderr)
        );
    }

    let typecheck_output =
        run_npm_command(&project_dir, &path_env, cache_dir, &["run", "typecheck"]);
    assert!(
        typecheck_output.status.success(),
        "npm run typecheck failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&typecheck_output.stdout),
        String::from_utf8_lossy(&typecheck_output.stderr)
    );

    let fmt_output = run_npm_command(&project_dir, &path_env, cache_dir, &["run", "fmt:check"]);
    assert!(
        fmt_output.status.success(),
        "npm run fmt:check failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&fmt_output.stdout),
        String::from_utf8_lossy(&fmt_output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn test_new_app_templates_npm_quality_checks() {
    let cache_workspace = ProjectBuilder::new("new-app-templates-npm-cache")
        .without_acton_toml()
        .build();
    let cache_dir = cache_workspace.path().join("npm-cache");

    for template in ["empty", "counter", "jetton", "nft", "w5-extension"] {
        assert_app_template_npm_quality_checks(
            &format!("new-{template}-app-npm-quality-checks"),
            template,
            &cache_dir,
        );
    }
}

fn read_new_template_file(template: &str, relative_path: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/commands/new/templates")
            .join(template)
            .join(relative_path),
    )
    .unwrap_or_else(|e| panic!("Failed to read {template}/{relative_path}: {e}"))
}

fn read_new_template_package_json(template: &str) -> JsonValue {
    serde_json::from_str(&read_new_template_file(template, "package.json"))
        .unwrap_or_else(|e| panic!("Failed to parse {template}/package.json: {e}"))
}

fn assert_new_template_files_match_baseline(
    baseline_template: &str,
    candidate_templates: &[&str],
    relative_paths: &[&str],
    reason: &str,
) {
    let mut mismatches = Vec::new();

    for candidate_template in candidate_templates {
        for relative_path in relative_paths {
            if read_new_template_file(candidate_template, relative_path)
                != read_new_template_file(baseline_template, relative_path)
            {
                mismatches.push(format!(
                    "- {candidate_template}/{relative_path} differs from {baseline_template}/{relative_path}\n  inspect: diff -u src/commands/new/templates/{baseline_template}/{relative_path} src/commands/new/templates/{candidate_template}/{relative_path}"
                ));
            }
        }
    }

    if !mismatches.is_empty() {
        panic!(
            "App template shared file parity regression.\n\n{}\n\n{reason}",
            mismatches.join("\n")
        );
    }
}

#[test]
fn test_new_w5_extension_app_template_matches_contract_app_package_sections() {
    let baseline = read_new_template_package_json("counter-app");
    let w5_package = read_new_template_package_json("w5-extension-app");

    for section in ["scripts", "dependencies", "devDependencies"] {
        assert_eq!(
            w5_package[section], baseline[section],
            "w5-extension app template package.json `{section}` must match the common contract app template shape"
        );
    }
}

#[test]
fn test_new_w5_extension_app_template_matches_contract_app_tooling_files() {
    for relative_path in [
        ".github/workflows/ci.yml",
        ".prettierignore",
        ".prettierrc",
        "components.json",
        "eslint.config.js",
        "package-lock.json",
        "tsconfig.json",
        "vite.config.ts",
    ] {
        assert_eq!(
            read_new_template_file("w5-extension-app", relative_path),
            read_new_template_file("counter-app", relative_path),
            "w5-extension app template `{relative_path}` must match the common contract app template tooling file"
        );
    }
}

#[test]
fn test_new_app_template_common_styles_match_shared_baseline() {
    assert_new_template_files_match_baseline(
        "counter-app",
        &["empty-app", "jetton-app", "w5-extension-app"],
        &["app/src/styles.css"],
        "Keep the shared app CSS baseline byte-for-byte identical across empty, counter, jetton, and w5-extension app templates. If a style change is common, apply it to every shared stylesheet. If a divergence is intentional, document the exception in this test.",
    );
}

#[test]
fn test_new_nft_app_template_styles_only_extend_shared_baseline() {
    let baseline = read_new_template_file("counter-app", "app/src/styles.css");
    let nft_styles = read_new_template_file("nft-app", "app/src/styles.css");

    let domain_tail = nft_styles
        .strip_prefix(&baseline)
        .unwrap_or_else(|| {
            panic!(
                "nft-app/app/src/styles.css no longer starts with the shared app CSS baseline from counter-app/app/src/styles.css.\n\nKeep common theme tokens, base styles, scrollbars, animations, and overlay z-index rules identical, then append NFT-only classes after the shared block.\n\ninspect: diff -u src/commands/new/templates/counter-app/app/src/styles.css src/commands/new/templates/nft-app/app/src/styles.css"
            )
        });

    assert!(
        domain_tail.starts_with("\n/* ─── Domain-specific styles ─── */\n"),
        "nft-app/app/src/styles.css may only differ from the shared baseline by appending the documented NFT domain-specific block. Found unexpected tail after the shared baseline:\n{domain_tail}"
    );
}

#[test]
fn test_new_contract_app_template_shared_ui_components_match_baseline() {
    assert_new_template_files_match_baseline(
        "counter-app",
        &["jetton-app", "nft-app", "w5-extension-app"],
        &[
            "app/src/components/ui/alert.tsx",
            "app/src/components/ui/badge.tsx",
            "app/src/components/ui/button.tsx",
            "app/src/components/ui/card.tsx",
            "app/src/components/ui/dropdown-menu.tsx",
            "app/src/components/ui/input.tsx",
            "app/src/components/ui/label.tsx",
            "app/src/components/ui/separator.tsx",
            "app/src/components/ui/tabs.tsx",
            "app/src/components/ui/textarea.tsx",
        ],
        "These shadcn-style primitives are shared by all contract app templates. Keep them byte-for-byte identical unless a template-specific component is deliberately split into a separate file.",
    );
}

#[test]
fn test_new_empty_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-empty-localnet-deploy",
        "empty",
        false,
        "deployer",
        "scripts/deploy.tolk",
        "integration/snapshots/new/test_new_empty_project_localnet_deploy.stdout.txt",
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
        "integration/snapshots/new/test_new_counter_project_localnet_deploy.stdout.txt",
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
        "integration/snapshots/new/test_new_counter_app_project_localnet_deploy.stdout.txt",
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
        "integration/snapshots/new/test_new_jetton_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_nft_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-nft-localnet-deploy",
        "nft",
        false,
        "deployer",
        "scripts/deploy-collection.tolk",
        "integration/snapshots/new/test_new_nft_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_w5_extension_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-w5-extension-localnet-deploy",
        "w5-extension",
        false,
        "deployer",
        "scripts/deploy.tolk",
        "integration/snapshots/new/test_new_w5_extension_project_localnet_deploy.stdout.txt",
    );
}

#[test]
fn test_new_w5_extension_app_project_localnet_deploy_snapshot() {
    assert_new_project_localnet_deploy_snapshot(
        "new-w5-extension-app-localnet-deploy",
        "w5-extension",
        true,
        "deployer",
        "contracts/scripts/deploy.tolk",
        "integration/snapshots/new/test_new_w5_extension_app_project_localnet_deploy.stdout.txt",
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
        "integration/snapshots/new/test_new_empty_project_in_existed_directory.stderr.txt",
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
            "integration/snapshots/new/test_new_empty_project_in_existed_directory_with_acton_toml.stderr.txt",
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
            "integration/snapshots/new/test_new_empty_project_in_non_empty_current_directory.stdout.txt",
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

#[test]
fn test_new_current_directory_rejects_colliding_files_non_interactively() {
    let project = ProjectBuilder::new("new-current-directory-collision-noninteractive")
        .without_acton_toml()
        .raw_file("README.md", "# Existing README\n")
        .build();

    project
        .acton()
        .current_dir(project.path())
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("collision-project")
        .arg("--description")
        .arg("collision description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_current_directory_rejects_colliding_files_non_interactively.stderr.txt",
        );

    assert_eq!(
        fs::read_to_string(project.path().join("README.md")).unwrap(),
        "# Existing README\n"
    );
    assert!(!project.path().join("Acton.toml").exists());
}

#[test]
fn test_new_current_directory_overwrites_colliding_files_with_flag_non_interactively() {
    let project = ProjectBuilder::new("new-current-directory-collision-overwrite")
        .without_acton_toml()
        .raw_file("README.md", "# Existing README\n")
        .build();

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("collision-overwrite-project")
        .arg("--description")
        .arg("collision overwrite description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .arg("--overwrite")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_file_snapshot_matches(
            "README.md",
            "integration/snapshots/new/test_new_current_directory_overwrites_colliding_files_with_flag_non_interactively.readme.md",
        );
}

#[cfg(unix)]
#[test]
fn test_new_current_directory_warns_before_overwriting_colliding_files_interactively() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("new-current-directory-collision-interactive")
        .without_acton_toml()
        .raw_file("README.md", "# Existing README\n")
        .build();

    let mut session = project
        .acton()
        .current_dir(project.path())
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("interactive-collision")
        .arg("--description")
        .arg("interactive collision description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Warning: acton new will overwrite existing files:");
    session.expect("README.md");
    session.expect("Overwrite existing files?");
    session.send_line("", "failed to reject overwrite prompt");
    session.expect("Aborted to avoid overwriting existing files.");
    session.expect(Eof);

    assert_eq!(
        fs::read_to_string(project.path().join("README.md")).unwrap(),
        "# Existing README\n"
    );
    assert!(!project.path().join("Acton.toml").exists());
}

#[cfg(unix)]
#[test]
fn test_new_current_directory_overwrites_after_confirmation_interactively() {
    let project = ProjectBuilder::new("new-current-directory-collision-interactive-confirm")
        .without_acton_toml()
        .raw_file("README.md", "# Existing README\n")
        .build();

    let mut session = project
        .acton()
        .current_dir(project.path())
        .arg("new")
        .arg(".")
        .arg("--name")
        .arg("interactive-collision-confirm")
        .arg("--description")
        .arg("interactive collision confirm description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Include the TypeScript dApp?");
    session.send_line("", "failed to keep default no-app choice");
    session.expect("Do you want to configure advanced options (Git hooks, license, etc.)?");
    session.send_line("", "failed to keep default no-advanced choice");
    session.expect("Warning: acton new will overwrite existing files:");
    session.expect("README.md");
    session.expect("Overwrite existing files?");
    session.send_line("y", "failed to confirm overwrite prompt");
    session.expect("Created new Acton project");
    session.assert_file_snapshot_matches(
        "README.md",
        "integration/snapshots/new/test_new_current_directory_overwrites_colliding_files_with_flag_non_interactively.readme.md",
    );
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
    assert!(project_dir.join(".env.example").exists());
    assert!(!project_dir.join(".env").exists());
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
            "integration/snapshots/new/test_new_project_fails_when_git_init_fails.stderr.txt",
        )
        .assert_file_snapshot_matches(
            "foobar/Acton.toml",
            "integration/snapshots/new/test_new_project_fails_when_git_init_fails.acton.toml.gen",
        );
    assert!(project_dir.join("contracts").exists());
    assert!(project_dir.join("tests").exists());
    assert!(project_dir.join(".gitignore").exists());
    assert!(project_dir.join(".env.example").exists());
    assert!(!project_dir.join(".env").exists());
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
        "integration/snapshots/new/test_new_project_warns_when_git_is_unavailable_but_still_succeeds.stdout.txt",
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

    let output = project
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

    output.assert_file_snapshot_matches(
        "foobar/contracts/Empty.tolk",
        "integration/snapshots/new/test_new_project_uses_acton_user_when_git_user_name_is_missing.empty.tolk.gen",
    );

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
            "integration/snapshots/new/test_new_empty_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_empty_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_empty_project_full_flow_test.stdout.txt",
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
            "integration/snapshots/new/test_new_empty_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_empty_project_full_flow_check.stderr.txt",
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
            "integration/snapshots/new/test_new_empty_project_full_flow_fmt.stdout.txt",
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
            "integration/snapshots/new/test_new_counter_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_counter_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_counter_project_full_flow_test.stdout.txt",
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
            "integration/snapshots/new/test_new_counter_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_counter_project_full_flow_check.stderr.txt",
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
            "integration/snapshots/new/test_new_counter_project_full_flow_fmt.stdout.txt",
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
            "integration/snapshots/new/test_new_jetton_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_jetton_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_jetton_project_full_flow_test.stdout.txt",
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
            "integration/snapshots/new/test_new_jetton_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_jetton_project_full_flow_check.stderr.txt",
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
            "integration/snapshots/new/test_new_jetton_project_full_flow_fmt.stdout.txt",
        );
}

#[test]
fn test_new_nft_project_full_flow() {
    let project = ProjectBuilder::new("new-nft-full")
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
        .arg("nft")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_nft_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_nft_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_nft_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy-collection.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_nft_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_nft_project_full_flow_check.stderr.txt",
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
            "integration/snapshots/new/test_new_nft_project_full_flow_fmt.stdout.txt",
        );
}

#[test]
fn test_new_w5_extension_project_full_flow() {
    let project = ProjectBuilder::new("new-w5-extension-full")
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
        .arg("w5-extension")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_w5_extension_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_w5_extension_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/new/test_new_w5_extension_project_full_flow_test.stdout.txt",
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
            "integration/snapshots/new/test_new_w5_extension_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/new/test_new_w5_extension_project_full_flow_check.stderr.txt",
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
            "integration/snapshots/new/test_new_w5_extension_project_full_flow_fmt.stdout.txt",
        );
}

#[test]
fn test_new_empty_project_with_env_example() {
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
    assert!(project.path().join("foobar/.env.example").exists());
    assert!(!project.path().join("foobar/.env").exists());
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
            "integration/snapshots/new/test_new_empty_project_editorconfig.gen",
        );
}

fn create_project_and_check_wrappers(
    test_name: &str,
    template: &str,
    contracts_and_wrappers: &[(&str, &str)],
) {
    create_project_and_check_wrappers_inner(test_name, template, false, contracts_and_wrappers);
}

fn create_app_project_and_check_wrappers(
    test_name: &str,
    template: &str,
    contracts_and_wrappers: &[(&str, &str)],
) {
    create_project_and_check_wrappers_inner(test_name, template, true, contracts_and_wrappers);
}

fn create_project_and_check_wrappers_inner(
    test_name: &str,
    template: &str,
    app: bool,
    contracts_and_wrappers: &[(&str, &str)],
) {
    let workspace = ProjectBuilder::new(test_name).without_acton_toml().build();

    let project_dir = workspace.path().join("generated");
    let project_dir_str = project_dir.display().to_string();

    let mut cmd = workspace
        .acton()
        .arg("new")
        .arg(&project_dir_str)
        .arg("--name")
        .arg("wrapper-check")
        .arg("--description")
        .arg("wrapper consistency check")
        .arg("--template")
        .arg(template)
        .arg("--license")
        .arg("MIT");

    if app {
        cmd = cmd.arg("--app");
    }

    cmd.run().success();

    workspace
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success();

    for &(contract_name, template_wrapper_path) in contracts_and_wrappers {
        let template_wrapper = fs::read_to_string(project_dir.join(template_wrapper_path))
            .unwrap_or_else(|e| {
                panic!("Failed to read template wrapper {template_wrapper_path}: {e}")
            });
        fs::remove_file(project_dir.join(template_wrapper_path)).unwrap_or_else(|e| {
            panic!("Failed to remove template wrapper {template_wrapper_path}: {e}")
        });

        workspace
            .acton()
            .current_dir(&project_dir)
            .arg("wrapper")
            .arg(contract_name)
            .run()
            .success();

        let generated_wrapper = fs::read_to_string(project_dir.join(template_wrapper_path))
            .unwrap_or_else(|e| {
                panic!("Failed to read generated wrapper {template_wrapper_path}: {e}")
            });

        assert_eq!(
            template_wrapper, generated_wrapper,
            "Template wrapper `{template_wrapper_path}` does not match auto-generated wrapper for contract `{contract_name}`"
        );
    }
}

#[cfg(unix)]
fn create_app_project_and_check_typescript_wrappers(
    test_name: &str,
    template: &str,
    contracts_and_wrappers: &[(&str, &str)],
) {
    if !is_npm_available() {
        eprintln!("Skipping TypeScript wrapper generation check: npm is not available in PATH");
        return;
    }

    let workspace = ProjectBuilder::new(test_name).without_acton_toml().build();

    let project_dir = workspace.path().join("generated");
    create_app_project(&workspace, &project_dir, template);

    workspace
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success();

    for &(contract_name, template_wrapper_path) in contracts_and_wrappers {
        let template_wrapper = fs::read_to_string(project_dir.join(template_wrapper_path))
            .unwrap_or_else(|e| {
                panic!("Failed to read template wrapper {template_wrapper_path}: {e}")
            });
        fs::remove_file(project_dir.join(template_wrapper_path)).unwrap_or_else(|e| {
            panic!("Failed to remove template wrapper {template_wrapper_path}: {e}")
        });

        let output = Command::new(acton_exe())
            .args(["wrapper", contract_name, "--ts"])
            .current_dir(&project_dir)
            .output()
            .unwrap();

        if !output.status.success() && npm_failure_looks_environment_specific(&output) {
            eprintln!(
                "Skipping TypeScript wrapper generation check for {template}/{contract_name} due to environment-specific npx failure:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        assert!(
            output.status.success(),
            "acton wrapper {contract_name} --ts failed for {template} app:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let generated_wrapper = fs::read_to_string(project_dir.join(template_wrapper_path))
            .unwrap_or_else(|e| {
                panic!("Failed to read generated wrapper {template_wrapper_path}: {e}")
            });

        assert_eq!(
            template_wrapper, generated_wrapper,
            "Template TypeScript wrapper `{template_wrapper_path}` does not match auto-generated `acton wrapper {contract_name} --ts` output"
        );
    }
}

#[cfg(unix)]
#[test]
fn test_new_counter_app_template_typescript_wrappers_match_autogenerated() {
    create_app_project_and_check_typescript_wrappers(
        "new-counter-app-ts-wrapper-check",
        "counter",
        &[("Counter", "wrappers-ts/Counter.gen.ts")],
    );
}

#[cfg(unix)]
#[test]
fn test_new_jetton_app_template_typescript_wrappers_match_autogenerated() {
    create_app_project_and_check_typescript_wrappers(
        "new-jetton-app-ts-wrapper-check",
        "jetton",
        &[
            ("JettonMinter", "wrappers-ts/JettonMinter.gen.ts"),
            ("JettonWallet", "wrappers-ts/JettonWallet.gen.ts"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn test_new_nft_app_template_typescript_wrappers_match_autogenerated() {
    create_app_project_and_check_typescript_wrappers(
        "new-nft-app-ts-wrapper-check",
        "nft",
        &[
            ("NftCollection", "wrappers-ts/NftCollection.gen.ts"),
            ("NftItem", "wrappers-ts/NftItem.gen.ts"),
        ],
    );
}

#[cfg(unix)]
#[test]
fn test_new_w5_extension_app_template_typescript_wrappers_match_autogenerated() {
    create_app_project_and_check_typescript_wrappers(
        "new-w5-extension-app-ts-wrapper-check",
        "w5-extension",
        &[
            ("SimpleExtension", "wrappers-ts/SimpleExtension.gen.ts"),
            ("WalletV5", "wrappers-ts/WalletV5.gen.ts"),
        ],
    );
}

#[test]
fn test_new_empty_template_wrappers_match_autogenerated() {
    create_project_and_check_wrappers(
        "new-empty-wrapper-check",
        "empty",
        &[("Empty", "wrappers/Empty.gen.tolk")],
    );
}

#[test]
fn test_new_counter_template_wrappers_match_autogenerated() {
    create_project_and_check_wrappers(
        "new-counter-wrapper-check",
        "counter",
        &[("Counter", "wrappers/Counter.gen.tolk")],
    );
}

#[test]
fn test_new_counter_app_template_wrappers_match_autogenerated() {
    create_app_project_and_check_wrappers(
        "new-counter-app-wrapper-check",
        "counter",
        &[("Counter", "contracts/wrappers/Counter.gen.tolk")],
    );
}

#[test]
fn test_new_jetton_template_wrappers_match_autogenerated() {
    create_project_and_check_wrappers(
        "new-jetton-wrapper-check",
        "jetton",
        &[
            ("JettonMinter", "wrappers/JettonMinter.gen.tolk"),
            ("JettonWallet", "wrappers/JettonWallet.gen.tolk"),
        ],
    );
}

#[test]
fn test_new_jetton_app_template_wrappers_match_autogenerated() {
    create_app_project_and_check_wrappers(
        "new-jetton-app-wrapper-check",
        "jetton",
        &[
            ("JettonMinter", "contracts/wrappers/JettonMinter.gen.tolk"),
            ("JettonWallet", "contracts/wrappers/JettonWallet.gen.tolk"),
        ],
    );
}

#[test]
fn test_new_nft_template_wrappers_match_autogenerated() {
    create_project_and_check_wrappers(
        "new-nft-wrapper-check",
        "nft",
        &[
            ("NftCollection", "wrappers/NftCollection.gen.tolk"),
            ("NftItem", "wrappers/NftItem.gen.tolk"),
        ],
    );
}

#[test]
fn test_new_nft_app_template_wrappers_match_autogenerated() {
    create_app_project_and_check_wrappers(
        "new-nft-app-wrapper-check",
        "nft",
        &[
            ("NftCollection", "contracts/wrappers/NftCollection.gen.tolk"),
            ("NftItem", "contracts/wrappers/NftItem.gen.tolk"),
        ],
    );
}

#[test]
fn test_new_w5_extension_template_wrappers_match_autogenerated() {
    create_project_and_check_wrappers(
        "new-w5-extension-wrapper-check",
        "w5-extension",
        &[
            ("SimpleExtension", "wrappers/SimpleExtension.gen.tolk"),
            ("WalletV5", "wrappers/WalletV5.gen.tolk"),
        ],
    );
}

#[test]
fn test_new_w5_extension_app_template_wrappers_match_autogenerated() {
    create_app_project_and_check_wrappers(
        "new-w5-extension-app-wrapper-check",
        "w5-extension",
        &[
            (
                "SimpleExtension",
                "contracts/wrappers/SimpleExtension.gen.tolk",
            ),
            ("WalletV5", "contracts/wrappers/WalletV5.gen.tolk"),
        ],
    );
}
