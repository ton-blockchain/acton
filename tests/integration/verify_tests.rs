use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use crate::support::toncenter::{spawn_toncenter_v3_mock, toncenter_v3_account_states_ok_response};
use crate::support::verifier::{VerifierMockResponse, spawn_verifier_mock};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

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
const TEST_TONCENTER_TESTNET_V3_URL_ENV: &str = "ACTON_TEST_TONCENTER_TESTNET_V3_URL";
const VERIFY_TEST_CODE_HASH: &str =
    "e67eec3bd481c7910c87a061e60ca509e82edd687a0e1c8bf1b437e6de3e6973";
const VERIFY_TEST_SOURCE_BUNDLE_HASH: &str =
    "a7f1d1a6aabbccddeeff00112233445566778899aabbccddeeff001122334455";
const VERIFY_TEST_PAYMENT_ADDRESS: &str =
    "0:1111111111111111111111111111111111111111111111111111111111111111";
const VERIFY_TEST_PAYMENT_TX_HASH: &str =
    "a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b";
const VERIFY_TEST_PAYMENT_TX_HASH_BASE64: &str = "oH2VGnArkQ1fZbcQyozpZnvQ89gDz4SOAfdXRKCNOUs=";

static VERIFY_BACKEND_MOCK_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn write_deployer_wallets(project_path: &Path) {
    std::fs::write(project_path.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
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

fn payment_ticket_response() -> VerifierMockResponse {
    VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "status": "payment_required",
            "code_hash": VERIFY_TEST_CODE_HASH,
            "payment_address": VERIFY_TEST_PAYMENT_ADDRESS,
            "amount_nano": "10000000",
            "comment": format!("acton-verify:v1:{VERIFY_TEST_CODE_HASH}")
        })
        .to_string(),
        headers: vec![],
    }
}

fn verifier_error_response(status: u16, error: &str) -> VerifierMockResponse {
    VerifierMockResponse {
        status,
        body: serde_json::json!({"error": error}).to_string(),
        headers: vec![],
    }
}

fn successful_verification_response() -> VerifierMockResponse {
    VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "code_hash": VERIFY_TEST_CODE_HASH,
            "compiled_code_hash": VERIFY_TEST_CODE_HASH,
            "verification_result": "match",
            "source_bundle_hash": VERIFY_TEST_SOURCE_BUNDLE_HASH,
            "storage_revision": "0123456789abcdef"
        })
        .to_string(),
        headers: vec![],
    }
}

fn assert_verifier_payment_error(project_name: &str, status: u16, error: &str, snapshot: &str) {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project(project_name);
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        verifier_error_response(status, error),
    ]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(snapshot);

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected ticket and verify requests");
    drop(captured);
}

fn compile_simple_contract_boc_base64(project: &Project) -> String {
    project
        .acton()
        .compile("contracts/simple.tolk")
        .base64_only()
        .run()
        .success()
        .get_stdout()
        .trim()
        .to_owned()
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
            "integration/snapshots/verify/test_verify_contract_not_found.stderr.txt",
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
            "integration/snapshots/verify/test_verify_contract_display_name_shows_contract_id_hint.stderr.txt",
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
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_boc_file.stderr.txt",
        );
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
            "integration/snapshots/verify/test_verify_non_tolk_file.stderr.txt",
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
            "integration/snapshots/verify/test_verify_invalid_address.stderr.txt",
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
            "integration/snapshots/verify/test_verify_compilation_error.stderr.txt",
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
            "integration/snapshots/verify/test_verify_no_contracts_configured.stderr.txt",
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
            "integration/snapshots/verify/test_verify_empty_contracts_section.stderr.txt",
        );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_sends_api_payload_and_reports_success() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-success");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        successful_verification_response(),
    ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH_BASE64)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_sends_api_payload_and_reports_success.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected ticket and verify requests");
    assert_eq!(captured[0].method, "POST");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
    assert_eq!(captured[1].method, "POST");
    assert_eq!(captured[1].path, "/api/v1/verify");
    let ticket_body = String::from_utf8_lossy(&captured[0].body);
    assert!(
        ticket_body.contains(VERIFY_TEST_CODE_HASH),
        "ticket request must include target code hash, got: {ticket_body}"
    );
    let body = String::from_utf8_lossy(&captured[1].body);
    assert!(
        body.contains("name=\"code_hash\"") && body.contains(VERIFY_TEST_CODE_HASH),
        "multipart request must include target code hash, got: {body}"
    );
    assert!(
        !body.contains("name=\"address\""),
        "multipart request must not include address when --address is omitted, got: {body}"
    );
    assert!(
        body.contains("name=\"language\"") && body.contains("tolk"),
        "multipart request must include Tolk language, got: {body}"
    );
    assert!(
        body.contains("name=\"compile_params\"") && body.contains("\"compiler_version\":\"1.4.2\""),
        "multipart request must include compiler params, got: {body}"
    );
    assert!(
        body.contains("name=\"sources\"")
            && body.contains("\"path\":\"contracts/simple.tolk\"")
            && body.contains("\"is_entrypoint\":true"),
        "multipart request must include source metadata, got: {body}"
    );
    assert!(
        body.contains("name=\"files\"") && body.contains("filename=\"contracts/simple.tolk\""),
        "multipart request must upload source files under matching paths, got: {body}"
    );
    assert!(
        body.contains("name=\"tx_hash\"") && body.contains(VERIFY_TEST_PAYMENT_TX_HASH),
        "multipart request must include the payment transaction hash, got: {body}"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_stops_when_code_is_already_verified() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-already-verified");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "status": "already_verified",
            "code_hash": VERIFY_TEST_CODE_HASH,
            "source_bundle_hash": VERIFY_TEST_SOURCE_BUNDLE_HASH,
            "storage_revision": "0123456789abcdef"
        })
        .to_string(),
        headers: vec![],
    }]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .env(TEST_TONCENTER_TESTNET_V3_URL_ENV, "http://127.0.0.1:1")
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .arg("--payment-tx-hash")
        .arg("not-a-transaction-hash")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_stops_when_code_is_already_verified.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_dry_run_formats_payment_without_sending_it() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-dry-run");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![payment_ticket_response()]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--dry-run")
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_dry_run_formats_payment_without_sending_it.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_rejects_a_ticket_for_another_code_hash() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-ticket-code-hash-mismatch");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "status": "payment_required",
            "code_hash": "1111111111111111111111111111111111111111111111111111111111111111",
            "payment_address": VERIFY_TEST_PAYMENT_ADDRESS,
            "amount_nano": "10000000",
            "comment": "acton-verify:v1:1111111111111111111111111111111111111111111111111111111111111111"
        })
        .to_string(),
        headers: vec![],
    }]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_rejects_a_ticket_for_another_code_hash.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_rejects_a_ticket_with_a_wrong_payment_comment() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-ticket-comment-mismatch");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "status": "payment_required",
            "code_hash": VERIFY_TEST_CODE_HASH,
            "payment_address": VERIFY_TEST_PAYMENT_ADDRESS,
            "amount_nano": "10000000",
            "comment": "acton-verify:v1:1111111111111111111111111111111111111111111111111111111111111111"
        })
        .to_string(),
        headers: vec![],
    }]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_rejects_a_ticket_with_a_wrong_payment_comment.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_rejects_a_non_basechain_payment_address() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-ticket-payment-address");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 200,
        body: serde_json::json!({
            "status": "payment_required",
            "code_hash": VERIFY_TEST_CODE_HASH,
            "payment_address": "-1:1111111111111111111111111111111111111111111111111111111111111111",
            "amount_nano": "10000000",
            "comment": format!("acton-verify:v1:{VERIFY_TEST_CODE_HASH}")
        })
        .to_string(),
        headers: vec![],
    }]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_rejects_a_non_basechain_payment_address.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_address_option_validates_deployed_code_hash() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-address-validation");
    let contract_code_boc = compile_simple_contract_boc_base64(&project);
    let (toncenter_url, toncenter_handle, toncenter_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_account_states_ok_response(
            VERIFY_TEST_ADDRESS,
            Some(&contract_code_boc),
            "active",
        )]);
    let (mock_url, mock_handle, _captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "code_hash": VERIFY_TEST_CODE_HASH,
                "compiled_code_hash": VERIFY_TEST_CODE_HASH,
                "verification_result": "match",
                "source_bundle_hash": null
            })
            .to_string(),
            headers: vec![],
        },
    ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .env(TEST_TONCENTER_TESTNET_V3_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_address_option_validates_deployed_code_hash.stdout.txt",
    );

    toncenter_handle.join().expect("mock toncenter must finish");
    mock_handle.join().expect("mock verifier must finish");

    let toncenter_captured = toncenter_captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(
        toncenter_captured.len(),
        1,
        "expected exactly one account state request"
    );
    assert!(
        toncenter_captured[0]
            .path
            .starts_with("/accountStates?address="),
        "expected accountStates request, got: {}",
        toncenter_captured[0].path
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_address_option_rejects_mismatched_deployed_code_hash() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-address-mismatch");
    let wrong_code_boc = Boc::encode_base64(Cell::default());
    let (toncenter_url, toncenter_handle, _toncenter_captured) =
        spawn_toncenter_v3_mock(vec![toncenter_v3_account_states_ok_response(
            VERIFY_TEST_ADDRESS,
            Some(&wrong_code_boc),
            "active",
        )]);
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![payment_ticket_response()]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .env(TEST_TONCENTER_TESTNET_V3_URL_ENV, &toncenter_url)
        .verify()
        .verify_contract("simple")
        .verify_address(VERIFY_TEST_ADDRESS)
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_address_option_rejects_mismatched_deployed_code_hash.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_address_option_rejects_mismatched_deployed_code_hash.stderr.txt",
    );

    toncenter_handle.join().expect("mock toncenter must finish");
    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_reports_mismatch() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-mismatch");
    let (mock_url, mock_handle, _captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        VerifierMockResponse {
            status: 200,
            body: serde_json::json!({
                "code_hash": VERIFY_TEST_CODE_HASH,
                "compiled_code_hash": "2222222222222222222222222222222222222222222222222222222222222222",
                "verification_result": "mismatch",
                "source_bundle_hash": null,
                "storage_revision": null
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
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .failure();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_reports_mismatch.stdout.txt",
    );
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_reports_mismatch.stderr.txt",
    );

    mock_handle.join().expect("mock verifier must finish");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_reports_http_error_body() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-http-error");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        VerifierMockResponse {
            status: 400,
            body: serde_json::json!({
                "error": "verifier rejected sources"
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
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_reports_http_error_body.stderr.txt",
    );

    mock_handle.join().expect("mock verifier must finish");

    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected ticket and verify requests");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
    assert_eq!(captured[1].path, "/api/v1/verify");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_rejects_an_invalid_payment_transaction_hash() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-invalid-payment-hash");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![payment_ticket_response()]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg("123")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_rejects_an_invalid_payment_transaction_hash.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
    assert_eq!(captured[0].path, "/api/v1/take_ticket");
}

#[test]
fn test_verify_verifier_reports_payment_not_found() {
    assert_verifier_payment_error(
        "verify-verifier-payment-not-found",
        402,
        "payment_not_found: transaction was not found on TON testnet",
        "integration/snapshots/verify/test_verify_verifier_reports_payment_not_found.stderr.txt",
    );
}

#[test]
fn test_verify_verifier_reports_backend_payment_hash_validation() {
    assert_verifier_payment_error(
        "verify-verifier-backend-payment-hash-validation",
        400,
        "payment_tx_hash_invalid: transaction hash must be 64 hexadecimal characters or a 32-byte base64 value",
        "integration/snapshots/verify/test_verify_verifier_reports_backend_payment_hash_validation.stderr.txt",
    );
}

#[test]
fn test_verify_verifier_reports_invalid_payment() {
    assert_verifier_payment_error(
        "verify-verifier-invalid-payment",
        402,
        "payment_invalid: transaction is not a finalized incoming payment",
        "integration/snapshots/verify/test_verify_verifier_reports_invalid_payment.stderr.txt",
    );
}

#[test]
fn test_verify_verifier_reports_insufficient_payment() {
    assert_verifier_payment_error(
        "verify-verifier-insufficient-payment",
        402,
        "payment_insufficient: expected at least 1000000 nanoGRAM, received 999999",
        "integration/snapshots/verify/test_verify_verifier_reports_insufficient_payment.stderr.txt",
    );
}

#[test]
fn test_verify_verifier_reports_payment_code_hash_mismatch() {
    assert_verifier_payment_error(
        "verify-verifier-payment-code-hash-mismatch",
        402,
        "payment_code_hash_mismatch: transaction comment does not match the requested code hash",
        "integration/snapshots/verify/test_verify_verifier_reports_payment_code_hash_mismatch.stderr.txt",
    );
}

#[test]
fn test_verify_verifier_reports_used_payment() {
    assert_verifier_payment_error(
        "verify-verifier-used-payment",
        409,
        "payment_used: transaction has already been used",
        "integration/snapshots/verify/test_verify_verifier_reports_used_payment.stderr.txt",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_retries_payment_in_progress_then_succeeds() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-payment-in-progress-retry");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        verifier_error_response(
            409,
            "payment_in_progress: transaction is already being processed",
        ),
        successful_verification_response(),
    ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_retries_payment_in_progress_then_succeeds.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 3, "expected ticket and two verify requests");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_retries_a_retryable_storage_error_then_succeeds() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-storage-retry");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        verifier_error_response(
            502,
            "verification_retryable: source storage is temporarily unavailable",
        ),
        successful_verification_response(),
    ]);

    let output = project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/verify/test_verify_verifier_retries_a_retryable_storage_error_then_succeeds.stdout.txt",
    );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 3, "expected ticket and two verify requests");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_reports_payment_in_progress_after_bounded_retries() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-payment-in-progress");
    let in_progress = verifier_error_response(
        409,
        "payment_in_progress: transaction is already being processed",
    );
    let mut responses = vec![payment_ticket_response()];
    responses.extend(vec![in_progress; 8]);
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(responses);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_reports_payment_in_progress_after_bounded_retries.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(
        captured.len(),
        9,
        "expected ticket and eight verify requests"
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_does_not_retry_a_generic_server_error() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-generic-server-error");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![
        payment_ticket_response(),
        verifier_error_response(502, "internal verifier error"),
    ]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_does_not_retry_a_generic_server_error.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 2, "expected ticket and one verify request");
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_verify_verifier_reports_payment_recovery() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-verifier-payment-recovery");
    let (mock_url, mock_handle, captured) = spawn_verifier_mock(vec![VerifierMockResponse {
        status: 503,
        body: serde_json::json!({
            "error": "payment_recovery_in_progress: payment history is still being recovered"
        })
        .to_string(),
        headers: vec![],
    }]);

    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .arg("--payment-tx-hash")
        .arg(VERIFY_TEST_PAYMENT_TX_HASH)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_reports_payment_recovery.stderr.txt",
        );

    mock_handle.join().expect("mock verifier must finish");
    let captured = captured
        .lock()
        .expect("captured verifier requests mutex poisoned");
    assert_eq!(captured.len(), 1, "expected only the ticket request");
}

#[test]
fn test_verify_verifier_rejects_network_option() {
    let project = build_verify_backend_project("verify-verifier-network-option");

    project
        .acton()
        .verify()
        .verify_contract("simple")
        .verify_network("mainnet")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/test_verify_verifier_rejects_network_option.stderr.txt",
        );
}

#[test]
fn test_verify_rejects_inconsistent_success_responses() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-response-integrity");
    for (field, value, snapshot) in [
        (
            "code_hash",
            serde_json::json!("b".repeat(64)),
            "wrong-target",
        ),
        (
            "compiled_code_hash",
            serde_json::json!("b".repeat(64)),
            "wrong-compiled",
        ),
        (
            "compiled_code_hash",
            serde_json::Value::Null,
            "wrong-compiled",
        ),
    ] {
        let mut response = successful_verification_response();
        let mut body: serde_json::Value = serde_json::from_str(&response.body).unwrap();
        body[field] = value;
        response.body = body.to_string();
        let (mock_url, mock_handle, _) =
            spawn_verifier_mock(vec![payment_ticket_response(), response]);
        project
            .acton()
            .env("ACTON_VERIFY_BACKEND", &mock_url)
            .verify()
            .verify_contract("simple")
            .arg("--payment-tx-hash")
            .arg(VERIFY_TEST_PAYMENT_TX_HASH)
            .run()
            .failure()
            .assert_stderr_snapshot_matches(&format!(
                "integration/snapshots/verify/verify-{snapshot}.stderr.txt"
            ));
        mock_handle.join().expect("mock verifier must finish");
    }
}

#[test]
fn test_verify_rejects_unsupported_source_paths_before_payment() {
    let _guard = verify_backend_mock_guard();
    let project = build_verify_backend_project("verify-path-preflight");
    std::fs::rename(
        project.path().join("contracts/simple.tolk"),
        project.path().join("contracts/my contract.tolk"),
    )
    .unwrap();
    let config_path = project.path().join("Acton.toml");
    let config = std::fs::read_to_string(&config_path)
        .unwrap()
        .replace("contracts/simple.tolk", "contracts/my contract.tolk");
    std::fs::write(config_path, config).unwrap();
    let (mock_url, mock_handle, _) = spawn_verifier_mock(vec![payment_ticket_response()]);
    project
        .acton()
        .env("ACTON_VERIFY_BACKEND", &mock_url)
        .verify()
        .verify_contract("simple")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/verify/verify-path-preflight.stderr.txt",
        );
    mock_handle
        .join()
        .expect("only a quote should be requested; no payment or upload");
}
