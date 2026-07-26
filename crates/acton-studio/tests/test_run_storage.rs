use std::path::PathBuf;

use acton_studio::{
    StudioDaemonDescriptor, TestRunRecord, TestRunSource, TestRunStats, TestRunStatus,
    load_studio_daemon_descriptor, load_test_runs, persist_studio_daemon_descriptor,
    persist_test_run, remove_studio_daemon_descriptor,
};
use chrono::{DateTime, Utc};
use expect_test::expect;

#[test]
fn test_history_and_daemon_descriptor_use_the_studio_directory() {
    let workspace = tempfile::tempdir().expect("temporary Studio workspace must be created");
    let root = workspace.path();
    let run = TestRunRecord {
        format_version: 1,
        id: "manual-run".to_owned(),
        project_root: root.to_path_buf(),
        source: TestRunSource::Manual,
        status: TestRunStatus::Failed,
        command: vec!["acton".to_owned(), "test".to_owned()],
        started_at: fixed_time("2026-07-26T10:00:00Z"),
        finished_at: Some(fixed_time("2026-07-26T10:00:01Z")),
        exit_code: Some(1),
        stats: TestRunStats {
            total: 2,
            passed: 1,
            failed: 1,
            skipped: 0,
            todo: 0,
            duration_ms: 998,
        },
        reports: Vec::new(),
        trace_dir: Some(PathBuf::from(".studio/tests/traces/manual-run")),
        error: None,
    };
    persist_test_run(root, &run).expect("test run history must be persisted");
    let invalid_run = TestRunRecord {
        id: "../outside".to_owned(),
        ..run
    };
    let invalid_run_error =
        persist_test_run(root, &invalid_run).expect_err("unsafe test run ID must be rejected");
    persist_studio_daemon_descriptor(
        root,
        &StudioDaemonDescriptor {
            protocol_version: 1,
            url: "http://127.0.0.1:3016".to_owned(),
            pid: 42,
        },
    )
    .expect("daemon descriptor must be persisted");

    let stored_run = load_test_runs(root)
        .expect("test run history must load")
        .remove("manual-run")
        .expect("stored test run must exist");
    let descriptor = load_studio_daemon_descriptor(root)
        .expect("daemon descriptor must load")
        .expect("daemon descriptor must exist");
    remove_studio_daemon_descriptor(root, 41)
        .expect("another process must not remove the descriptor");
    let after_wrong_pid = load_studio_daemon_descriptor(root)
        .expect("daemon descriptor must still load")
        .is_some();
    remove_studio_daemon_descriptor(root, 42).expect("owner must remove the descriptor");
    let after_owner = load_studio_daemon_descriptor(root)
        .expect("removed daemon descriptor lookup must succeed")
        .is_some();
    let actual = format!(
        "history path: {}\nsource: {:?}\nstatus: {:?}\nstats: {}/{}/{}\ninvalid run ID: {:?}\ndescriptor: {} pid={}\nafter wrong pid: {after_wrong_pid}\nafter owner: {after_owner}",
        root.join(".studio/tests/runs/manual-run.json")
            .strip_prefix(root)
            .expect("history path must be inside root")
            .display(),
        stored_run.source,
        stored_run.status,
        stored_run.stats.total,
        stored_run.stats.passed,
        stored_run.stats.failed,
        invalid_run_error.kind(),
        descriptor.url,
        descriptor.pid,
    );

    expect![[r"history path: .studio/tests/runs/manual-run.json
source: Manual
status: Failed
stats: 2/1/1
invalid run ID: InvalidInput
descriptor: http://127.0.0.1:3016 pid=42
after wrong pid: true
after owner: false"]]
    .assert_eq(&actual);
}

fn fixed_time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixed test time must be valid")
        .with_timezone(&Utc)
}
