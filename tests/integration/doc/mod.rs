use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::verifier::{abi_response, spawn_verifier_mock};
use serde_json::Value as JsonValue;
use std::fs;
use tycho_types::boc::Boc;

const DOC_ABI_LOCAL_CONTRACT: &str = r"
struct Storage {
    value: uint32
}

struct (0x12345678) Increase {
    amount: uint32
}

contract MyContract {
    storage: Storage
    incomingMessages: Increase
}

fun onInternalMessage(_: InMessage) {}
";

#[test]
fn test_doc_tvm_add_text() {
    let project = ProjectBuilder::new("doc-tvm-add-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_add.stdout.txt");
}

#[test]
fn test_doc_tvm_add_json() {
    let project = ProjectBuilder::new("doc-tvm-add-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("add")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_add_json.stdout.json.txt");
}

#[test]
fn test_doc_tvm_multi_text() {
    let project = ProjectBuilder::new("doc-tvm-multi-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .arg("SUB")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_multi.stdout.txt");
}

#[test]
fn test_doc_tvm_multi_json() {
    let project = ProjectBuilder::new("doc-tvm-multi-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .arg("SUB")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_multi.stdout.json.txt");
}

#[test]
fn test_doc_tvm_unknown_instruction() {
    let project = ProjectBuilder::new("doc-tvm-unknown").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADDD")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_unknown.stderr.txt",
        );
}

#[test]
fn test_doc_tvm_find_text() {
    let project = ProjectBuilder::new("doc-tvm-find-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("--find")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_find.stdout.txt");
}

#[test]
fn test_doc_tvm_find_json() {
    let project = ProjectBuilder::new("doc-tvm-find-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("--find")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_json.stdout.json.txt",
        );
}

#[test]
fn test_doc_tvm_find_multi_text() {
    let project = ProjectBuilder::new("doc-tvm-find-multi-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("outcomng")
        .arg("--find")
        .arg("--description")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_find_multi.stdout.txt");
}

#[test]
fn test_doc_tvm_find_multi_json() {
    let project = ProjectBuilder::new("doc-tvm-find-multi-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("outcomng")
        .arg("--find")
        .arg("--description")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_multi.stdout.json.txt",
        );
}

#[test]
fn test_doc_tvm_find_description_text() {
    let project = ProjectBuilder::new("doc-tvm-find-description-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("outcomng")
        .arg("--find")
        .arg("--description")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_description.stdout.txt",
        );
}

#[test]
fn test_doc_tvm_find_without_description_flag() {
    let project = ProjectBuilder::new("doc-tvm-find-description-missing-flag").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("outcomng")
        .arg("--find")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_without_description.stderr.txt",
        );
}

#[test]
fn test_doc_tvm_description_requires_find_flag() {
    let project = ProjectBuilder::new("doc-tvm-description-requires-find").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .arg("--description")
        .run()
        .failure()
        .assert_stderr_contains("--find");
}

#[test]
fn test_doc_tvm_empty_sub_category_does_not_print_separator() {
    let project = ProjectBuilder::new("doc-tvm-empty-sub-category").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("DICTIADDGETREF")
        .run()
        .success()
        .assert_contains("Category:")
        .assert_contains("dictionary")
        .assert_not_contains("Category:      dictionary /");
}

#[test]
fn test_doc_without_subcommand() {
    let project = ProjectBuilder::new("doc-without-subcommand").build();
    let log_dir = project.path().join(".acton/logs");
    fs::create_dir_all(&log_dir).expect("failed to create ACTON_LOG_DIR");

    project
        .acton()
        .env(
            "ACTON_LOG_DIR",
            log_dir.to_str().expect("log dir path is not valid UTF-8"),
        )
        .arg("doc")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/doc/test_doc_without_subcommand.stderr.txt",
        );
}

#[test]
fn test_doc_tvm_empty_query_is_rejected() {
    let project = ProjectBuilder::new("doc-tvm-empty-query").build();
    let log_dir = project.path().join(".acton/logs");
    fs::create_dir_all(&log_dir).expect("failed to create ACTON_LOG_DIR");

    project
        .acton()
        .env(
            "ACTON_LOG_DIR",
            log_dir.to_str().expect("log dir path is not valid UTF-8"),
        )
        .arg("doc")
        .arg("tvm")
        .arg("")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_empty_query.stderr.txt",
        );
}

#[test]
fn test_doc_abi_catalog_contract() {
    let project = ProjectBuilder::new("doc-abi-catalog").build();

    project
        .acton()
        .current_dir(project.path())
        .arg("doc")
        .arg("abi")
        .arg("WalletV1r1")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_abi_catalog_contract.stdout.json.txt",
        );
}

#[test]
fn test_doc_abi_local_contract_by_abi_name() {
    let project = ProjectBuilder::new("doc-abi-local")
        .contract("my_contract", DOC_ABI_LOCAL_CONTRACT)
        .build();

    project
        .acton()
        .current_dir(project.path())
        .arg("doc")
        .arg("abi")
        .arg("MyContract")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_abi_local_contract_by_abi_name.stdout.json.txt",
        );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_doc_abi_verifier_contract_by_code_hash_and_cache() {
    let source_project = ProjectBuilder::new("doc-abi-verifier-source")
        .contract("my_contract", DOC_ABI_LOCAL_CONTRACT)
        .build();
    source_project
        .acton()
        .build()
        .contract("my_contract")
        .run()
        .success();

    let artifact = fs::read_to_string(source_project.path().join("build/my_contract.json"))
        .expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");
    let code = Boc::decode_base64(code_boc64).expect("code BoC must decode");
    let code_hash = code.repr_hash().to_string();
    let abi = fs::read_to_string(source_project.path().join("build/abi/my_contract.json"))
        .expect("ABI artifact must exist");
    let abi: JsonValue = serde_json::from_str(&abi).expect("ABI artifact must be valid json");

    let project = ProjectBuilder::new("doc-abi-verifier").build();
    let log_dir = project.path().join(".acton/logs");
    fs::create_dir_all(&log_dir).expect("failed to create ACTON_LOG_DIR");
    let log_dir = log_dir.to_string_lossy().into_owned();
    let code_hash_query = format!("0x{code_hash}");
    let (verifier_url, verifier_handle, verifier_captured) =
        spawn_verifier_mock(vec![abi_response(&code_hash, &abi)]);

    for _ in 0..2 {
        project
            .acton()
            .current_dir(project.path())
            .arg("doc")
            .arg("abi")
            .arg(&code_hash_query)
            .env("ACTON_NEW_VERIFY_BACKEND", &verifier_url)
            .env("ACTON_LOG_DIR", &log_dir)
            .run()
            .success()
            .assert_snapshot_matches(
                "integration/snapshots/doc/test_doc_abi_local_contract_by_abi_name.stdout.json.txt",
            );
    }

    verifier_handle
        .join()
        .expect("verifier mock server thread must finish");
    let verifier_captured = verifier_captured
        .lock()
        .expect("captured verifier requests mutex should not be poisoned");
    assert_eq!(verifier_captured.len(), 1, "verifier ABI should be cached");
    assert_eq!(verifier_captured[0].method, "GET");
    assert_eq!(
        verifier_captured[0].path,
        format!("/api/v1/abi?code_hash={code_hash}"),
    );
}

#[test]
fn test_doc_tvm_whitespace_query_is_normalized() {
    let project = ProjectBuilder::new("doc-tvm-whitespace-query").build();
    let log_dir = project.path().join(".acton/logs");
    fs::create_dir_all(&log_dir).expect("failed to create ACTON_LOG_DIR");

    project
        .acton()
        .env(
            "ACTON_LOG_DIR",
            log_dir.to_str().expect("log dir path is not valid UTF-8"),
        )
        .arg("doc")
        .arg("tvm")
        .arg("   ADD   ")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_add.stdout.txt");
}
