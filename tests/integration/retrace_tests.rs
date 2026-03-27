use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

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
