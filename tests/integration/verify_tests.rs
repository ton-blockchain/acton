use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use crate::support::toncenter::spawn_toncenter_v2_mock_with_capture;
use crate::support::toncenter::{
    toncenter_v2_send_boc_client_error_response, toncenter_v2_send_boc_ok_response,
    toncenter_v2_verify_quorum_response, toncenter_v2_verify_registry_address_response,
};
use crate::support::verifier::{VerifierMockResponse, spawn_verifier_mock};
#[cfg(unix)]
use expectrl::Eof;
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;

const MULTI_WALLET_CONFIG: &str = r#"[wallets.alpha]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }

[wallets.beta]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;

const VERIFY_TEST_ADDRESS: &str = "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot";
const TEST_TONCENTER_MAINNET_V2_URL_ENV: &str = "ACTON_TEST_TONCENTER_MAINNET_V2_URL";
const TEST_TONCENTER_TESTNET_V2_URL_ENV: &str = "ACTON_TEST_TONCENTER_TESTNET_V2_URL";
const VERIFY_BACKENDS_ENV: &str = "ACTON_VERIFY_BACKENDS";
const VERIFY_TEST_REGISTRY_ADDRESS: &str = "EQD-BJSVUJviud_Qv7Ymfd3qzXdrmV525e3YDzWQoHIAiInL";
const VERIFY_TEST_API_KEY: &str = "verify-test-api-key";

static VERIFY_BACKEND_MOCK_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn write_deployer_wallets(project_path: &Path) {
    std::fs::write(project_path.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
}

fn write_multiple_wallets(project_path: &Path) {
    std::fs::write(project_path.join("wallets.toml"), MULTI_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
}

fn replace_contract_display_name(project_path: &Path, from: &str, to: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let acton_toml = std::fs::read_to_string(&acton_toml_path).expect("failed to read Acton.toml");
    let updated = acton_toml.replace(
        &format!("display-name = \"{from}\""),
        &format!("display-name = \"{to}\""),
    );
    std::fs::write(&acton_toml_path, updated).expect("failed to write Acton.toml");
}

fn build_verify_backend_project(name: &str) -> Project {
    let project = ProjectBuilder::new(name)
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    write_deployer_wallets(project.path());
    project
}

fn verify_backend_mock_guard() -> std::sync::MutexGuard<'static, ()> {
    VERIFY_BACKEND_MOCK_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn valid_message_cell_boc() -> Vec<u8> {
    let mut builder = CellBuilder::new();
    builder
        .store_u8(0xAB)
        .expect("message byte must store into cell");
    let body = builder.build().expect("message cell must build");
    Boc::encode(body)
}

#[test]
fn test_verify_contract_not_found() {
    let project = ProjectBuilder::new("verify-contract-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("nonexistent")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_contract_not_found.stderr.txt",
        );
}

#[test]
fn test_verify_contract_display_name_shows_contract_id_hint() {
    let project = ProjectBuilder::new("verify-contract-display-name-hint")
        .contract("simple_id", SIMPLE_CONTRACT)
        .build();
    replace_contract_display_name(project.path(), "simple_id", "Visible Simple");

    project
        .acton()
        .verify()
        .verify_contract("Visible Simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_contract_display_name_shows_contract_id_hint.stderr.txt",
        );
}

#[test]
fn test_verify_boc_file() {
    let project = ProjectBuilder::new("verify-boc-file")
        .raw_file("contracts/contract.boc", "some boc content")
        .build();

    let toml_content = r#"[package]
name = "verify-boc-file"
description = ""
version = "0.1.0"

[contracts.contract]
display-name = "contract"
src = "contracts/contract.boc"
depends = []
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .verify_contract("contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/test_verify_boc_file.stderr.txt");
}

#[test]
fn test_verify_non_tolk_file() {
    let project = ProjectBuilder::new("verify-non-tolk-file")
        .raw_file("contracts/contract.fc", "some func content")
        .build();

    let toml_content = r#"[package]
name = "verify-non-tolk-file"
description = ""
version = "0.1.0"

[contracts.contract]
display-name = "contract"
src = "contracts/contract.fc"
depends = []
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .verify_contract("contract")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_non_tolk_file.stderr.txt",
        );
}

#[test]
fn test_verify_invalid_network() {
    let project = ProjectBuilder::new("verify-invalid-net")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_network("invalid-network")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_invalid_network.stderr.txt",
        );
}

#[test]
fn test_verify_unsupported_network() {
    let project = ProjectBuilder::new("verify-unsupported-net")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_network("custom:no-such-net")
        .arg("--dry-run")
        .run()
        .failure()
        .assert_not_contains("Using wallet")
        .assert_not_contains("Fetching backends configuration")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_unsupported_network.stderr.txt",
        );
}

#[test]
fn test_verify_invalid_address() {
    let project = ProjectBuilder::new("verify-invalid-addr")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_address("invalid-address-format")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_invalid_address.stderr.txt",
        );
}

#[test]
fn test_verify_base64_address() {
    let project = ProjectBuilder::new("verify-base64-address")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .verify()
        .verify_contract("simple")
        .verify_address("kQCPzICFdKkkWB7Bs4MSVzf8cHU52+MOyScFB2ARtaF37Vl5")
        .wallet("nonexistent-wallet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_base64_address.stderr.txt",
        );
}

#[test]
fn test_verify_wallet_not_found_without_wallets() {
    let project = ProjectBuilder::new("verify-wallet-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .verify()
        .verify_contract("simple")
        .verify_address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .wallet("nonexistent-wallet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_wallet_not_found_without_wallets.stderr.txt",
        );
}

#[test]
fn test_verify_wallet_not_found() {
    let project = ProjectBuilder::new("verify-wallet-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let toml_content = r#"[package]
name = "verify-contracts"
description = ""
version = "0.1.0"

[contracts.simple]
display-name = "Simple"
src = "contracts/simple.tolk"
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    let wallets_toml = r#"[wallets.deployer]
kind = "v5r1"
workchain = 0
keys = { mnemonic-file = "Acton.toml" }
"#;
    std::fs::write(project.path().join("wallets.toml"), wallets_toml).expect("Write wallets.toml");

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot")
        .wallet("nonexistent-wallet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_wallet_not_found.stderr.txt",
        );
}

#[test]
fn test_verify_compilation_error() {
    let project = ProjectBuilder::new("verify-compilation-error")
        .contract(
            "broken",
            r"
            fun onInternalMessage(in: InMessage) {
                val x = nonexistent_symbol();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .build();

    project
        .acton()
        .verify()
        .verify_contract("broken")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_compilation_error.stderr.txt",
        );
}

#[test]
fn test_verify_no_contracts_configured() {
    let project = ProjectBuilder::new("verify-no-contracts").build();

    let toml_content = r#"[package]
name = "verify-no-contracts"
description = ""
version = "0.1.0"
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_no_contracts_configured.stderr.txt",
        );
}

#[test]
fn test_verify_empty_contracts_section() {
    let project = ProjectBuilder::new("verify-empty-contracts").build();

    let toml_content = r#"[package]
name = "verify-empty-contracts"
description = ""
version = "0.1.0"

[contracts]
"#;
    std::fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .verify()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_verify_empty_contracts_section.stderr.txt",
        );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_backend_client_error_reports_response_body() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-backend-client-error");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 400,
        body: serde_json::json!({
            "error": "mock backend rejected sources"
        })
        .to_string(),
        headers: vec![],
    }]);
    let backend_url = format!("{mock_url}/");

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &backend_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_verify_backend_client_error_reports_response_body.stderr.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one verifier request");
    assert_eq!(captured[0].method, "POST");
    assert_eq!(captured[0].path, "/source");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_backend_retries_after_server_error() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-backend-retry");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        VerifierMockResponse {
            status: 500,
            body: "temporary verifier outage".to_string(),
            headers: vec![("cf-ray".to_string(), "retry-please".to_string())],
        },
        VerifierMockResponse {
            status: 400,
            body: "backend rejected after retry".to_string(),
            headers: vec![],
        },
    ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_backend_retries_after_server_error.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_verify_backend_retries_after_server_error.stderr.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected retry to hit verifier twice");
    assert!(captured.iter().all(|request| request.path == "/source"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_backend_retries_then_proof_already_deployed_returns_success() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-backend-retry-proof-already-deployed");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        VerifierMockResponse {
            status: 500,
            body: "temporary verifier outage".to_string(),
            headers: vec![("cf-ray".to_string(), "retry-please".to_string())],
        },
        VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "different",
                    "error": "Proof has already been deployed"
                }
            })
            .to_string(),
            headers: vec![],
        },
    ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_backend_retries_then_proof_already_deployed_returns_success.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected retry to hit verifier twice");
    assert!(captured.iter().all(|request| request.path == "/source"));
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_backend_invalid_json_response_reports_parse_error() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-backend-invalid-json");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: "not valid json".to_string(),
        headers: vec![],
    }]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_verify_backend_invalid_json_response_reports_parse_error.stderr.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one verifier request");
    assert_eq!(captured[0].path, "/source");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_dry_run_uses_overridden_mainnet_toncenter_url() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-dry-run-toncenter-override");
    let (verifier_url, verifier_handle, verifier_captured) =
        spawn_verifier_mock(vec![VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "similar"
                },
                "msgCell": {
                    "data": [1, 2, 3, 4]
                }
            })
            .to_string(),
            headers: vec![],
        }]);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v2_mock_with_capture(vec![
            toncenter_v2_verify_registry_address_response(VERIFY_TEST_REGISTRY_ADDRESS),
            toncenter_v2_verify_quorum_response("verifier.ton.org", 1),
        ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &verifier_url)
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .arg("--dry-run")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_dry_run_uses_overridden_mainnet_toncenter_url.stdout.txt",
    );

    verifier_handle.join().expect("mock verifier must finish");
    toncenter_handle.join().expect("mock toncenter must finish");

    let verifier_captured = verifier_captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(
        verifier_captured.len(),
        1,
        "expected exactly one verifier request"
    );
    assert_eq!(verifier_captured[0].path, "/source");

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        2,
        "expected exactly two Toncenter runGetMethod requests",
    );
    assert!(
        toncenter_captured
            .iter()
            .all(|request| request.path == "/jsonRPC")
    );

    let first_body = String::from_utf8_lossy(&toncenter_captured[0].body);
    let second_body = String::from_utf8_lossy(&toncenter_captured[1].body);
    assert!(
        first_body.contains("\"get_verifier_registry_address\""),
        "expected first Toncenter request to fetch verifier registry address, got: {first_body}"
    );
    assert!(
        second_body.contains("\"get_verifiers\""),
        "expected second Toncenter request to fetch verifier quorum, got: {second_body}"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_backend_proof_already_deployed_returns_success() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-backend-proof-already-deployed");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "compileResult": {
                "result": "different",
                "error": "Proof has already been deployed"
            }
        })
        .to_string(),
        headers: vec![],
    }]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_backend_proof_already_deployed_returns_success.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one verifier request");
    assert_eq!(captured[0].path, "/source");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_backend_proof_already_deployed_returns_success_on_testnet() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-backend-proof-already-deployed-testnet");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "compileResult": {
                "result": "different",
                "error": "Proof has already been deployed"
            }
        })
        .to_string(),
        headers: vec![],
    }]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("testnet")
        .wallet("deployer")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_backend_proof_already_deployed_returns_success_on_testnet.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one verifier request");
    assert_eq!(captured[0].path, "/source");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_debug_mode_prints_source_details_and_builds_multipart_upload() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-debug-source-upload");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 400,
        body: serde_json::json!({
            "error": "debug backend failure"
        })
        .to_string(),
        headers: vec![],
    }]);
    let backend_url = format!("{mock_url}/");

    let output = project
        .acton()
        .env("ACTON_VERIFY_DEBUG", "1")
        .env("ACTON_VERIFY_BACKEND", &backend_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_debug_mode_prints_source_details_and_builds_multipart_upload.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_verify_debug_mode_prints_source_details_and_builds_multipart_upload.stderr.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one verifier request");
    assert_eq!(captured[0].path, "/source");
    let body = String::from_utf8_lossy(&captured[0].body);
    assert!(
        body.contains("name=\"contracts/simple.tolk\""),
        "multipart request must include normalized source path, got: {body}"
    );
    assert!(
        body.contains("filename=\"simple.tolk\""),
        "multipart request must include uploaded source filename, got: {body}"
    );
    assert!(
        body.contains(
            "\"knownContractAddress\":\"UQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7ACfo\""
        ),
        "multipart request must include known contract address metadata, got: {body}"
    );
    assert!(
        body.contains("\"senderAddress\":\"UQBRPsl7DGApAcPPFKwKpYgpJGiWnMzQ2EpMP7gef4l6nCkD\""),
        "multipart request must include sender address metadata, got: {body}"
    );
    assert!(
        body.contains("\"folder\":\"contracts\""),
        "multipart request must include verifier folder metadata, got: {body}"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_dry_run_collects_signature_from_override_backend() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-dry-run-sign-success");
    let (source_url, source_handle, source_captured) =
        spawn_verifier_mock(vec![VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "similar"
                },
                "msgCell": {
                    "data": [1, 2, 3, 4]
                }
            })
            .to_string(),
            headers: vec![],
        }]);
    let (sign_url, sign_handle, sign_captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "msgCell": {
                "data": [9, 8, 7, 6]
            }
        })
        .to_string(),
        headers: vec![],
    }]);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v2_mock_with_capture(vec![
            toncenter_v2_verify_registry_address_response(VERIFY_TEST_REGISTRY_ADDRESS),
            toncenter_v2_verify_quorum_response("verifier.ton.org", 2),
        ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &source_url)
        .env(VERIFY_BACKENDS_ENV, &sign_url)
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .arg("--api-key")
        .arg(VERIFY_TEST_API_KEY)
        .arg("--dry-run")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_dry_run_collects_signature_from_override_backend.stdout.txt",
    );

    source_handle.join().expect("mock verifier must finish");
    sign_handle.join().expect("mock signer must finish");
    toncenter_handle.join().expect("mock toncenter must finish");

    let source_captured = source_captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(
        source_captured.len(),
        1,
        "expected one source backend request"
    );
    assert_eq!(source_captured[0].path, "/source");

    let sign_captured = sign_captured
        .lock()
        .expect("captured signer requests mutex poisoned");
    assert_eq!(sign_captured.len(), 1, "expected one sign backend request");
    assert_eq!(sign_captured[0].path, "/sign");
    let sign_body: serde_json::Value =
        serde_json::from_slice(&sign_captured[0].body).expect("sign request must be valid json");
    assert_eq!(
        sign_body,
        serde_json::json!({
            "messageCell": {
                "data": [1, 2, 3, 4]
            }
        })
    );

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        2,
        "expected registry + quorum Toncenter requests",
    );
    assert!(
        toncenter_captured
            .iter()
            .all(|request| header_value(&request.headers, "x-api-key") == Some(VERIFY_TEST_API_KEY))
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_fails_when_signer_backends_do_not_reach_quorum() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-sign-quorum-failure");
    let (source_url, source_handle, source_captured) =
        spawn_verifier_mock(vec![VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "similar"
                },
                "msgCell": {
                    "data": [1, 2, 3, 4]
                }
            })
            .to_string(),
            headers: vec![],
        }]);
    let (sign_url, sign_handle, sign_captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 500,
        body: "mock sign failure".to_string(),
        headers: vec![],
    }]);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v2_mock_with_capture(vec![
            toncenter_v2_verify_registry_address_response(VERIFY_TEST_REGISTRY_ADDRESS),
            toncenter_v2_verify_quorum_response("verifier.ton.org", 2),
        ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &source_url)
        .env(VERIFY_BACKENDS_ENV, &sign_url)
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .arg("--api-key")
        .arg(VERIFY_TEST_API_KEY)
        .arg("--dry-run")
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_fails_when_signer_backends_do_not_reach_quorum.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_verify_fails_when_signer_backends_do_not_reach_quorum.stderr.txt",
    );

    source_handle.join().expect("mock verifier must finish");
    sign_handle.join().expect("mock signer must finish");
    toncenter_handle.join().expect("mock toncenter must finish");

    let source_captured = source_captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(
        source_captured.len(),
        1,
        "expected one source backend request"
    );

    let sign_captured = sign_captured
        .lock()
        .expect("captured signer requests mutex poisoned");
    assert_eq!(sign_captured.len(), 1, "expected one sign backend request");
    assert_eq!(sign_captured[0].path, "/sign");

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        2,
        "expected registry + quorum Toncenter requests",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_send_transaction_successfully_after_mocked_prepare_flow() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-send-success");
    let msg_cell = valid_message_cell_boc();
    let (verifier_url, verifier_handle, verifier_captured) =
        spawn_verifier_mock(vec![VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "similar"
                },
                "msgCell": {
                    "data": msg_cell
                }
            })
            .to_string(),
            headers: vec![],
        }]);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v2_mock_with_capture(vec![
            toncenter_v2_verify_registry_address_response(VERIFY_TEST_REGISTRY_ADDRESS),
            toncenter_v2_verify_quorum_response("verifier.ton.org", 1),
            crate::support::toncenter::toncenter_v2_seqno_ok_response(),
            toncenter_v2_send_boc_ok_response(),
        ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &verifier_url)
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .arg("--api-key")
        .arg(VERIFY_TEST_API_KEY)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_send_transaction_successfully_after_mocked_prepare_flow.stdout.txt",
    );

    verifier_handle.join().expect("mock verifier must finish");
    toncenter_handle.join().expect("mock toncenter must finish");

    let verifier_captured = verifier_captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(verifier_captured.len(), 1, "expected one verifier request");
    assert_eq!(verifier_captured[0].path, "/source");

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        4,
        "expected registry + quorum + seqno + sendBoc requests",
    );
    assert_eq!(toncenter_captured[0].path, "/jsonRPC");
    assert_eq!(toncenter_captured[1].path, "/jsonRPC");
    assert_eq!(toncenter_captured[2].path, "/jsonRPC");
    assert_eq!(toncenter_captured[3].path, "/sendBoc");
    assert!(
        toncenter_captured
            .iter()
            .all(|request| header_value(&request.headers, "x-api-key") == Some(VERIFY_TEST_API_KEY))
    );
    let send_boc_body: serde_json::Value = serde_json::from_slice(&toncenter_captured[3].body)
        .expect("sendBoc request must be valid json");
    let boc = send_boc_body
        .get("boc")
        .and_then(serde_json::Value::as_str)
        .expect("sendBoc request must include boc string");
    assert!(
        !boc.is_empty(),
        "sendBoc request must include non-empty boc"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_send_transaction_successfully_on_testnet() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-send-success-testnet");
    let msg_cell = valid_message_cell_boc();
    let (verifier_url, verifier_handle, verifier_captured) =
        spawn_verifier_mock(vec![VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "similar"
                },
                "msgCell": {
                    "data": msg_cell
                }
            })
            .to_string(),
            headers: vec![],
        }]);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v2_mock_with_capture(vec![
            toncenter_v2_verify_registry_address_response(VERIFY_TEST_REGISTRY_ADDRESS),
            toncenter_v2_verify_quorum_response("verifier.ton.org", 1),
            crate::support::toncenter::toncenter_v2_seqno_ok_response(),
            toncenter_v2_send_boc_ok_response(),
        ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &verifier_url)
        .env(TEST_TONCENTER_TESTNET_V2_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("testnet")
        .wallet("deployer")
        .arg("--api-key")
        .arg(VERIFY_TEST_API_KEY)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_send_transaction_successfully_on_testnet.stdout.txt",
    );

    verifier_handle.join().expect("mock verifier must finish");
    toncenter_handle.join().expect("mock toncenter must finish");

    let verifier_captured = verifier_captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(verifier_captured.len(), 1, "expected one verifier request");
    assert_eq!(verifier_captured[0].path, "/source");

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        4,
        "expected registry + quorum + seqno + sendBoc requests",
    );
    assert_eq!(toncenter_captured[0].path, "/jsonRPC");
    assert_eq!(toncenter_captured[1].path, "/jsonRPC");
    assert_eq!(toncenter_captured[2].path, "/jsonRPC");
    assert_eq!(toncenter_captured[3].path, "/sendBoc");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_reports_send_boc_failure() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-send-failure");
    let msg_cell = valid_message_cell_boc();
    let (verifier_url, verifier_handle, verifier_captured) =
        spawn_verifier_mock(vec![VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "compileResult": {
                    "result": "similar"
                },
                "msgCell": {
                    "data": msg_cell
                }
            })
            .to_string(),
            headers: vec![],
        }]);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v2_mock_with_capture(vec![
            toncenter_v2_verify_registry_address_response(VERIFY_TEST_REGISTRY_ADDRESS),
            toncenter_v2_verify_quorum_response("verifier.ton.org", 1),
            crate::support::toncenter::toncenter_v2_seqno_ok_response(),
            toncenter_v2_send_boc_client_error_response("mock verification send failure"),
        ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &verifier_url)
        .env(TEST_TONCENTER_MAINNET_V2_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .verify_network("mainnet")
        .wallet("deployer")
        .arg("--api-key")
        .arg(VERIFY_TEST_API_KEY)
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/test_verify_reports_send_boc_failure.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_verify_reports_send_boc_failure.stderr.txt",
    );

    verifier_handle.join().expect("mock verifier must finish");
    toncenter_handle.join().expect("mock toncenter must finish");

    let verifier_captured = verifier_captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(verifier_captured.len(), 1, "expected one verifier request");

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        4,
        "expected registry + quorum + seqno + sendBoc requests",
    );
    assert_eq!(toncenter_captured[3].path, "/sendBoc");
}

#[allow(clippy::significant_drop_tightening)]
#[cfg(unix)]
#[test]
fn test_verify_without_contract_address_and_wallet_uses_prompts() {
    let _guard = verify_backend_mock_guard();
    let project = ProjectBuilder::new("verify-interactive-prompts")
        .contract("alpha", SIMPLE_CONTRACT)
        .contract("beta", SIMPLE_CONTRACT)
        .build();
    write_multiple_wallets(project.path());
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "compileResult": {
                "result": "different",
                "error": "Proof has already been deployed"
            }
        })
        .to_string(),
        headers: vec![],
    }]);

    let mut session = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_network("mainnet")
        .spawn_pty()
        .set_expect_timeout(Some(std::time::Duration::from_secs(30)));

    session.expect("Multiple contracts found. Please select which contract to verify:");
    session.send_line("", "failed to select default contract");
    session.expect("Enter deployed contract address:");
    session.send_line(VERIFY_TEST_ADDRESS, "failed to provide contract address");
    session.expect("Multiple wallets configured. Please select which wallet to use:");
    session.send_line("", "failed to select default wallet");
    session.expect("has already been verified previously");
    session.expect(Eof);

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected one verifier request");
    assert_eq!(captured[0].path, "/source");
}
