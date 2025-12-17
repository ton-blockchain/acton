use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn test_wallet_new_with_flags() {
    let project = ProjectBuilder::new("wallet-new-flags").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .run()
        .success();

    let mnemonic_file = project.path().join("my-wallet.mnemonic");
    assert!(mnemonic_file.exists());
    let mnemonic = fs::read_to_string(mnemonic_file).unwrap();

    assert_eq!(mnemonic.split_whitespace().count(), 24);

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[wallets.my-wallet]"));
    assert!(acton_toml.contains("kind = \"v5r1\""));
    assert!(acton_toml.contains("mnemonic-file = \"my-wallet.mnemonic\""));

    output
        .assert_file_snapshot_matches(
            "Acton.toml",
            "integration/snapshots/wallet/test_wallet_new_with_flags.Acton.toml.txt",
        )
        .assert_snapshot_matches(
            "integration/snapshots/wallet/test_wallet_new_with_flags.stdout.txt",
        );
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
        .run()
        .success();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .run()
        .failure();

    output.assert_contains("Wallet my-wallet already exists");
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
        .run()
        .failure();

    output.assert_contains("Unsupported wallet version 1111. Supported versions: v1r1, v1r2, v1r3, v2r1, v2r2, v3r1, v3r2, v4r1, v4r2, v5r1, highloadv1r1, highloadv1r2, highloadv2, highloadv2r1, highloadv2r2");
}
