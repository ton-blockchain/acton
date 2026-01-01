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

    output.assert_contains("[possible values: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2]");
}

#[test]
fn test_wallet_new_normalized_name() {
    let project = ProjectBuilder::new("wallet-normalized").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("My Wallet 123!")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    // "My Wallet 123!" -> "my-wallet-123"
    assert!(wallets_toml.contains("[wallets.my-wallet-123]"));
    assert!(wallets_toml.contains("[wallets.my-wallet-123.expected]"));

    output.assert_contains("Wallet successfully created and added to wallets.toml");
}

#[test]
fn test_wallet_new_normalization_variants() {
    let project = ProjectBuilder::new("wallet-variants").build();

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("Test.Wallet_Name")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    // "Test.Wallet_Name" -> "testwallet_name" (dot is removed, underscore kept)
    assert!(wallets_toml.contains("[wallets.testwallet_name]"));

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("Too   Many   Spaces")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    assert!(wallets_toml.contains("[wallets.too---many---spaces]"));
}

#[test]
fn test_wallet_new_invalid_name() {
    let project = ProjectBuilder::new("wallet-invalid").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("!!!")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .failure();

    output.assert_contains("Wallet name '!!!' is invalid");
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

#[test]
fn test_wallet_import_local() {
    let project = ProjectBuilder::new("wallet-import-local").build();
    let mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

    let output = project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("imported-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(mnemonic)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_local.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_import_local.wallets.toml.txt",
    );
}

#[test]
fn test_wallet_import_invalid_mnemonic() {
    let project = ProjectBuilder::new("wallet-import-invalid").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("invalid-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("invalid mnemonic phrase")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/wallet/test_wallet_import_invalid_mnemonic.stderr.txt",
        );
}

#[test]
fn test_wallet_import_already_exists() {
    let project = ProjectBuilder::new("wallet-import-exists").build();
    let mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(mnemonic)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(mnemonic)
        .run()
        .failure();

    output.assert_contains("Wallet my-wallet already exists in local config");
}

#[test]
fn test_wallet_preserves_comments() {
    let project = ProjectBuilder::new("wallet-comments")
        .raw_file(
            "wallets.toml",
            r#"# This is a global comment
[wallets.existing]
# This is a comment for existing wallet
kind = "v4r2"
workchain = 0
keys = { mnemonic = "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12 word13 word14 word15 word16 word17 word18 word19 word20 word21 word22 word23 word24" }

# This is a comment before expected
[wallets.existing.expected]
address-testnet = "EQD_existing_address"
"#,
        )
        .build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("new-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_preserves_comments.wallets.toml.txt",
    );
}
