use crate::common::strip_ansi;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const TEST_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";
const MOCK_FAUCET_ACCEPT_TIMEOUT: Duration = Duration::from_secs(20);
const MOCK_FAUCET_READ_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
struct FaucetMockResponse {
    method: &'static str,
    path: &'static str,
    status: u16,
    body: &'static str,
}

#[derive(Debug, Clone)]
struct CapturedRequest {
    method: String,
    path: String,
    body: String,
}

fn spawn_faucet_mock(
    responses: Vec<FaucetMockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedRequest>>>,
) {
    let (port, handle, captured_requests) = spawn_http_mock(responses);
    (
        format!("http://127.0.0.1:{port}/faucet"),
        handle,
        captured_requests,
    )
}

fn spawn_http_mock(
    responses: Vec<FaucetMockResponse>,
) -> (
    u16,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedRequest>>>,
) {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind mock faucet listener");
    listener
        .set_nonblocking(true)
        .expect("failed to set non-blocking mode for mock faucet listener");
    let port = listener
        .local_addr()
        .expect("failed to get mock faucet listener address")
        .port();

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedRequest>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for response in responses {
            let wait_until = Instant::now() + MOCK_FAUCET_ACCEPT_TIMEOUT;
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for request {} {}",
                            response.method,
                            response.path
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("mock faucet accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(MOCK_FAUCET_READ_TIMEOUT))
                .expect("failed to set mock faucet read timeout");

            let mut reader = BufReader::new(stream.try_clone().expect("failed to clone stream"));
            let mut request_line = String::new();
            let read_deadline = Instant::now() + MOCK_FAUCET_READ_TIMEOUT;
            loop {
                request_line.clear();
                match reader.read_line(&mut request_line) {
                    Ok(0) => {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for mock faucet request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => break,
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for mock faucet request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("failed to read mock faucet request line: {err}"),
                }
            }

            assert!(
                !request_line.is_empty(),
                "mock faucet received empty request line"
            );

            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();

            let mut content_length = 0_usize;
            loop {
                let mut header_line = String::new();
                let read = reader
                    .read_line(&mut header_line)
                    .expect("failed to read mock faucet header line");
                if read == 0 || header_line == "\r\n" {
                    break;
                }

                if header_line
                    .to_ascii_lowercase()
                    .starts_with("content-length:")
                {
                    let len_value = header_line
                        .split_once(':')
                        .map_or("", |(_, value)| value)
                        .trim();
                    content_length = len_value.parse().unwrap_or(0);
                }
            }

            let mut request_body = Vec::<u8>::new();
            if content_length > 0 {
                request_body.resize(content_length, 0_u8);
                reader
                    .read_exact(&mut request_body)
                    .expect("failed to read mock faucet request body");
            }

            captured_requests_thread
                .lock()
                .expect("captured requests mutex poisoned")
                .push(CapturedRequest {
                    method: method.clone(),
                    path: path.clone(),
                    body: String::from_utf8_lossy(&request_body).into_owned(),
                });

            assert_eq!(method, response.method, "unexpected HTTP method");
            assert_eq!(path, response.path, "unexpected HTTP path");

            let body = response.body;
            let raw_response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                status_text(response.status),
                body.len(),
                body
            );
            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write mock faucet response");
            stream
                .flush()
                .expect("failed to flush mock faucet response");
        }
    });

    (port, handle, captured_requests)
}

fn spawn_localnet_faucet_mock(
    responses: Vec<FaucetMockResponse>,
) -> (
    u16,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedRequest>>>,
) {
    let (port, handle, captured_requests) = spawn_http_mock(responses);
    (port, handle, captured_requests)
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

fn append_litenode_port(project_path: &Path, port: u16) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("Failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r"

[litenode]
port = {port}
"
    ));
    fs::write(&acton_toml_path, acton_toml).expect("Failed to write Acton.toml with litenode port");
}

fn find_unused_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("failed to reserve test port")
        .local_addr()
        .expect("failed to inspect reserved test port")
        .port()
}

fn parse_airdrop_wallet_address(stdout: &str, wallet_name: &str) -> String {
    let prefix = format!("→ Requesting airdrop for wallet {wallet_name} ");
    strip_ansi(stdout)
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix).map(ToOwned::to_owned))
        .unwrap_or_else(|| {
            panic!(
                "Airdrop address line not found in output:\n{}",
                strip_ansi(stdout)
            )
        })
}

fn parse_address_balance(address_information: &Value) -> u128 {
    address_information["result"]["balance"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "Expected string balance field in getAddressInformation response:\n{}",
                serde_json::to_string_pretty(address_information).unwrap_or_default()
            )
        })
        .parse::<u128>()
        .unwrap_or_else(|e| {
            panic!(
                "Failed to parse balance from getAddressInformation response: {e}\n{}",
                serde_json::to_string_pretty(address_information).unwrap_or_default()
            )
        })
}

#[test]
fn test_wallet_airdrop_rejects_difficulty_above_256() {
    let project = ProjectBuilder::new("wallet-airdrop-invalid-difficulty").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![FaucetMockResponse {
        method: "GET",
        path: "/faucet/challenge",
        status: 200,
        body: r#"{"challenge":"mock-challenge","difficulty":257}"#,
    }]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_rejects_difficulty_above_256.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_json_error_exits_non_zero() {
    let project = ProjectBuilder::new("wallet-airdrop-json-error").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"mock-challenge","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 500,
            body: r#"{"error":"faucet down"}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 500,
            body: r#"{"error":"faucet down"}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 500,
            body: r#"{"error":"faucet down"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");

    assert_eq!(json["success"], false);
    assert!(
        json["error"]
            .as_str()
            .expect("error must be a string")
            .contains("Faucet returned error 500 Internal Server Error: faucet down")
    );
}

#[test]
fn test_wallet_airdrop_json_success_with_message() {
    let project = ProjectBuilder::new("wallet-airdrop-json-success").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"challenge-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"Airdrop complete"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");

    assert_eq!(json["success"], true);
    assert_eq!(json["message"], "Airdrop complete");
    assert!(
        json["address"]
            .as_str()
            .is_some_and(|address| !address.is_empty())
    );
    assert_eq!(json["difficulty"], 0);
    assert_eq!(json["nonce"], 0);
    assert!(json["solve_ms"].as_u64().is_some());
}

#[test]
fn test_wallet_airdrop_json_success_without_message_uses_default() {
    let project = ProjectBuilder::new("wallet-airdrop-json-success-default-message").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"challenge-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r"{}",
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");

    assert_eq!(json["success"], true);
    assert_eq!(json["message"], "Success");
    assert!(
        json["address"]
            .as_str()
            .is_some_and(|address| !address.is_empty())
    );
    assert_eq!(json["difficulty"], 0);
    assert_eq!(json["nonce"], 0);
    assert!(json["solve_ms"].as_u64().is_some());
}

#[test]
fn test_wallet_airdrop_non_json_success_outputs_human_readable_message() {
    let project = ProjectBuilder::new("wallet-airdrop-non-json-success").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"human-readable-success","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"Airdrop complete"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_contains("Requesting airdrop for wallet airdrop-wallet");
    output.assert_contains("Fetching PoW challenge...");
    output.assert_contains("Solving challenge (difficulty: 0 bits)...");
    output.assert_contains("Airdrop complete");
    output.assert_not_contains("\"success\": true");
}

#[test]
fn test_wallet_airdrop_localnet_success_uses_configured_port_and_fixed_amount() {
    let project = ProjectBuilder::new("wallet-airdrop-localnet-success").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let node = project.litenode().start();
    append_litenode_port(project.path(), node.port());

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--net")
        .arg("localnet")
        .run()
        .success();

    let stdout = output.get_stdout();
    let address = parse_airdrop_wallet_address(&stdout, "airdrop-wallet");
    output.assert_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_localnet_success_uses_configured_port_and_fixed_amount.stdout.txt",
    );

    let address_info = node.get_json(&format!("/api/v2/getAddressInformation?address={address}"));
    assert_eq!(parse_address_balance(&address_info), 100_000_000_000);

    node.stop();
}

#[test]
fn test_wallet_airdrop_localnet_transport_error_without_running_node() {
    let project = ProjectBuilder::new("wallet-airdrop-localnet-transport-error").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    append_litenode_port(project.path(), find_unused_port());

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--net")
        .arg("localnet")
        .run()
        .failure();

    output.assert_stderr_contains(
        "Failed to send request to localnet faucet. Make sure `acton litenode start` is running",
    );
}

#[test]
fn test_wallet_airdrop_localnet_http_error_preserves_response_body() {
    let project = ProjectBuilder::new("wallet-airdrop-localnet-http-error").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (port, faucet_handle, _) = spawn_localnet_faucet_mock(vec![FaucetMockResponse {
        method: "POST",
        path: "/admin/faucet",
        status: 500,
        body: r#"{"error":"backend unavailable"}"#,
    }]);
    append_litenode_port(project.path(), port);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--net")
        .arg("localnet")
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock localnet faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_localnet_http_error_preserves_response_body.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_localnet_invalid_json_response_reports_parse_error() {
    let project = ProjectBuilder::new("wallet-airdrop-localnet-invalid-json").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (port, faucet_handle, _) = spawn_localnet_faucet_mock(vec![FaucetMockResponse {
        method: "POST",
        path: "/admin/faucet",
        status: 200,
        body: "not json",
    }]);
    append_litenode_port(project.path(), port);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--net")
        .arg("localnet")
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock localnet faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_localnet_invalid_json_response_reports_parse_error.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_localnet_json_success_omits_pow_fields() {
    let project = ProjectBuilder::new("wallet-airdrop-localnet-json-success").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let node = project.litenode().start();
    append_litenode_port(project.path(), node.port());

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--net")
        .arg("localnet")
        .arg("--json")
        .run()
        .success();

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");

    assert_eq!(json["success"], true);
    assert_eq!(
        json["message"],
        "Successfully airdropped 100 TON on localnet"
    );
    assert!(
        json["address"]
            .as_str()
            .is_some_and(|address| !address.is_empty())
    );
    assert!(json.get("difficulty").is_none());
    assert!(json.get("nonce").is_none());
    assert!(json.get("solve_ms").is_none());

    node.stop();
}

#[test]
fn test_wallet_airdrop_localnet_rejects_faucet_url_override() {
    let project = ProjectBuilder::new("wallet-airdrop-localnet-faucet-url").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--net")
        .arg("localnet")
        .arg("--faucet-url")
        .arg("https://example.com/faucet")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_localnet_rejects_faucet_url_override.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_wallet_not_found() {
    let project = ProjectBuilder::new("wallet-airdrop-wallet-not-found").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("known-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("missing-wallet")
        .run()
        .failure();

    output.assert_stderr_contains(
        "Wallet missing-wallet not found in wallets.toml and global.wallets.toml",
    );
    output.assert_stderr_contains("Available wallets:");
    output.assert_stderr_contains("known-wallet");
}

#[cfg(unix)]
#[test]
fn test_wallet_airdrop_without_name_fails_when_no_wallets_config_interactive() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-airdrop-no-wallets-config").build();
    let isolated_home = project
        .path()
        .join("home-no-global-wallets")
        .to_string_lossy()
        .to_string();

    let mut session = project
        .acton()
        .wallet_airdrop()
        .env("HOME", &isolated_home)
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("No wallets configured in wallets.toml or global.wallets.toml.");
    session.expect("To add a wallet use acton wallet new");
    session.expect(
        "See https://ton-blockchain.github.io/acton/docs/setup-wallets/ for more information",
    );
    session.expect(Eof);
}

#[cfg(unix)]
#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_airdrop_without_name_selects_wallet_via_prompt() {
    use expectrl::Eof;

    let project = ProjectBuilder::new("wallet-airdrop-select-wallet").build();
    let isolated_home = project
        .path()
        .join("home-no-global-wallets")
        .to_string_lossy()
        .to_string();

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

    let (faucet_url, faucet_handle, captured_requests) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"select-wallet-success","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"selected wallet airdrop ok"}"#,
        },
    ]);

    let mut session = project
        .acton()
        .wallet_airdrop()
        .arg("--faucet-url")
        .arg(&faucet_url)
        .env("HOME", &isolated_home)
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(20)));

    session.expect("Multiple wallets configured. Please select which wallet to use:");
    session.send_line("", "failed to select default wallet for airdrop");
    session.expect("Requesting airdrop for wallet aaa-wallet");
    session.expect("selected wallet airdrop ok");
    session.expect(Eof);

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let captured = captured_requests
        .lock()
        .expect("captured requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected challenge and claim requests");
}

#[test]
fn test_wallet_airdrop_rate_limit_uses_friendly_error_message() {
    let project = ProjectBuilder::new("wallet-airdrop-rate-limit").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"challenge-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 429,
            body: r#"{"error":"rate limit"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_rate_limit_uses_friendly_error_message.stderr.txt",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_airdrop_claim_request_contains_challenge_nonce_and_address() {
    let project = ProjectBuilder::new("wallet-airdrop-claim-payload").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, captured_requests) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"payload-check","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"ok"}"#,
        },
    ]);

    project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let captured = captured_requests
        .lock()
        .expect("captured requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected challenge and claim requests");

    let claim_request = captured
        .iter()
        .find(|req| req.method == "POST" && req.path == "/faucet/claim")
        .expect("claim request not captured");

    let claim_body: Value =
        serde_json::from_str(&claim_request.body).expect("claim body must be valid JSON");

    assert_eq!(claim_body["challenge"], "payload-check");
    assert!(claim_body["nonce"].as_u64().is_some(), "nonce must be u64");
    assert!(
        claim_body["address"]
            .as_str()
            .is_some_and(|address| !address.is_empty()),
        "address must be non-empty string"
    );
}

#[test]
fn test_wallet_airdrop_challenge_http_error_with_body() {
    let project = ProjectBuilder::new("wallet-airdrop-challenge-http-error-body").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: r#"{"error":"backend unavailable"}"#,
        },
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: r#"{"error":"backend unavailable"}"#,
        },
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: r#"{"error":"backend unavailable"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_challenge_http_error_with_body.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_challenge_http_error_without_body() {
    let project = ProjectBuilder::new("wallet-airdrop-challenge-http-error-empty").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: "",
        },
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: "",
        },
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: "",
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_challenge_http_error_without_body.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_challenge_invalid_json_response() {
    let project = ProjectBuilder::new("wallet-airdrop-challenge-invalid-json").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![FaucetMockResponse {
        method: "GET",
        path: "/faucet/challenge",
        status: 200,
        body: r#"{"challenge":"only-challenge"}"#,
    }]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_challenge_invalid_json_response.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_claim_success_invalid_json_response() {
    let project = ProjectBuilder::new("wallet-airdrop-claim-success-invalid-json").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"challenge-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: "not-json",
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_claim_success_invalid_json_response.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_claim_error_uses_message_fallback() {
    let project = ProjectBuilder::new("wallet-airdrop-claim-error-message-fallback").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"challenge-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 400,
            body: r#"{"message":"temporary failure"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_claim_error_uses_message_fallback.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_claim_error_uses_raw_body_fallback() {
    let project = ProjectBuilder::new("wallet-airdrop-claim-error-raw-fallback").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"challenge-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 400,
            body: "plain error text",
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_claim_error_uses_raw_body_fallback.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_rejects_invalid_faucet_url_scheme() {
    let project = ProjectBuilder::new("wallet-airdrop-invalid-url-scheme").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg("ftp://example.com/faucet")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_rejects_invalid_faucet_url_scheme.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_rejects_empty_faucet_url() {
    let project = ProjectBuilder::new("wallet-airdrop-empty-url").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg("")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_rejects_empty_faucet_url.stderr.txt",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_airdrop_retries_challenge_request_after_server_error() {
    let project = ProjectBuilder::new("wallet-airdrop-retry-challenge").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, captured_requests) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 500,
            body: r#"{"error":"transient"}"#,
        },
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"retry-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"ok"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");
    assert_eq!(json["success"], true);

    let captured = captured_requests
        .lock()
        .expect("captured requests mutex poisoned");
    let challenge_attempts = captured
        .iter()
        .filter(|req| req.method == "GET" && req.path == "/faucet/challenge")
        .count();
    assert_eq!(
        challenge_attempts, 2,
        "challenge request must be retried once"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_airdrop_retries_claim_request_after_server_error() {
    let project = ProjectBuilder::new("wallet-airdrop-retry-claim").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, captured_requests) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"retry-claim","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 500,
            body: r#"{"error":"transient"}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"ok"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");
    assert_eq!(json["success"], true);

    let captured = captured_requests
        .lock()
        .expect("captured requests mutex poisoned");
    let claim_attempts = captured
        .iter()
        .filter(|req| req.method == "POST" && req.path == "/faucet/claim")
        .count();
    assert_eq!(claim_attempts, 2, "claim request must be retried once");
}

#[test]
fn test_wallet_airdrop_json_outputs_error_for_challenge_parse_failures() {
    let project = ProjectBuilder::new("wallet-airdrop-json-challenge-parse-error").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![FaucetMockResponse {
        method: "GET",
        path: "/faucet/challenge",
        status: 200,
        body: r#"{"challenge":"only-challenge"}"#,
    }]);

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg(&faucet_url)
        .arg("--json")
        .run()
        .failure();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");
    assert_eq!(json["success"], false);
    assert!(
        json["error"]
            .as_str()
            .expect("error must be a string")
            .contains("Failed to parse challenge response")
    );
}

#[test]
fn test_wallet_airdrop_json_outputs_error_for_invalid_faucet_url() {
    let project = ProjectBuilder::new("wallet-airdrop-json-invalid-url").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg("ftp://example.com/faucet")
        .arg("--json")
        .run()
        .failure();

    let stdout = output.get_stdout();
    let json: Value =
        serde_json::from_str(stdout.trim()).expect("airdrop --json output must be valid JSON");
    assert_eq!(json["success"], false);
    assert!(
        json["error"]
            .as_str()
            .expect("error must be a string")
            .contains("Faucet URL scheme must be http or https")
    );
}

#[test]
fn test_wallet_airdrop_rejects_faucet_url_with_query() {
    let project = ProjectBuilder::new("wallet-airdrop-invalid-url-query").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg("https://example.com/faucet?token=123")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_rejects_faucet_url_with_query.stderr.txt",
    );
}

#[test]
fn test_wallet_airdrop_rejects_faucet_url_with_fragment() {
    let project = ProjectBuilder::new("wallet-airdrop-invalid-url-fragment").build();

    project
        .acton()
        .wallet_import()
        .arg("--name")
        .arg("airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg(TEST_MNEMONIC)
        .run()
        .success();

    let output = project
        .acton()
        .wallet_airdrop()
        .arg("airdrop-wallet")
        .arg("--faucet-url")
        .arg("https://example.com/faucet#frag")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_airdrop_rejects_faucet_url_with_fragment.stderr.txt",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_wallet_new_airdrop_uses_env_faucet_url_success_non_json() {
    let project = ProjectBuilder::new("wallet-new-airdrop-success").build();

    let (faucet_url, faucet_handle, captured_requests) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"new-wallet-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"mock airdrop success"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("new-airdrop-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--airdrop")
        .env("ACTON_FAUCET_URL", &faucet_url)
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_new_airdrop_uses_env_faucet_url_success_non_json.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet_airdrop/test_wallet_new_airdrop_uses_env_faucet_url_success_non_json.wallets.toml.txt",
    );

    let captured = captured_requests
        .lock()
        .expect("captured requests mutex poisoned");
    let challenge_attempts = captured
        .iter()
        .filter(|req| req.method == "GET" && req.path == "/faucet/challenge")
        .count();
    let claim_attempts = captured
        .iter()
        .filter(|req| req.method == "POST" && req.path == "/faucet/claim")
        .count();
    assert_eq!(challenge_attempts, 1);
    assert_eq!(claim_attempts, 1);
}

#[test]
fn test_wallet_new_airdrop_failure_keeps_wallet_and_prints_warning() {
    let project = ProjectBuilder::new("wallet-new-airdrop-fail").build();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"new-wallet-fail","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 400,
            body: r#"{"error":"faucet down"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("new-airdrop-fail-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--airdrop")
        .env("ACTON_FAUCET_URL", &faucet_url)
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    output.assert_snapshot_matches(
        "integration/snapshots/wallet_airdrop/test_wallet_new_airdrop_failure_keeps_wallet_and_prints_warning.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        "wallets.toml",
        "integration/snapshots/wallet_airdrop/test_wallet_new_airdrop_failure_keeps_wallet_and_prints_warning.wallets.toml.txt",
    );
}

#[test]
fn test_wallet_new_airdrop_json_success_has_airdrop_block() {
    let project = ProjectBuilder::new("wallet-new-airdrop-json-success").build();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"new-wallet-json-ok","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 200,
            body: r#"{"message":"json airdrop ok"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("new-airdrop-json-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--airdrop")
        .arg("--json")
        .env("ACTON_FAUCET_URL", &faucet_url)
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).expect("wallet new --json must return JSON");
    assert_eq!(json["success"], true);
    assert_eq!(json["airdrop_requested"], true);
    assert_eq!(json["airdrop"]["success"], true);
    assert_eq!(json["airdrop"]["message"], "json airdrop ok");
    assert!(json["airdrop"]["address"].is_string());
}

#[test]
fn test_wallet_new_airdrop_json_failure_has_airdrop_error_block() {
    let project = ProjectBuilder::new("wallet-new-airdrop-json-fail").build();

    let (faucet_url, faucet_handle, _) = spawn_faucet_mock(vec![
        FaucetMockResponse {
            method: "GET",
            path: "/faucet/challenge",
            status: 200,
            body: r#"{"challenge":"new-wallet-json-fail","difficulty":0}"#,
        },
        FaucetMockResponse {
            method: "POST",
            path: "/faucet/claim",
            status: 400,
            body: r#"{"error":"claim failed"}"#,
        },
    ]);

    let output = project
        .acton()
        .wallet_new()
        .arg("--name")
        .arg("new-airdrop-json-fail-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .arg("--airdrop")
        .arg("--json")
        .env("ACTON_FAUCET_URL", &faucet_url)
        .run()
        .success();

    faucet_handle
        .join()
        .expect("mock faucet thread must finish without panic");

    let stdout = output.get_stdout();
    let json: Value = serde_json::from_str(&stdout).expect("wallet new --json must return JSON");
    assert_eq!(json["success"], true);
    assert_eq!(json["airdrop_requested"], true);
    assert_eq!(json["airdrop"]["success"], false);
    assert!(
        json["airdrop"]["error"]
            .as_str()
            .expect("airdrop.error must be a string")
            .contains("Faucet returned error 400 Bad Request: claim failed")
    );
}
