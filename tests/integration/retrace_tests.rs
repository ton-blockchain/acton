use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
";

const BROKEN_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {
    val broken = ;
}
";

#[test]
fn test_retrace_requires_transaction_hash() {
    ProjectBuilder::new("retrace-requires-hash")
        .build()
        .acton()
        .arg("retrace")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_requires_transaction_hash.stderr.txt",
        );
}

#[test]
fn test_retrace_rejects_invalid_network_name() {
    ProjectBuilder::new("retrace-invalid-network-name")
        .build()
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--net")
        .arg("invalid-network")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_rejects_invalid_network_name.stderr.txt",
        );
}

#[test]
fn test_retrace_localnet_is_rejected_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-localnet-unsupported").build();
    let logs_dir = project.path().join("retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--net")
        .arg("localnet")
        .arg("--logs-dir")
        .arg("retrace-logs")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_localnet_is_rejected_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}

#[test]
fn test_retrace_debug_requires_contract_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-debug-requires-contract").build();
    let logs_dir = project.path().join("retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--debug")
        .arg("--logs-dir")
        .arg("retrace-logs")
        .current_dir(project.path())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_debug_requires_contract_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}

#[test]
fn test_retrace_debug_port_without_debug_is_ignored_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-debug-port-without-debug").build();
    let logs_dir = project.path().join("retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--debug-port")
        .arg("5005")
        .arg("--net")
        .arg("localnet")
        .arg("--logs-dir")
        .arg("retrace-logs")
        .current_dir(project.path())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_debug_port_without_debug_is_ignored_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}

#[test]
fn test_retrace_contract_not_found_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-contract-not-found")
        .contract("counter", SIMPLE_CONTRACT)
        .build();
    let logs_dir = project.path().join("retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--contract")
        .arg("missing")
        .arg("--logs-dir")
        .arg("retrace-logs")
        .current_dir(project.path())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_contract_not_found_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}

#[test]
fn test_retrace_non_tolk_contract_is_rejected_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-non-tolk-contract")
        .without_acton_toml()
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "retrace-non-tolk-contract"
description = "A test project"
version = "0.1.0"
license = "MIT"

[contracts.legacy]
display-name = "legacy"
src = "contracts/legacy.fc"
depends = []
"#,
        )
        .raw_file("contracts/legacy.fc", "() recv_internal(int msg_value) {}")
        .build();
    let logs_dir = project.path().join("retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--contract")
        .arg("legacy")
        .arg("--logs-dir")
        .arg("retrace-logs")
        .current_dir(project.path())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_non_tolk_contract_is_rejected_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}

#[test]
fn test_retrace_contract_compile_error_is_reported_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-contract-compile-error")
        .contract("broken", BROKEN_CONTRACT)
        .build();
    let logs_dir = project.path().join("retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--contract")
        .arg("broken")
        .arg("--logs-dir")
        .arg("retrace-logs")
        .current_dir(project.path())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_contract_compile_error_is_reported_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}

#[test]
fn test_retrace_custom_network_is_rejected_before_logs_are_created() {
    let project = ProjectBuilder::new("retrace-custom-network-unsupported").build();
    let logs_dir = project.path().join("custom-retrace-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("retrace")
        .arg("deadbeef")
        .arg("--net")
        .arg("custom:ci")
        .arg("--logs-dir")
        .arg("custom-retrace-logs")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/retrace/test_retrace_custom_network_is_rejected_before_logs_are_created.stderr.txt",
        );

    assert!(
        !logs_dir.exists(),
        "logs directory should not be created on early retrace failure: {}",
        logs_dir.display()
    );
}
