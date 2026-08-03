#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use acton_studio::{
    LocalProcessTestRunRuntime, STUDIO_TEST_RUN_FORMAT_VERSION, StartTestRunRequest, TestRunEvent,
    TestRunEventEnvelope, TestRunRecord, TestRunRuntime, TestRunSource, TestRunStatus,
    persist_test_run,
};
use expect_test::expect;

#[tokio::test]
async fn local_runtime_owns_the_cli_process_and_captures_its_output() {
    let workspace = tempfile::tempdir().expect("temporary Studio workspace must be created");
    let executable = workspace.path().join("fake-acton");
    std::fs::write(
        &executable,
        "#!/bin/sh\nprintf 'running the shared acton test path\\n'\nprintf 'diagnostic output\\n' >&2\n",
    )
    .expect("fake Acton executable must be written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fake Acton metadata must be available")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions)
        .expect("fake Acton executable must be executable");

    let runtime =
        LocalProcessTestRunRuntime::new(&executable, workspace.path(), "http://127.0.0.1:3016");
    let started = runtime
        .start(StartTestRunRequest {
            paths: vec!["tests/counter.test.tolk".to_owned()],
            filter: Some("increments".to_owned()),
            include: vec!["**/counter*".to_owned()],
            exclude: vec!["**/slow/**".to_owned()],
            fail_fast: true,
            save_traces: true,
        })
        .await
        .expect("test process must start");
    let finished = wait_for_finished(&runtime, &started.id).await;
    let mut late_started_run = finished.clone();
    late_started_run.status = TestRunStatus::Running;
    late_started_run.finished_at = None;
    let after_late_started = runtime
        .ingest(TestRunEventEnvelope {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            run_id: started.id.clone(),
            sequence: 1,
            event: TestRunEvent::RunStarted {
                run: late_started_run,
            },
        })
        .await
        .expect("late run-started event must resolve to the finished run");
    let invalid_id_error = runtime
        .ingest(TestRunEventEnvelope {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            run_id: "../outside".to_owned(),
            sequence: 1,
            event: TestRunEvent::RunStarted {
                run: finished.clone(),
            },
        })
        .await
        .expect_err("unsafe reporter run ID must be rejected");
    let output = runtime
        .output(&started.id)
        .await
        .expect("captured output must load");
    let actual = format!(
        "status: {:?}\nafter late start: {:?}\ninvalid ID: {}\ncommand: {}\ntrace dir: {}\nstdout: {}stderr: {}",
        finished.status,
        after_late_started.status,
        invalid_id_error,
        finished.command.join(" "),
        finished
            .trace_dir
            .as_deref()
            .and_then(|path| path.strip_prefix(workspace.path()).ok())
            .expect("trace directory must be inside the workspace")
            .display(),
        output.stdout,
        output.stderr,
    );

    expect![[r"status: Passed
after late start: Passed
invalid ID: Test run ID contains unsupported characters
command: acton test tests/counter.test.tolk --filter increments --include **/counter* --exclude **/slow/** --fail-fast --save-test-trace /WORKSPACE/.studio/tests/traces/RUN_ID
trace dir: .studio/tests/traces/RUN_ID
stdout: running the shared acton test path
stderr: diagnostic output
"]]
    .assert_eq(
        &actual
            .replace(workspace.path().to_string_lossy().as_ref(), "/WORKSPACE")
            .replace(&started.id, "RUN_ID"),
    );
}

#[tokio::test]
async fn history_refresh_never_rolls_back_live_reporter_state() {
    let workspace = tempfile::tempdir().expect("temporary Studio workspace must be created");
    let executable = workspace.path().join("slow-acton");
    std::fs::write(&executable, "#!/bin/sh\nsleep 1\n")
        .expect("fake Acton executable must be written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fake Acton metadata must be available")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions)
        .expect("fake Acton executable must be executable");

    let runtime =
        LocalProcessTestRunRuntime::new(&executable, workspace.path(), "http://127.0.0.1:3016");
    let started = runtime
        .start(StartTestRunRequest::default())
        .await
        .expect("test process must start");
    let mut live = started.clone();
    live.error = Some("live reporter state".to_owned());
    runtime
        .ingest(TestRunEventEnvelope {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            run_id: started.id.clone(),
            sequence: 1,
            event: TestRunEvent::RunStarted { run: live },
        })
        .await
        .expect("live reporter state must be accepted");

    runtime
        .list()
        .await
        .expect("test history must be refreshable while a run is active");
    let refreshed = runtime
        .get(&started.id)
        .await
        .expect("active test run must remain available");
    runtime
        .cancel(&started.id)
        .await
        .expect("test process must be cancellable");
    let cancelled = wait_for_finished(&runtime, &started.id).await;
    let actual = format!(
        "after disk refresh: {}\nfinal status: {:?}",
        refreshed.error.as_deref().unwrap_or("missing"),
        cancelled.status,
    );

    expect![[r"after disk refresh: live reporter state
final status: Cancelled"]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn reporter_events_cannot_redirect_studio_to_external_run_paths() {
    let workspace = tempfile::tempdir().expect("temporary Studio workspace must be created");
    let outside = tempfile::tempdir().expect("temporary outside directory must be created");
    let executable = workspace.path().join("slow-acton");
    std::fs::write(&executable, "#!/bin/sh\nsleep 1\n")
        .expect("fake Acton executable must be written");
    let mut permissions = std::fs::metadata(&executable)
        .expect("fake Acton metadata must be available")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions)
        .expect("fake Acton executable must be executable");

    let runtime =
        LocalProcessTestRunRuntime::new(&executable, workspace.path(), "http://127.0.0.1:3016");
    let started = runtime
        .start(StartTestRunRequest {
            save_traces: true,
            ..StartTestRunRequest::default()
        })
        .await
        .expect("test process must start");

    let mut external_project = started.clone();
    external_project.project_root = outside.path().to_path_buf();
    let project_error = runtime
        .ingest(TestRunEventEnvelope {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            run_id: started.id.clone(),
            sequence: 1,
            event: TestRunEvent::RunStarted {
                run: external_project,
            },
        })
        .await
        .expect_err("external reporter project root must be rejected");

    let mut external_trace = started.clone();
    external_trace.trace_dir = Some(outside.path().to_path_buf());
    let trace_error = runtime
        .ingest(TestRunEventEnvelope {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            run_id: started.id.clone(),
            sequence: 1,
            event: TestRunEvent::RunStarted {
                run: external_trace,
            },
        })
        .await
        .expect_err("external reporter trace directory must be rejected");

    runtime
        .cancel(&started.id)
        .await
        .expect("test process must be cancellable");
    wait_for_finished(&runtime, &started.id).await;

    expect![[
        r"The reported test project root does not match the Studio workspace
The reported test trace directory is outside the Studio test run"
    ]]
    .assert_eq(&format!("{project_error}\n{trace_error}"));
}

#[tokio::test]
async fn runtime_recovers_studio_runs_interrupted_by_a_restart() {
    let workspace = tempfile::tempdir().expect("temporary Studio workspace must be created");
    let run = TestRunRecord::new(
        "interrupted-run".to_owned(),
        workspace.path().to_path_buf(),
        TestRunSource::Studio,
        vec!["acton".to_owned(), "test".to_owned()],
        None,
    );
    persist_test_run(workspace.path(), &run).expect("running test history must be persisted");

    let runtime =
        LocalProcessTestRunRuntime::new("/usr/bin/true", workspace.path(), "http://127.0.0.1:3016");
    let recovered = runtime
        .get(&run.id)
        .await
        .expect("interrupted test run must remain in history");
    let actual = format!(
        "status: {:?}\nexit code: {:?}\nfinished: {}\nerror: {}",
        recovered.status,
        recovered.exit_code,
        recovered.finished_at.is_some(),
        recovered.error.as_deref().unwrap_or("missing"),
    );

    expect![[r"status: Failed
exit code: Some(1)
finished: true
error: Studio stopped before the test run finished"]]
    .assert_eq(&actual);
}

async fn wait_for_finished(runtime: &LocalProcessTestRunRuntime, run_id: &str) -> TestRunRecord {
    for _ in 0..100 {
        let run = runtime
            .get(run_id)
            .await
            .expect("started test run must remain available");
        if run.status.is_finished() {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    panic!("test process did not finish in time")
}
