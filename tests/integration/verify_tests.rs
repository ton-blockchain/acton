use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use crate::support::verifier::{VerifierMockResponse, spawn_verifier_mock};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;

const VERIFY_TEST_ADDRESS: &str = "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot";

static VERIFY_BACKEND_MOCK_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn write_deployer_wallets(project_path: &Path) {
    std::fs::write(project_path.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
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
fn test_verify_boc_file() {
    let project = ProjectBuilder::new("verify-boc-file")
        .raw_file("contracts/contract.boc", "some boc content")
        .build();

    let toml_content = r#"[package]
name = "verify-boc-file"
description = ""
version = "0.1.0"

[contracts.contract]
name = "contract"
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
name = "Simple"
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
