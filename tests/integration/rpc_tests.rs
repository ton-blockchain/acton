use crate::common::{assertion, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::{ActonCommand, Project, ProjectBuilder};
use serde_json::Value as JsonValue;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{thread, vec};
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;

const RAW_INFO_ADDRESS: &str = "0:1111111111111111111111111111111111111111111111111111111111111111";
const MATCHED_INFO_ADDRESS: &str =
    "0:2222222222222222222222222222222222222222222222222222222222222222";
const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;
const PRINT_DEPLOYER_ADDRESS_SCRIPT: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");
    println("DEPLOYER_ADDRESS={}", wallet.address);
}
"#;
const DEPLOY_COUNTER_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");
    val counterData = beginCell()
        .storeUint(7, 32)
        .storeUint(42, 32)
        .endCell();

    val counterInit = ContractState {
        code: build("counter"),
        data: counterData,
    };
    val counterAddress = AutoDeployAddress {
        stateInit: counterInit,
    }.calculateAddress();

    val deployCounter = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: counterInit,
        },
    });
    net.send(wallet.address, deployCounter);

    println("COUNTER_ADDRESS={}", counterAddress);
}
"#;

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_info_prints_remote_account_without_local_abi_match() {
    let project = ProjectBuilder::new("rpc-info-raw").build();
    let log_dir = prepare_log_dir(project.path());
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            777_000_000,
            &test_cell_boc64(0xdead_beef),
            &test_cell_boc64(0x1234_5678),
            "active",
            "",
            "17",
            "deadbeef",
        )]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(RAW_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();

    output
        .success()
        .assert_snapshot_matches("integration/snapshots/rpc/test_rpc_info_raw.stdout.txt");

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_eq!(captured[0].method, "GET");
    assert!(
        captured[0]
            .path
            .starts_with("/api/v2/getAddressInformation?address=0%3A1111111111111111111111111111111111111111111111111111111111111111"),
        "unexpected request path: {}",
        captured[0].path
    );
    assert_eq!(
        header_value(&captured[0].headers, "X-API-Key"),
        Some("custom-mock-api-key"),
        "rpc info should send TonCenter API keys for custom networks from MOCK_API_KEY",
    );
}

#[test]
fn test_rpc_info_decodes_storage_when_local_code_hash_matches() {
    let project = ProjectBuilder::new("rpc-info-storage-decode")
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .build()
        .contract("counter")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();

    let artifact_path = project.path().join("build/counter.json");
    let artifact = fs::read_to_string(&artifact_path).expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");

    let (mock_url, mock_handle, _) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, 42),
            "active",
            "",
            "999",
            "c0ffee",
        )]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_decodes_storage.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_info_skips_broken_contract_candidates_and_matches_later_contract() {
    let project = ProjectBuilder::new("rpc-info-skips-broken-candidate")
        .contract_from_boc("a_bad", vec![0x01, 0x02, 0x03])
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .build()
        .contract("counter")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();

    let artifact_path = project.path().join("build/counter.json");
    let artifact = fs::read_to_string(&artifact_path).expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");

    let (mock_url, mock_handle, _) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, 42),
            "active",
            "",
            "999",
            "c0ffee",
        )]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_decodes_storage.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_info_surfaces_malformed_manifest_errors() {
    let project = ProjectBuilder::new("rpc-info-malformed-manifest").build();
    let log_dir = prepare_log_dir(project.path());
    fs::write(
        project.path().join("Acton.toml"),
        "[package\nname = \"broken\"\n",
    )
    .expect("failed to write malformed Acton.toml");

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(RAW_INFO_ADDRESS)
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_contains("Failed to load Acton config")
        .assert_stderr_contains("TOML parse error");
}

#[test]
fn test_rpc_info_rejects_invalid_address() {
    let project = ProjectBuilder::new("rpc-info-invalid-address").build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg("not-an-address")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_invalid_address.stderr.txt",
        );
}

#[test]
fn test_rpc_info_reads_wallet_account_from_localnet() {
    let project = ProjectBuilder::new("rpc-info-localnet-wallet")
        .script_file("print_deployer_address", PRINT_DEPLOYER_ADDRESS_SCRIPT)
        .build();
    write_deployer_wallets(project.path());

    let node = start_localnet_with_localnet(&project);
    let log_dir = prepare_log_dir(project.path());

    let script_output = project
        .acton()
        .script("scripts/print_deployer_address.tolk")
        .verify_network("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();
    let script_stdout = stdout(&script_output);
    script_output.success();

    let deployer_address = extract_marker_value(&script_stdout, "DEPLOYER_ADDRESS=");

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(&deployer_address)
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();
    assert_localnet_rpc_snapshot(
        &output,
        "integration/snapshots/rpc/test_rpc_info_localnet_wallet.stdout.txt",
    );

    node.stop();
}

#[test]
fn test_rpc_info_decodes_storage_from_localnet() {
    let project = ProjectBuilder::new("rpc-info-localnet-storage")
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .script_file("deploy_counter", DEPLOY_COUNTER_SCRIPT)
        .build();
    write_deployer_wallets(project.path());

    let node = project
        .localnet()
        .before_start(ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());
    let log_dir = prepare_log_dir(project.path());

    let deploy_output = project
        .acton()
        .script("scripts/deploy_counter.tolk")
        .verify_network("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();
    let deploy_stdout = stdout(&deploy_output);
    deploy_output.success();

    let counter_address = extract_marker_value(&deploy_stdout, "COUNTER_ADDRESS=");
    wait_until_address_state_active(&node, &counter_address, Duration::from_secs(12));

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(&counter_address)
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();
    assert_localnet_rpc_snapshot(
        &output,
        "integration/snapshots/rpc/test_rpc_info_localnet_storage.stdout.txt",
    );

    node.stop();
}

#[derive(Debug, Clone)]
struct ToncenterV2MockResponse {
    status: u16,
    body: String,
}

#[derive(Debug, Clone)]
struct CapturedToncenterV2Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn spawn_toncenter_v2_mock(
    responses: Vec<ToncenterV2MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterV2Request>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind TonCenter mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set TonCenter mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get TonCenter mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedToncenterV2Request>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for response in responses {
            let wait_until = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for TonCenter request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("TonCenter mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set TonCenter mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone TonCenter mock stream"),
            );

            let request_line = read_request_line(&mut reader);
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_owned();
            let path = parts.next().unwrap_or_default().to_owned();
            let headers = read_headers(&mut reader);

            captured_requests_thread
                .lock()
                .expect("captured requests mutex should not be poisoned")
                .push(CapturedToncenterV2Request {
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
                .expect("failed to write TonCenter response");
            stream.flush().expect("failed to flush TonCenter response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

fn read_request_line(reader: &mut BufReader<std::net::TcpStream>) -> String {
    let mut request_line = String::new();
    let read_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        request_line.clear();
        match reader.read_line(&mut request_line) {
            Ok(0) => {
                assert!(
                    Instant::now() <= read_deadline,
                    "timed out waiting for TonCenter request line"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Ok(_) => return request_line,
            Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                assert!(
                    Instant::now() <= read_deadline,
                    "timed out waiting for TonCenter request line"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("failed to read TonCenter request line: {err}"),
        }
    }
}

fn read_headers(reader: &mut BufReader<std::net::TcpStream>) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    loop {
        let mut header_line = String::new();
        let read = reader
            .read_line(&mut header_line)
            .expect("failed to read TonCenter header line");
        if read == 0 || header_line == "\r\n" {
            return headers;
        }

        if let Some((name, value)) = header_line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
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

fn toncenter_v2_account_info_ok_response(
    balance: i64,
    code_boc64: &str,
    data_boc64: &str,
    state: &str,
    frozen_hash: &str,
    lt: &str,
    hash: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "balance": balance.to_string(),
                "code": code_boc64,
                "data": data_boc64,
                "state": state,
                "frozen_hash": frozen_hash,
                "last_transaction_id": {
                    "lt": lt,
                    "hash": hash,
                }
            }
        })
        .to_string(),
    }
}

fn test_cell_boc64(value: u32) -> String {
    let mut builder = CellBuilder::new();
    builder.store_u32(value).expect("must store u32");
    let cell = builder.build().expect("must build cell");
    Boc::encode_base64(&cell)
}

fn counter_storage_boc64(id: u32, counter: u32) -> String {
    let mut builder = CellBuilder::new();
    builder.store_u32(id).expect("must store id");
    builder.store_u32(counter).expect("must store counter");
    let cell = builder.build().expect("must build storage cell");
    Boc::encode_base64(&cell)
}

fn write_custom_network_config(project_root: &Path, name: &str, url: &str) {
    let config_path = project_root.join("Acton.toml");
    let mut config = fs::read_to_string(&config_path).expect("Acton.toml must exist");
    config.push_str(&format!(
        "\n[networks.{name}]\napi = {{ v2 = \"{url}/api/v2\" }}\n"
    ));
    fs::write(config_path, config).expect("failed to update Acton.toml");
}

fn write_deployer_wallets(project_root: &Path) {
    fs::write(project_root.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
}

fn start_localnet_with_localnet(project: &Project) -> crate::support::localnet::LocalnetHandle {
    let node = project.localnet().args(["--accounts", "deployer"]).start();
    append_localnet_network(project.path(), &node.base_url());
    node
}

fn append_localnet_network(project_path: &Path, base_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r#"

[networks.localnet]
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
"#
    ));
    fs::write(&acton_toml_path, acton_toml).expect("failed to write Acton.toml with localnet");
}

fn stdout(output: &crate::support::assertions::TestOutput) -> String {
    String::from_utf8(output.output.get_output().stdout.clone())
        .expect("command stdout must be utf-8")
}

fn extract_marker_value(output: &str, marker: &str) -> String {
    let cleaned = strip_ansi(output);
    cleaned
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(marker).map(ToOwned::to_owned))
        .unwrap_or_else(|| panic!("Marker `{marker}` not found in output:\n{cleaned}"))
}

fn wait_until_address_state_active(
    node: &crate::support::localnet::LocalnetHandle,
    address: &str,
    timeout: Duration,
) {
    let query = format!("/api/v2/getAddressState?address={address}");
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json(&query);
        if response["ok"].as_bool() == Some(true) && response["result"].as_str() == Some("active") {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for address `{address}` to become active:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn prepare_log_dir(project_root: &Path) -> String {
    let log_dir = project_root.join(".acton-logs");
    fs::create_dir_all(&log_dir).expect("must create log dir");
    log_dir.to_string_lossy().into_owned()
}

fn assert_localnet_rpc_snapshot(
    output: &crate::support::assertions::TestSuccess,
    snapshot_path: &str,
) {
    let normalized = normalize_localnet_rpc_stdout(&output.get_normalized_stdout());
    let expected_path = Path::new("tests").join(snapshot_path);
    let expected =
        fs::read_to_string(&expected_path).expect("localnet rpc snapshot file must exist");
    assertion().eq(normalized, expected);
}

fn normalize_localnet_rpc_stdout(stdout: &str) -> String {
    let mut normalized_lines = Vec::new();
    for line in stdout.lines() {
        if let Some((prefix, _)) = line.split_once("Last Tx Hash:") {
            normalized_lines.push(format!("{prefix}Last Tx Hash:      [TX_HASH]"));
        } else {
            normalized_lines.push(line.to_owned());
        }
    }
    let mut normalized = normalized_lines.join("\n");
    if stdout.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}
