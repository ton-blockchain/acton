use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn test_wallet_new_local() {
    let project = ProjectBuilder::new("wallet-new-local").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    let mnemonic_file = project.path().join("my-wallet.mnemonic");
    assert!(!mnemonic_file.exists()); // Should NOT exist anymore

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(!acton_toml.contains("[wallets.my-wallet]"));

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    assert!(wallets_toml.contains("[wallets.my-wallet]"));
    assert!(wallets_toml.contains("kind = \"v5r1\""));
    assert!(wallets_toml.contains("mnemonic = \"")); // Should contain direct mnemonic

    output.assert_snapshot_matches("integration/snapshots/wallet/test_wallet_new_local.stdout.txt");
}

#[test]
fn test_wallet_new_global() {
    let project = ProjectBuilder::new("wallet-new-global").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_new()
        .arg("--name")
        .arg("global-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .run()
        .success();

    let global_wallets_dir = home_path.join(".acton").join("wallets");
    let global_config = global_wallets_dir.join("global.wallets.toml");
    let global_mnemonic_file = global_wallets_dir.join("global-wallet.mnemonic");

    assert!(global_config.exists());
    assert!(!global_mnemonic_file.exists()); // Should NOT exist anymore

    let global_toml = fs::read_to_string(global_config).unwrap();
    assert!(global_toml.contains("[wallets.global-wallet]"));
    assert!(global_toml.contains("mnemonic = \""));

    // Check symlink
    let symlink = project.path().join("global.wallets.toml");
    assert!(symlink.exists());

    output
        .assert_snapshot_matches("integration/snapshots/wallet/test_wallet_new_global.stdout.txt");
}

#[test]
fn test_wallet_new_already_exists() {
    let project = ProjectBuilder::new("wallet-exists").build();

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .failure();

    output.assert_contains("Wallet my-wallet already exists in local config");
}

#[test]
fn test_wallet_new_unknown_kind() {
    let project = ProjectBuilder::new("wallet-unknown-kind").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("1111")
        .arg("--local")
        .run()
        .failure();

    output.assert_contains("Unsupported wallet version 1111. Supported versions: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2");
}

#[test]
fn test_wallet_list() {
    let project = ProjectBuilder::new("wallet-list").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_new()
        .arg("--name")
        .arg("global-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .run()
        .success();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_new()
        .arg("--name")
        .arg("local-wallet")
        .arg("--version")
        .arg("v4r2")
        .arg("--local")
        .run()
        .success();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_list()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/wallet/test_wallet_list.stdout.txt");
}
