use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton::wallets;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_wallet::{Mnemonic, TonWallet, WalletVersion};
use ton_api::Network;

#[allow(dead_code)]
const KEYRING_SERVICE: &str = "ton.acton.wallet";
const TEST_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";
const SECOND_TEST_MNEMONIC: &str = "section garden tomato dinner season dice renew length useful spin trade intact use universe what post spike keen mandate behind concert egg doll rug";
const TEST_WALLET_KEYRING_SUPPORTED_ENV: &str = "ACTON_TEST_WALLET_KEYRING_SUPPORTED";
const TEST_KEYRING_DIR_ENV: &str = "ACTON_TEST_KEYRING_DIR";
const TEST_TONCENTER_V3_URL_ENV: &str = "ACTON_TEST_TONCENTER_V3_URL";

#[derive(Clone)]
struct ToncenterMockResponse {
    status: u16,
    body: String,
}

#[derive(Debug, Clone)]
struct CapturedToncenterRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn wallet_sign_fixture() -> (String, String, String) {
    wallet_sign_fixture_for_mnemonic(TEST_MNEMONIC)
}

fn wallet_sign_fixture_for_mnemonic(mnemonic_str: &str) -> (String, String, String) {
    let mnemonic = Mnemonic::from_str(mnemonic_str, None).expect("invalid test mnemonic");
    let key_pair = mnemonic.to_key_pair().expect("mnemonic to keypair failed");
    let version = WalletVersion::V5R1;
    let wallet_id = wallets::wallet_id(version, &Network::Testnet);
    let wallet = TonWallet::new_with_params(version, key_pair, 0, wallet_id)
        .expect("failed to build test wallet");

    let body = wallet
        .create_ext_in_body(1_700_000_000, 7, Vec::<TonCell>::new())
        .expect("failed to build external body");
    let body_hex = body.to_boc_hex().expect("failed to encode body hex boc");
    let body_base64 = body
        .to_boc_base64()
        .expect("failed to encode body base64 boc");

    let signed = wallet
        .sign_ext_in_body(&body)
        .expect("failed to sign external body");
    let signed_hex = signed
        .to_boc_hex()
        .expect("failed to encode signed body hex boc");

    (body_hex, body_base64, signed_hex)
}

fn spawn_toncenter_v3_mock(
    responses: Vec<ToncenterMockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind toncenter mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set toncenter mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get toncenter mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedToncenterRequest>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for response in responses {
            let wait_until = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for toncenter request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("toncenter mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set toncenter mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone toncenter mock stream"),
            );
            let mut request_line = String::new();
            let read_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                request_line.clear();
                match reader.read_line(&mut request_line) {
                    Ok(0) => {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => break,
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("failed to read toncenter request line: {err}"),
                }
            }

            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();

            let mut headers = Vec::new();
            let mut content_length = 0_usize;
            loop {
                let mut header_line = String::new();
                let read = reader
                    .read_line(&mut header_line)
                    .expect("failed to read toncenter header line");
                if read == 0 || header_line == "\r\n" {
                    break;
                }

                if let Some((name, value)) = header_line.split_once(':') {
                    let name = name.trim().to_string();
                    let value = value.trim().to_string();
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.parse().unwrap_or(0);
                    }
                    headers.push((name, value));
                }
            }

            if content_length > 0 {
                let mut request_body = vec![0_u8; content_length];
                reader
                    .read_exact(&mut request_body)
                    .expect("failed to read toncenter request body");
            }

            captured_requests_thread
                .lock()
                .expect("captured toncenter requests mutex poisoned")
                .push(CapturedToncenterRequest {
                    method,
                    path,
                    headers,
                });

            let raw_response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                status_text(response.status),
                response.body.len(),
                response.body
            );
            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write toncenter response");
            stream.flush().expect("failed to flush toncenter response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn expected_testnet_address(project_root: &Path, wallet_name: &str) -> String {
    let content =
        fs::read_to_string(project_root.join("wallets.toml")).expect("failed to read wallets.toml");
    let value: toml::Value = toml::from_str(&content).expect("wallets.toml must be valid TOML");
    value["wallets"][wallet_name]["expected"]["address-testnet"]
        .as_str()
        .expect("wallets.<name>.expected.address-testnet must be present")
        .to_string()
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
fn test_wallet_new_rejects_conflicting_global_and_local_flags() {
    let project = ProjectBuilder::new("wallet-new-conflicting-flags").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("conflict-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .arg("--local")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_new_rejects_conflicting_global_and_local_flags.stderr.txt",
    );
}

#[test]
fn test_wallet_new_global_duplicate_name() {
    let project = ProjectBuilder::new("wallet-new-global-duplicate").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_new()
        .arg("--name")
        .arg("global-duplicate")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .run()
        .success();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_new()
        .arg("--name")
        .arg("global-duplicate")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_new_global_duplicate_name.stderr.txt",
    );
}

#[test]
fn test_wallet_new_secure_true_fails_when_keyring_unsupported() {
    let project = ProjectBuilder::new("wallet-new-secure-true-unsupported").build();

    let output = project
        .acton()
        .arg("wallet")
        .arg("new")
        .current_dir(project.path())
        .arg("--name")
        .arg("secure-unsupported")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure")
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_new_secure_true_fails_when_keyring_unsupported.stderr.txt",
    );
}

#[test]
fn test_wallet_new_secure_false_succeeds_when_keyring_unsupported() {
    let project = ProjectBuilder::new("wallet-new-secure-false-unsupported").build();

    let output = project
        .acton()
        .arg("wallet")
        .arg("new")
        .current_dir(project.path())
        .arg("--name")
        .arg("secure-false-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=false")
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_new_secure_false_succeeds_when_keyring_unsupported.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_new_secure_false_succeeds_when_keyring_unsupported.wallets.toml.txt",
    );
}

#[test]
fn test_wallet_new_falls_back_to_plain_mnemonic_when_keyring_unsupported() {
    let project = ProjectBuilder::new("wallet-new-secure-fallback-unsupported").build();

    let output = project
        .acton()
        .arg("wallet")
        .arg("new")
        .current_dir(project.path())
        .arg("--name")
        .arg("secure-fallback-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_new_falls_back_to_plain_mnemonic_when_keyring_unsupported.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_new_falls_back_to_plain_mnemonic_when_keyring_unsupported.wallets.toml.txt",
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
fn test_wallet_import_global() {
    let project = ProjectBuilder::new("wallet-import-global").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_import()
        .arg("--name")
        .arg("imported-global")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let global_wallets_dir = home_path.join(".config").join("acton").join("wallets");
    let global_config = global_wallets_dir.join("global.wallets.toml");
    assert!(global_config.exists());

    let global_toml = fs::read_to_string(global_config).unwrap();
    assert!(global_toml.contains("[wallets.imported-global]"));
    assert!(global_toml.contains("kind = \"v5r1\""));
    assert!(global_toml.contains("workchain = 0"));
    assert!(global_toml.contains("mnemonic = \""));

    let local_wallets_path = project.path().join("wallets.toml");
    if local_wallets_path.exists() {
        let local_toml = fs::read_to_string(local_wallets_path).unwrap();
        assert!(!local_toml.contains("[wallets.imported-global]"));
    }

    let symlink = project.path().join("global.wallets.toml");
    assert!(symlink.exists());

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_global.stdout.txt",
    );
}

#[test]
fn test_wallet_import_rejects_conflicting_global_and_local_flags() {
    let project = ProjectBuilder::new("wallet-import-conflicting-flags").build();

    let output = project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("import-conflict")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_rejects_conflicting_global_and_local_flags.stderr.txt",
    );
}

#[test]
fn test_wallet_import_invalid_name() {
    let project = ProjectBuilder::new("wallet-import-invalid-name").build();

    let output = project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("!!!")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_invalid_name.stderr.txt",
    );
}

#[test]
fn test_wallet_import_secure_true_fails_when_keyring_unsupported() {
    let project = ProjectBuilder::new("wallet-import-secure-true-unsupported").build();

    let output = project
        .acton()
        .arg("wallet")
        .arg("import")
        .current_dir(project.path())
        .arg("--name")
        .arg("import-secure-unsupported")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=true")
        .arg(TEST_MNEMONIC)
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_secure_true_fails_when_keyring_unsupported.stderr.txt",
    );
}

#[test]
fn test_wallet_import_secure_false_succeeds_when_keyring_unsupported() {
    let project = ProjectBuilder::new("wallet-import-secure-false-unsupported").build();

    let output = project
        .acton()
        .arg("wallet")
        .arg("import")
        .current_dir(project.path())
        .arg("--name")
        .arg("import-secure-false")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=false")
        .arg(TEST_MNEMONIC)
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_secure_false_succeeds_when_keyring_unsupported.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_import_secure_false_succeeds_when_keyring_unsupported.wallets.toml.txt",
    );
}

#[test]
fn test_wallet_import_falls_back_to_plain_mnemonic_when_keyring_unsupported() {
    let project = ProjectBuilder::new("wallet-import-secure-fallback-unsupported").build();

    let output = project
        .acton()
        .arg("wallet")
        .arg("import")
        .current_dir(project.path())
        .arg("--name")
        .arg("import-secure-fallback")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_import_falls_back_to_plain_mnemonic_when_keyring_unsupported.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet/test_wallet_import_falls_back_to_plain_mnemonic_when_keyring_unsupported.wallets.toml.txt",
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

#[cfg(unix)]
#[test]
fn test_wallet_export_mnemonic_interactive_success() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-export-mnemonic-interactive-success").build();

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

    let mut session = project
        .acton()
        .wallet_export_mnemonic()
        .arg("my-wallet")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Type wallet name to confirm mnemonic export:");
    session.send_line("my-wallet", "failed to confirm wallet name");
    session.expect(TEST_MNEMONIC);
    session.expect(Eof);
}

#[cfg(unix)]
#[test]
fn test_wallet_export_mnemonic_interactive_rejects_wrong_confirmation() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-export-mnemonic-interactive-reject").build();

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

    let mut session = project
        .acton()
        .wallet_export_mnemonic()
        .arg("my-wallet")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Type wallet name to confirm mnemonic export:");
    session.send_line("wrong-wallet", "failed to send wrong confirmation name");

    session.expect("Confirmation failed: wallet name does not match");
    session.expect(Eof);
}

#[cfg(unix)]
#[test]
fn test_wallet_export_mnemonic_interactive_wallet_not_found() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-export-mnemonic-wallet-not-found").build();

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

    let mut session = project
        .acton()
        .wallet_export_mnemonic()
        .arg("unknown-wallet")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Type wallet name to confirm mnemonic export:");
    session.send_line("unknown-wallet", "failed to confirm unknown wallet name");

    session.expect("Wallet unknown-wallet not found in wallets.toml and global.wallets.toml");
    session.expect("Available wallets:");
    session.expect("my-wallet");
    session.expect(Eof);
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
    assert!(TonCell::from_boc_hex(&signed_hex).is_ok());
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
    assert!(TonCell::from_boc_hex(signed_hex).is_ok());
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

#[cfg(unix)]
#[test]
fn test_wallet_remove_interactive_cancel_branch() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-remove-interactive-cancel").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("cancel-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let mut session = project
        .acton()
        .wallet_remove()
        .arg("cancel-wallet")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Remove wallet 'cancel-wallet'? This action cannot be undone.");
    session.send_line("No", "failed to send cancellation response");
    session.expect("Wallet removal cancelled.");
    session.expect(Eof);

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap_or_default();
    assert!(wallets_toml.contains("[wallets.cancel-wallet]"));
}

#[cfg(unix)]
#[test]
fn test_wallet_remove_without_name_selects_wallet_via_prompt() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-remove-select-wallet").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("aaa-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("zzz-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let mut session = project
        .acton()
        .wallet_remove()
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Multiple wallets configured. Please select which wallet to use:");
    session.send_line("", "failed to select default wallet");
    session.expect("Remove wallet 'aaa-wallet'? This action cannot be undone.");
    session.send_line("y", "failed to confirm wallet removal");
    session.expect("Wallet");
    session.expect("aaa-wallet");
    session.expect("removed from wallets.toml");
    session.expect(Eof);

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap_or_default();
    assert!(!wallets_toml.contains("[wallets.aaa-wallet]"));
    assert!(wallets_toml.contains("[wallets.zzz-wallet]"));
}

#[test]
fn test_wallet_remove_by_name_with_empty_wallets_toml() {
    let project = ProjectBuilder::new("wallet-remove-empty-wallets-toml").build();
    let isolated_home = project
        .path()
        .join("home-no-global-wallets")
        .to_string_lossy()
        .to_string();

    fs::write(
        project.path().join("Acton.toml"),
        format!(
            r#"[package]
name = "wallet-remove-empty-wallets-toml"
description = "A test project"
version = "0.1.0"
license = "MIT"

[wallets.config-wallet]
kind = "v5r1"
workchain = 0
keys = {{ mnemonic = "{TEST_MNEMONIC}" }}
"#
        ),
    )
    .unwrap();
    fs::write(project.path().join("wallets.toml"), "").unwrap();

    let output = project
        .acton()
        .wallet_remove()
        .arg("config-wallet")
        .arg("-y")
        .env("HOME", &isolated_home)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_remove_by_name_with_empty_wallets_toml.stderr.txt",
    );
}

#[test]
fn test_wallet_remove_by_name_with_broken_wallets_toml() {
    let project = ProjectBuilder::new("wallet-remove-broken-wallets-toml").build();

    fs::write(
        project.path().join("Acton.toml"),
        format!(
            r#"[package]
name = "wallet-remove-broken-wallets-toml"
description = "A test project"
version = "0.1.0"
license = "MIT"

[wallets.config-wallet]
kind = "v5r1"
workchain = 0
keys = {{ mnemonic = "{TEST_MNEMONIC}" }}
"#
        ),
    )
    .unwrap();
    fs::write(project.path().join("wallets.toml"), "[wallets\n").unwrap();

    let output = project
        .acton()
        .wallet_remove()
        .arg("config-wallet")
        .arg("-y")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet/test_wallet_remove_by_name_with_broken_wallets_toml.stderr.txt",
    );
}

#[cfg(feature = "only_ci")]
#[test]
#[cfg(not(target_os = "macos"))]
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
    wallets::store_mnemonic_in_keyring(&keyring_id, "secure-wallet", TEST_MNEMONIC).unwrap();

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

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_list_balance_plain_uses_mocked_toncenter() {
    let project = ProjectBuilder::new("wallet-list-balance-plain").build();

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("balance-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=false")
        .run()
        .success();

    let address = expected_testnet_address(project.path(), "balance-wallet");
    let response_body = serde_json::json!({
        "accounts": [{
            "address": address,
            "balance": "2445700000",
            "code_boc": Value::Null,
            "status": "active"
        }]
    })
    .to_string();

    let (toncenter_url, toncenter_handle, captured) =
        spawn_toncenter_v3_mock(vec![ToncenterMockResponse {
            status: 200,
            body: response_body,
        }]);

    let output = project
        .acton()
        .wallet_list()
        .arg("--balance")
        .env(TEST_TONCENTER_V3_URL_ENV, &toncenter_url)
        .run()
        .success();

    toncenter_handle.join().expect("mock toncenter must finish");

    output.assert_contains("Available wallets:");
    output.assert_contains("balance-wallet");
    output.assert_contains("2.4457 TON");

    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one accountStates request");
    assert_eq!(captured[0].method, "GET");
    assert!(
        captured[0].path.starts_with("/accountStates?address="),
        "unexpected path: {}",
        captured[0].path
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_list_balance_json_uses_env_api_key() {
    let project = ProjectBuilder::new("wallet-list-balance-json-env-api-key").build();

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("balance-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=false")
        .run()
        .success();

    let address = expected_testnet_address(project.path(), "balance-wallet");
    let response_body = serde_json::json!({
        "accounts": [{
            "address": address,
            "balance": "500000000",
            "code_boc": Value::Null,
            "status": "active"
        }]
    })
    .to_string();

    let (toncenter_url, toncenter_handle, captured) =
        spawn_toncenter_v3_mock(vec![ToncenterMockResponse {
            status: 200,
            body: response_body,
        }]);

    let output = project
        .acton()
        .wallet_list()
        .arg("--balance")
        .arg("--json")
        .env(TEST_TONCENTER_V3_URL_ENV, &toncenter_url)
        .env("TONCENTER_API_KEY", "env-api-key")
        .run()
        .success();

    toncenter_handle.join().expect("mock toncenter must finish");

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).expect("wallet list --json must return json");
    assert_eq!(json["success"], true);
    let wallets = json["wallets"].as_array().expect("wallets must be array");
    let wallet = wallets
        .iter()
        .find(|w| w["name"] == "balance-wallet")
        .expect("wallet entry must exist");
    assert_eq!(wallet["balance"], 500000000);

    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    let header = captured[0]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
        .map(|(_, value)| value.as_str());
    assert_eq!(header, Some("env-api-key"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_list_balance_cli_api_key_overrides_env() {
    let project = ProjectBuilder::new("wallet-list-balance-flag-api-key").build();

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("balance-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=false")
        .run()
        .success();

    let address = expected_testnet_address(project.path(), "balance-wallet");
    let response_body = serde_json::json!({
        "accounts": [{
            "address": address,
            "balance": "1000000000",
            "code_boc": Value::Null,
            "status": "active"
        }]
    })
    .to_string();

    let (toncenter_url, toncenter_handle, captured) =
        spawn_toncenter_v3_mock(vec![ToncenterMockResponse {
            status: 200,
            body: response_body,
        }]);

    project
        .acton()
        .wallet_list()
        .arg("--balance")
        .arg("--api-key")
        .arg("flag-api-key")
        .env(TEST_TONCENTER_V3_URL_ENV, &toncenter_url)
        .env("TONCENTER_API_KEY", "env-api-key")
        .run()
        .success();

    toncenter_handle.join().expect("mock toncenter must finish");

    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    let header = captured[0]
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("x-api-key"))
        .map(|(_, value)| value.as_str());
    assert_eq!(header, Some("flag-api-key"));
}

#[test]
fn test_wallet_list_balance_handles_toncenter_failure() {
    let project = ProjectBuilder::new("wallet-list-balance-toncenter-error").build();

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("balance-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=false")
        .run()
        .success();

    let (toncenter_url, toncenter_handle, _) =
        spawn_toncenter_v3_mock(vec![ToncenterMockResponse {
            status: 500,
            body: "{}".to_string(),
        }]);

    let output = project
        .acton()
        .wallet_list()
        .arg("--balance")
        .env(TEST_TONCENTER_V3_URL_ENV, &toncenter_url)
        .run()
        .success();

    toncenter_handle.join().expect("mock toncenter must finish");

    output.assert_contains("balance-wallet");
    output.assert_contains("0 TON");
}

#[test]
fn test_wallet_new_secure_true_uses_keyring_when_supported() {
    let project = ProjectBuilder::new("wallet-new-keyring-supported").build();
    let keyring_dir = tempfile::TempDir::new().expect("failed to create keyring temp dir");

    project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("secure-keyring-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=true")
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "1")
        .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    assert!(wallets_toml.contains("mnemonic-keyring"));
    assert!(!wallets_toml.contains("keys = { mnemonic = "));

    let files = fs::read_dir(keyring_dir.path()).expect("failed to read test keyring dir");
    assert!(
        files.count() > 0,
        "test keyring storage must contain at least one entry"
    );
}

#[test]
fn test_wallet_import_secure_true_uses_keyring_when_supported() {
    let project = ProjectBuilder::new("wallet-import-keyring-supported").build();
    let keyring_dir = tempfile::TempDir::new().expect("failed to create keyring temp dir");

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("secure-keyring-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--secure=true")
        .arg(TEST_MNEMONIC)
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "1")
        .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
        .run()
        .success();

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    assert!(wallets_toml.contains("mnemonic-keyring"));
    assert!(!wallets_toml.contains("keys = { mnemonic = "));

    let files = fs::read_dir(keyring_dir.path()).expect("failed to read test keyring dir");
    assert!(
        files.count() > 0,
        "test keyring storage must contain at least one entry"
    );
}

#[test]
fn test_wallet_secure_wallets_share_keyring_bundle_per_scope() {
    let project = ProjectBuilder::new("wallet-shared-keyring-bundle").build();
    let keyring_dir = tempfile::TempDir::new().expect("failed to create keyring temp dir");
    let (body_hex, _, signed_hex_expected) = wallet_sign_fixture_for_mnemonic(SECOND_TEST_MNEMONIC);

    for (name, mnemonic) in [
        ("alpha-wallet", TEST_MNEMONIC),
        ("beta-wallet", SECOND_TEST_MNEMONIC),
    ] {
        project
            .acton()
            .wallet_import()
            .arg("--name")
            .arg(name)
            .arg("--version")
            .arg("v5r1")
            .arg("--local")
            .arg("--secure=true")
            .arg(mnemonic)
            .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "1")
            .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
            .run()
            .success();
    }

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    let wallets_value: toml::Value = toml::from_str(&wallets_toml).unwrap();
    let alpha_keyring = wallets_value["wallets"]["alpha-wallet"]["keys"]["mnemonic-keyring"]
        .as_str()
        .expect("alpha wallet must use keyring");
    let beta_keyring = wallets_value["wallets"]["beta-wallet"]["keys"]["mnemonic-keyring"]
        .as_str()
        .expect("beta wallet must use keyring");
    assert_eq!(alpha_keyring, beta_keyring);

    let mut files: Vec<_> = fs::read_dir(keyring_dir.path())
        .expect("failed to read test keyring dir")
        .map(|entry| entry.expect("keyring dir entry"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "secure wallets in one scope must share one bundle"
    );

    let bundle_raw = fs::read_to_string(files[0].path()).expect("failed to read keyring bundle");
    let bundle: BTreeMap<String, String> =
        serde_json::from_str(&bundle_raw).expect("bundle must be valid json");
    assert_eq!(
        bundle.get("alpha-wallet").map(String::as_str),
        Some(TEST_MNEMONIC)
    );
    assert_eq!(
        bundle.get("beta-wallet").map(String::as_str),
        Some(SECOND_TEST_MNEMONIC)
    );

    let sign_output = project
        .acton()
        .wallet_sign()
        .arg("beta-wallet")
        .arg("--body")
        .arg(&body_hex)
        .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
        .run()
        .success();
    assert_eq!(sign_output.get_stdout().trim(), signed_hex_expected);

    project
        .acton()
        .wallet_remove()
        .arg("alpha-wallet")
        .arg("-y")
        .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
        .run()
        .success();

    files = fs::read_dir(keyring_dir.path())
        .expect("failed to read test keyring dir")
        .map(|entry| entry.expect("keyring dir entry"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "bundle must remain while one wallet still uses it"
    );

    let bundle_raw = fs::read_to_string(files[0].path()).expect("failed to read keyring bundle");
    let bundle: BTreeMap<String, String> =
        serde_json::from_str(&bundle_raw).expect("bundle must be valid json");
    assert!(!bundle.contains_key("alpha-wallet"));
    assert_eq!(bundle.len(), 1);
    assert_eq!(
        bundle.get("beta-wallet").map(String::as_str),
        Some(SECOND_TEST_MNEMONIC)
    );

    let sign_output = project
        .acton()
        .wallet_sign()
        .arg("beta-wallet")
        .arg("--body")
        .arg(&body_hex)
        .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
        .run()
        .success();
    assert_eq!(sign_output.get_stdout().trim(), signed_hex_expected);

    project
        .acton()
        .wallet_remove()
        .arg("beta-wallet")
        .arg("-y")
        .env(TEST_KEYRING_DIR_ENV, keyring_dir.path().to_str().unwrap())
        .run()
        .success();

    let files = fs::read_dir(keyring_dir.path())
        .expect("failed to read test keyring dir")
        .count();
    assert_eq!(
        files, 0,
        "bundle file must be deleted when the last wallet is removed"
    );
}

#[cfg(unix)]
#[test]
fn test_wallet_new_all_fields_interactive() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-new-all-fields-interactive").build();

    let mut session = project
        .acton()
        .wallet_new()
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "0")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Wallet name:");
    session.send_line("interactive-new-wallet", "failed to send wallet name");

    session.expect("Save wallet to:");
    session.send_line("", "failed to select default local destination");

    session.expect("Wallet type:");
    session.send_line("", "failed to select default wallet type");

    session.expect("Request testnet TON from faucet now?");
    session.send_line("", "failed to keep default no-airdrop option");

    session.expect("Wallet successfully created and added to");
    session.expect(Eof);

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap();
    assert!(wallets_toml.contains("[wallets.interactive-new-wallet]"));
}

#[cfg(unix)]
#[test]
fn test_wallet_sign_without_body_reads_interactive_input() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-sign-interactive-body").build();
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

    let mut session = project
        .acton()
        .wallet_sign()
        .arg("sign-wallet")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("External body BoC (hex/base64) to sign:");
    session.send_line(&body_hex, "failed to send interactive body");
    session.expect(signed_hex_expected.as_str());
    session.expect(Eof);
}

#[cfg(unix)]
#[test]
fn test_wallet_export_mnemonic_without_name_uses_single_wallet() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-export-without-name-single").build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_import()
        .arg("--name")
        .arg("single-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let mut session = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .wallet_export_mnemonic()
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Type wallet name to confirm mnemonic export:");
    session.send_line("single-wallet", "failed to confirm selected wallet");
    session.expect(TEST_MNEMONIC);
    session.expect(Eof);
}

#[cfg(unix)]
#[test]
fn test_wallet_export_mnemonic_without_name_prompts_for_wallet_selection() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-export-without-name-multiple").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("aaa-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("zzz-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let mut session = project
        .acton()
        .wallet_export_mnemonic()
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Multiple wallets configured. Please select which wallet to use:");
    session.send_line("", "failed to pick first wallet");
    session.expect("Type wallet name to confirm mnemonic export:");
    session.send_line("aaa-wallet", "failed to confirm picked wallet");
    session.expect(TEST_MNEMONIC);
    session.expect(Eof);
}

#[cfg(unix)]
#[test]
fn test_wallet_remove_json_cancel_branch() {
    use expectrl::{Eof, Regex};

    let project = ProjectBuilder::new("wallet-remove-json-cancel").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("cancel-json-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let mut session = project
        .acton()
        .wallet_remove()
        .arg("cancel-json-wallet")
        .arg("--json")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Remove wallet 'cancel-json-wallet'? This action cannot be undone.");
    session.send_line("No", "failed to reject wallet removal");
    session.expect(Regex(
        "(?s)(\"success\"\\s*:\\s*false.*\"cancelled\"\\s*:\\s*true|\"cancelled\"\\s*:\\s*true.*\"success\"\\s*:\\s*false)",
    ));
    session.expect(Eof);

    let wallets_toml = fs::read_to_string(project.path().join("wallets.toml")).unwrap_or_default();
    assert!(wallets_toml.contains("[wallets.cancel-json-wallet]"));
}

#[test]
fn test_wallet_new_rejects_invalid_keyring_support_env_value() {
    let project = ProjectBuilder::new("wallet-new-invalid-keyring-env").build();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("invalid-keyring-env-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .env(TEST_WALLET_KEYRING_SUPPORTED_ENV, "not-a-bool")
        .run()
        .failure();

    output.assert_stderr_contains("Invalid value for ACTON_TEST_WALLET_KEYRING_SUPPORTED");
}

#[test]
fn test_wallet_new_fails_with_malformed_wallets_toml() {
    let project = ProjectBuilder::new("wallet-new-malformed-wallets-toml").build();

    fs::write(project.path().join("wallets.toml"), "[wallets\n").unwrap();

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("broken-config-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .run()
        .failure();

    output.assert_stderr_contains("TOML parse error");
}

#[test]
fn test_wallet_import_fails_with_malformed_wallets_toml() {
    let project = ProjectBuilder::new("wallet-import-malformed-wallets-toml").build();

    fs::write(project.path().join("wallets.toml"), "[wallets\n").unwrap();

    let output = project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("broken-config-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .failure();

    output.assert_stderr_contains("TOML parse error");
}

#[test]
fn test_wallet_list_fails_with_malformed_wallets_toml() {
    let project = ProjectBuilder::new("wallet-list-malformed-wallets-toml").build();

    fs::write(project.path().join("wallets.toml"), "[wallets\n").unwrap();

    let output = project.acton().wallet_list().run().failure();
    output.assert_stderr_contains("TOML parse error");
}
