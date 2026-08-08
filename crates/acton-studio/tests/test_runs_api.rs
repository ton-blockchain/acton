use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use acton_studio::{
    STUDIO_TEST_RUNS_PATH, StartTestRunRequest, StudioServer, StudioServerConfig, TestRunEvent,
    TestRunEventEnvelope, TestRunOutput, TestRunRecord, TestRunRuntime, TestRunRuntimeError,
    TestRunRuntimeFuture, TestRunSource, TestRunStats, TestRunStatus, TestRunStreamEvent,
    TestRunSummary,
};
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response};
use chrono::{DateTime, Utc};
use expect_test::expect;
use tokio::sync::broadcast;
use tower::ServiceExt;

struct TestRunRuntimeFixture {
    runs: Mutex<BTreeMap<String, TestRunRecord>>,
    events: broadcast::Sender<TestRunStreamEvent>,
}

impl TestRunRuntimeFixture {
    fn new() -> Self {
        let (events, _) = broadcast::channel(16);
        Self {
            runs: Mutex::new(BTreeMap::new()),
            events,
        }
    }
}

impl TestRunRuntime for TestRunRuntimeFixture {
    fn list(&self) -> TestRunRuntimeFuture<'_, Vec<TestRunSummary>> {
        Box::pin(async {
            Ok(self
                .runs
                .lock()
                .expect("test run lock must not be poisoned")
                .values()
                .map(TestRunRecord::summary)
                .collect())
        })
    }

    fn get(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        let run_id = run_id.to_owned();
        Box::pin(async move {
            self.runs
                .lock()
                .expect("test run lock must not be poisoned")
                .get(&run_id)
                .cloned()
                .ok_or(TestRunRuntimeError::NotFound { run_id })
        })
    }

    fn start(&self, request: StartTestRunRequest) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        Box::pin(async move {
            let mut command = vec!["acton".to_owned(), "test".to_owned()];
            command.extend(request.paths);
            if let Some(filter) = request.filter {
                command.extend(["--filter".to_owned(), filter]);
            }
            for include in request.include {
                command.extend(["--include".to_owned(), include]);
            }
            for exclude in request.exclude {
                command.extend(["--exclude".to_owned(), exclude]);
            }
            if request.fail_fast {
                command.push("--fail-fast".to_owned());
            }
            if request.save_traces {
                command.push("--save-test-trace".to_owned());
            }
            let run = TestRunRecord {
                format_version: 1,
                id: "run-1".to_owned(),
                project_root: PathBuf::from("/workspace/counter"),
                source: TestRunSource::Studio,
                status: TestRunStatus::Running,
                command,
                started_at: fixed_time("2026-07-26T10:00:00Z"),
                finished_at: None,
                exit_code: None,
                stats: TestRunStats::default(),
                reports: Vec::new(),
                trace_dir: Some(PathBuf::from(
                    "/workspace/counter/.studio/tests/traces/run-1",
                )),
                error: None,
            };
            self.runs
                .lock()
                .expect("test run lock must not be poisoned")
                .insert(run.id.clone(), run.clone());
            Ok(run)
        })
    }

    fn cancel(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        let run_id = run_id.to_owned();
        Box::pin(async move {
            let mut runs = self
                .runs
                .lock()
                .expect("test run lock must not be poisoned");
            let run = runs
                .get_mut(&run_id)
                .ok_or_else(|| TestRunRuntimeError::NotFound {
                    run_id: run_id.clone(),
                })?;
            run.status = TestRunStatus::Cancelled;
            run.finished_at = Some(fixed_time("2026-07-26T10:00:02Z"));
            let result = run.clone();
            drop(runs);
            Ok(result)
        })
    }

    fn ingest(&self, envelope: TestRunEventEnvelope) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        Box::pin(async move {
            let mut runs = self
                .runs
                .lock()
                .expect("test run lock must not be poisoned");
            let run =
                runs.get_mut(&envelope.run_id)
                    .ok_or_else(|| TestRunRuntimeError::NotFound {
                        run_id: envelope.run_id.clone(),
                    })?;
            match &envelope.event {
                TestRunEvent::TestFinished { report } => run.reports.push(report.clone()),
                TestRunEvent::RunFinished { run: finished } => {
                    *run = finished.clone();
                }
                TestRunEvent::RunStarted { .. }
                | TestRunEvent::SuiteStarted { .. }
                | TestRunEvent::SuiteFinished { .. }
                | TestRunEvent::TestStarted { .. } => {}
            }
            let result = run.clone();
            drop(runs);
            self.events
                .send(TestRunStreamEvent::ReporterEvent {
                    event: Box::new(envelope),
                })
                .ok();
            Ok(result)
        })
    }

    fn output(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunOutput> {
        let exists = self
            .runs
            .lock()
            .expect("test run lock must not be poisoned")
            .contains_key(run_id);
        let run_id = run_id.to_owned();
        Box::pin(async move {
            if !exists {
                return Err(TestRunRuntimeError::NotFound { run_id });
            }
            Ok(TestRunOutput {
                stdout: "Running tests\nPASS counter increments\n".to_owned(),
                stderr: String::new(),
            })
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<TestRunStreamEvent> {
        self.events.subscribe()
    }
}

fn router() -> axum::Router {
    StudioServer::new(StudioServerConfig::new("test-version"))
        .with_test_run_runtime(TestRunRuntimeFixture::new())
        .router()
}

async fn response_snapshot(response: Response<Body>) -> String {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body))
}

#[tokio::test]
async fn gui_and_reporter_share_one_test_run_contract() {
    let app = router();
    let start = app
        .clone()
        .oneshot(
            Request::post(STUDIO_TEST_RUNS_PATH)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "paths":["tests/counter.test.tolk"],
                        "filter":"increments",
                        "include":["**/counter*"],
                        "exclude":["**/slow/**"],
                        "failFast":true,
                        "saveTraces":true
                    }"#,
                ))
                .expect("start request must be valid"),
        )
        .await
        .expect("start request must succeed");
    let test_finished = app
        .clone()
        .oneshot(
            Request::post("/api/v1/test-runs/run-1/events")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "formatVersion":1,
                        "runId":"run-1",
                        "sequence":1,
                        "event":{
                            "type":"testFinished",
                            "data":{
                                "report":{
                                    "name":"test increments",
                                    "suite_name":"counter.test.tolk",
                                    "file_path":"/workspace/counter/tests/counter.test.tolk",
                                    "row":12,
                                    "column":5,
                                    "duration":{"secs":0,"nanos":12500000},
                                    "status":"Passed",
                                    "trace_path":"test_increments.json"
                                }
                            }
                        }
                    }"#,
                ))
                .expect("test event request must be valid"),
        )
        .await
        .expect("test event request must succeed");
    let run_finished = app
        .clone()
        .oneshot(
            Request::post("/api/v1/test-runs/run-1/events")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "formatVersion":1,
                        "runId":"run-1",
                        "sequence":2,
                        "event":{
                            "type":"runFinished",
                            "data":{
                                "run":{
                                    "formatVersion":1,
                                    "id":"run-1",
                                    "projectRoot":"/workspace/counter",
                                    "source":"studio",
                                    "status":"passed",
                                    "command":["acton","test","tests/counter.test.tolk","--filter","increments","--include","**/counter*","--exclude","**/slow/**","--fail-fast","--save-test-trace"],
                                    "startedAt":"2026-07-26T10:00:00Z",
                                    "finishedAt":"2026-07-26T10:00:01Z",
                                    "exitCode":0,
                                    "stats":{
                                        "total":1,
                                        "passed":1,
                                        "failed":0,
                                        "skipped":0,
                                        "todo":0,
                                        "durationMs":14
                                    },
                                    "reports":[{
                                        "name":"test increments",
                                        "suite_name":"counter.test.tolk",
                                        "file_path":"/workspace/counter/tests/counter.test.tolk",
                                        "row":12,
                                        "column":5,
                                        "duration":{"secs":0,"nanos":12500000},
                                        "status":"Passed",
                                        "trace_path":"test_increments.json"
                                    }],
                                    "traceDir":"/workspace/counter/.studio/tests/traces/run-1"
                                }
                            }
                        }
                    }"#,
                ))
                .expect("finish event request must be valid"),
        )
        .await
        .expect("finish event request must succeed");
    let list = app
        .clone()
        .oneshot(
            Request::get(STUDIO_TEST_RUNS_PATH)
                .body(Body::empty())
                .expect("list request must be valid"),
        )
        .await
        .expect("list request must succeed");
    let reports = app
        .clone()
        .oneshot(
            Request::get("/api/v1/test-runs/run-1/artifacts/reports")
                .body(Body::empty())
                .expect("reports request must be valid"),
        )
        .await
        .expect("reports request must succeed");
    let logs = app
        .clone()
        .oneshot(
            Request::get(
                "/api/v1/test-runs/run-1/artifacts/test-logs?file_path=%2Fworkspace%2Fcounter%2Ftests%2Fcounter.test.tolk&name=test%20increments&row=12&column=5",
            )
            .body(Body::empty())
            .expect("logs request must be valid"),
        )
        .await
        .expect("logs request must succeed");
    let output = app
        .oneshot(
            Request::get("/api/v1/test-runs/run-1/output")
                .body(Body::empty())
                .expect("output request must be valid"),
        )
        .await
        .expect("output request must succeed");

    let actual = format!(
        "START\n{}\n\nTEST FINISHED\n{}\n\nRUN FINISHED\n{}\n\nLIST\n{}\n\nREPORTS\n{}\n\nLOGS\n{}\n\nOUTPUT\n{}",
        response_snapshot(start).await,
        response_snapshot(test_finished).await,
        response_snapshot(run_finished).await,
        response_snapshot(list).await,
        response_snapshot(reports).await,
        response_snapshot(logs).await,
        response_snapshot(output).await,
    );

    expect![[r#"START
status: 201 Created
body: {"formatVersion":1,"id":"run-1","projectRoot":"/workspace/counter","source":"studio","status":"running","command":["acton","test","tests/counter.test.tolk","--filter","increments","--include","**/counter*","--exclude","**/slow/**","--fail-fast","--save-test-trace"],"startedAt":"2026-07-26T10:00:00Z","stats":{"total":0,"passed":0,"failed":0,"skipped":0,"todo":0,"durationMs":0},"reports":[],"traceDir":"/workspace/counter/.studio/tests/traces/run-1"}

TEST FINISHED
status: 200 OK
body: {"formatVersion":1,"id":"run-1","projectRoot":"/workspace/counter","source":"studio","status":"running","command":["acton","test","tests/counter.test.tolk","--filter","increments","--include","**/counter*","--exclude","**/slow/**","--fail-fast","--save-test-trace"],"startedAt":"2026-07-26T10:00:00Z","stats":{"total":0,"passed":0,"failed":0,"skipped":0,"todo":0,"durationMs":0},"reports":[{"name":"test increments","suite_name":"counter.test.tolk","file_path":"/workspace/counter/tests/counter.test.tolk","row":12,"column":5,"duration":{"secs":0,"nanos":12500000},"status":"Passed","trace_path":"test_increments.json"}],"traceDir":"/workspace/counter/.studio/tests/traces/run-1"}

RUN FINISHED
status: 200 OK
body: {"formatVersion":1,"id":"run-1","projectRoot":"/workspace/counter","source":"studio","status":"passed","command":["acton","test","tests/counter.test.tolk","--filter","increments","--include","**/counter*","--exclude","**/slow/**","--fail-fast","--save-test-trace"],"startedAt":"2026-07-26T10:00:00Z","finishedAt":"2026-07-26T10:00:01Z","exitCode":0,"stats":{"total":1,"passed":1,"failed":0,"skipped":0,"todo":0,"durationMs":14},"reports":[{"name":"test increments","suite_name":"counter.test.tolk","file_path":"/workspace/counter/tests/counter.test.tolk","row":12,"column":5,"duration":{"secs":0,"nanos":12500000},"status":"Passed","trace_path":"test_increments.json"}],"traceDir":"/workspace/counter/.studio/tests/traces/run-1"}

LIST
status: 200 OK
body: [{"formatVersion":1,"id":"run-1","source":"studio","status":"passed","command":["acton","test","tests/counter.test.tolk","--filter","increments","--include","**/counter*","--exclude","**/slow/**","--fail-fast","--save-test-trace"],"startedAt":"2026-07-26T10:00:00Z","finishedAt":"2026-07-26T10:00:01Z","exitCode":0,"stats":{"total":1,"passed":1,"failed":0,"skipped":0,"todo":0,"durationMs":14}}]

REPORTS
status: 200 OK
body: [{"name":"test increments","suite_name":"counter.test.tolk","file_path":"/workspace/counter/tests/counter.test.tolk","row":12,"column":5,"duration":{"secs":0,"nanos":12500000},"status":"Passed","trace_path":"test_increments.json"}]

LOGS
status: 200 OK
body: {}

OUTPUT
status: 200 OK
body: {"stdout":"Running tests\nPASS counter increments\n","stderr":""}"#]]
    .assert_eq(&actual);
}

fn fixed_time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .expect("fixed test time must be valid")
        .with_timezone(&Utc)
}
