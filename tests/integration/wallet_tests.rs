use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton::wallets;
use serde_json::Value;
use std::fs;
#[cfg(unix)]
use std::time::Duration;
use ton_api::Network;
use tonlib_core::cell::Cell;
use tonlib_core::tlb_types::tlb::TLB;
use tonlib_core::wallet::mnemonic::Mnemonic;
use tonlib_core::wallet::ton_wallet::TonWallet;
use tonlib_core::wallet::wallet_version::WalletVersion;

#[allow(dead_code)]
const KEYRING_SERVICE: &str = "ton.acton.wallet";
const TEST_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

fn wallet_sign_fixture() -> (String, String, String) {
    let mnemonic = Mnemonic::from_str(TEST_MNEMONIC, &None).expect("invalid test mnemonic");
    let key_pair = mnemonic.to_key_pair().expect("mnemonic to keypair failed");
    let version = WalletVersion::V5R1;
    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)
        .expect("failed to build test wallet");

    let body = wallet
        .create_external_body(1_700_000_000, 7, Vec::<tonlib_core::cell::ArcCell>::new())
        .expect("failed to build external body");
    let body_hex = body
        .to_boc_hex(false)
        .expect("failed to encode body hex boc");
    let body_base64 = body
        .to_boc_b64(false)
        .expect("failed to encode body base64 boc");

    let signed = wallet
        .sign_external_body(&body)
        .expect("failed to sign external body");
    let signed_hex = signed
        .to_boc_hex(false)
        .expect("failed to encode signed body hex boc");

    (body_hex, body_base64, signed_hex)
}

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

    let global_wallets_dir = home_path.join(".config").join("acton").join("wallets");
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

#[cfg(unix)]
#[test]
fn test_wallet_import_all_fields_interactive() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-import-all-fields-interactive").build();
    let mut session = project
        .acton()
        .wallet_import()
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Wallet name:");
    session.send_line("interactive-wallet", "failed to send wallet name");

    session.expect("Save wallet to:");
    session.send_line("", "failed to select default local wallet config");

    session.expect("Enter mnemonic (24 words):");
    session.send_line(TEST_MNEMONIC, "failed to send mnemonic");

    session.expect("Wallet type:");
    session.send_line("", "failed to select default wallet type");

    session.expect("Wallet successfully created and added to");
    session.expect(Eof);

    session.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_import_all_fields_interactive.wallets.toml.txt",
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

#[test]
fn test_wallet_export_mnemonic_requires_interactive_mode() {
    let project = ProjectBuilder::new("wallet-export-mnemonic-non-interactive").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("my-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_export_mnemonic()
        .arg("my-wallet")
        .run()
        .failure();

    output.assert_contains("Exporting mnemonic is only allowed in interactive mode");
}

#[test]
fn test_wallet_export_mnemonic_non_interactive_denies_before_name_validation() {
    let project = ProjectBuilder::new("wallet-export-mnemonic-no-name-validation").build();

    let output = project
        .acton()
        .wallet_export_mnemonic()
        .arg("non-existent")
        .run()
        .failure();

    output.assert_contains("Exporting mnemonic is only allowed in interactive mode");
}

#[test]
fn test_wallet_sign_outputs_signed_body_boc_hex() {
    let project = ProjectBuilder::new("wallet-sign-hex-output").build();
    let (body_hex, _, signed_hex_expected) = wallet_sign_fixture();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("sign-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_sign()
        .arg("sign-wallet")
        .arg("--body")
        .arg(&body_hex)
        .run()
        .success();

    let signed_hex = output.get_stdout().trim().to_owned();
    assert_eq!(signed_hex, signed_hex_expected);
    assert!(Cell::from_boc_hex(&signed_hex).is_ok());
}

#[test]
fn test_wallet_sign_accepts_base64_body_input() {
    let project = ProjectBuilder::new("wallet-sign-ambiguous").build();
    let (body_hex, body_base64, signed_hex_expected) = wallet_sign_fixture();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("sign-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output_hex = project
        .acton()
        .wallet_sign()
        .arg("sign-wallet")
        .arg("--body")
        .arg(&body_hex)
        .run()
        .success();

    let output_base64 = project
        .acton()
        .wallet_sign()
        .arg("sign-wallet")
        .arg("--body")
        .arg(&body_base64)
        .run()
        .success();

    let sig_from_hex = output_hex.get_stdout().trim().to_owned();
    let sig_from_base64 = output_base64.get_stdout().trim().to_owned();
    assert_eq!(sig_from_hex, signed_hex_expected);
    assert_eq!(sig_from_base64, signed_hex_expected);
}

#[test]
fn test_wallet_sign_json_reports_detected_format() {
    let project = ProjectBuilder::new("wallet-sign-json").build();
    let (_, body_base64, signed_hex_expected) = wallet_sign_fixture();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("sign-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_sign()
        .arg("sign-wallet")
        .arg("--body")
        .arg(&body_base64)
        .arg("--json")
        .run()
        .success();

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["wallet"], "sign-wallet");
    assert_eq!(json["input"], "base64");
    assert_eq!(json["output"], "hex");
    assert_eq!(json["signed_body"], signed_hex_expected);

    let signed_hex = json["signed_body"].as_str().unwrap();
    assert!(Cell::from_boc_hex(signed_hex).is_ok());
}

#[test]
fn test_wallet_sign_rejects_invalid_payload() {
    let project = ProjectBuilder::new("wallet-sign-invalid-payload").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("sign-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_sign()
        .arg("sign-wallet")
        .arg("--body")
        .arg("not-valid@@@")
        .run()
        .failure();

    output.assert_contains("Body must be a valid BoC encoded as hex or base64");
}

#[test]
fn test_wallet_remove_local() {
    let project = ProjectBuilder::new("wallet-remove-local").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("remove-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_remove()
        .arg("remove-wallet")
        .arg("-y")
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap_or_default();
    assert!(!wallets_toml.contains("[wallets.remove-wallet]"));

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_remove_local.stdout.txt",
    );
}

#[test]
fn test_wallet_remove_global() {
    let project = ProjectBuilder::new("wallet-remove-global").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_import()
        .arg("--name")
        .arg("remove-remote-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_remove()
        .arg("remove-remote-wallet")
        .arg("-y")
        .run()
        .success();

    let global_wallets = home_path
        .join(".config")
        .join("acton")
        .join("wallets")
        .join("global.wallets.toml");
    let global_toml = fs::read_to_string(global_wallets).unwrap_or_default();
    assert!(!global_toml.contains("[wallets.remove-remote-wallet]"));

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_remove_global.stdout.txt",
    );
}

#[test]
fn test_wallet_remove_prefers_local_when_names_overlap() {
    let project = ProjectBuilder::new("wallet-remove-prefers-local").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_import()
        .arg("--name")
        .arg("shared-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_import()
        .arg("--name")
        .arg("shared-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_remove()
        .arg("shared-wallet")
        .arg("-y")
        .run()
        .success();

    let local_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap_or_default();
    assert!(!local_toml.contains("[wallets.shared-wallet]"));

    let global_wallets = home_path
        .join(".config")
        .join("acton")
        .join("wallets")
        .join("global.wallets.toml");
    let global_toml = fs::read_to_string(global_wallets).unwrap_or_default();
    assert!(global_toml.contains("[wallets.shared-wallet]"));

    output.assert_contains("removed from wallets.toml");
}

#[test]
fn test_wallet_remove_not_found() {
    let project = ProjectBuilder::new("wallet-remove-not-found").build();

    let output = project
        .acton()
        .wallet_remove()
        .arg("non-existent")
        .run()
        .failure();

    output.assert_contains("Wallet non-existent not found");
}

#[test]
fn test_wallet_remove_requires_confirmation_in_non_interactive_mode() {
    let project = ProjectBuilder::new("wallet-remove-confirmation-required").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("confirm-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_remove()
        .arg("confirm-wallet")
        .run()
        .failure();

    output.assert_contains("This action cannot be undone");
    output.assert_contains("Re-run with -y/--yes in non-interactive mode");
}

#[cfg(feature = "only_ci")]
#[test]
fn test_wallet_remove_deletes_keyring_mnemonic() {
    use keyring::{Entry, Error as KeyringError};

    if !wallets::is_keyring_supported() {
        return;
    }

    let project = ProjectBuilder::new("wallet-remove-keyring").build();
    let keyring_id = format!(
        "wallet-remove-keyring-{}",
        project.path().file_name().unwrap().to_string_lossy()
    );

    let entry = Entry::new(KEYRING_SERVICE, &keyring_id).unwrap();
    let _ = entry.delete_credential();
    entry.set_password(TEST_MNEMONIC).unwrap();

    fs::write(
        project.path().join("wallets.toml"),
        format!(
            r#"[wallets.secure-wallet]
kind = "v5r1"
workchain = 0
keys = {{ mnemonic-keyring = "{keyring_id}" }}
"#
        ),
    )
    .unwrap();

    let output = project
        .acton()
        .wallet_remove()
        .arg("secure-wallet")
        .arg("-y")
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap_or_default();
    assert!(!wallets_toml.contains("[wallets.secure-wallet]"));

    let err = Entry::new(KEYRING_SERVICE, &keyring_id)
        .unwrap()
        .get_password()
        .expect_err("keyring entry should be deleted");
    assert!(matches!(err, KeyringError::NoEntry));

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_remove_keyring.stdout.txt",
    );
}

#[test]
fn test_wallet_remove_json() {
    let project = ProjectBuilder::new("wallet-remove-json").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("remove-json-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_remove()
        .arg("remove-json-wallet")
        .arg("-y")
        .arg("--json")
        .run()
        .success();

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["name"], "remove-json-wallet");
    assert_eq!(json["is_global"], false);
    assert_eq!(json["keyring_mnemonic_removed"], false);
}

#[test]
fn test_wallet_new_json() {
    let project = ProjectBuilder::new("wallet-new-json").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("json-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--json")
        .run()
        .success();

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["name"], "json-wallet");
    assert!(json["address"].is_string());
    assert_eq!(json["kind"], "v5r1");
    assert_eq!(json["is_global"], false);
    assert_eq!(json["airdrop_requested"], false);
    assert!(json.get("airdrop").is_none());
}

#[test]
fn test_wallet_import_json() {
    let project = ProjectBuilder::new("wallet-import-json").build();
    let mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";

    let output = project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("imported-json")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--json")
        .arg(mnemonic)
        .run()
        .success();

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["name"], "imported-json");
    assert!(json["address"].is_string());
    assert_eq!(json["kind"], "v5r1");
    assert_eq!(json["is_global"], false);
}

#[test]
fn test_wallet_list_json() {
    let project = ProjectBuilder::new("wallet-list-json").build();

    // Create a wallet first
    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("list-json-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .success();

    let output = project.acton().wallet_list().arg("--json").run().success();

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["success"], true);
    let wallets = json["wallets"].as_array().unwrap();
    assert!(wallets.iter().any(|w| w["name"] == "list-json-wallet"));

    let wallet = wallets
        .iter()
        .find(|w| w["name"] == "list-json-wallet")
        .unwrap();
    assert!(wallet["address"].is_string());
    assert_eq!(wallet["kind"], "v5r1");
    assert_eq!(wallet["is_global"], false);
}
