use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use chrono::Utc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::{Mutex, RwLock, broadcast, watch};

use crate::{
    STUDIO_TEST_RUN_FORMAT_VERSION, StartTestRunRequest, TestOutputStream, TestRunEvent,
    TestRunEventEnvelope, TestRunOutput, TestRunRecord, TestRunRuntime, TestRunRuntimeError,
    TestRunRuntimeFuture, TestRunSource, TestRunStatus, TestRunStreamEvent, TestRunSummary,
    is_valid_test_run_id, load_test_runs, new_test_run_id, persist_test_run, test_output_paths,
    test_trace_dir,
};

const TEST_EVENT_BUFFER: usize = 256;

#[derive(Clone)]
pub struct LocalProcessTestRunRuntime {
    inner: Arc<Inner>,
}

struct Inner {
    acton_executable: PathBuf,
    project_root: PathBuf,
    studio_url: String,
    runs: RwLock<BTreeMap<String, TestRunRecord>>,
    sequences: Mutex<HashMap<String, u64>>,
    persistence: Mutex<()>,
    cancellations: Mutex<HashMap<String, watch::Sender<bool>>>,
    events: broadcast::Sender<TestRunStreamEvent>,
}

impl LocalProcessTestRunRuntime {
    #[must_use]
    pub fn new(
        acton_executable: impl Into<PathBuf>,
        project_root: impl Into<PathBuf>,
        studio_url: impl Into<String>,
    ) -> Self {
        let project_root = project_root.into();
        let mut runs = load_test_runs(&project_root).unwrap_or_default();
        for run in runs
            .values_mut()
            .filter(|run| run.source == TestRunSource::Studio && !run.status.is_finished())
        {
            run.status = TestRunStatus::Failed;
            run.finished_at = Some(Utc::now());
            run.exit_code = Some(1);
            run.error = Some("Studio stopped before the test run finished".to_owned());
            persist_test_run(&project_root, run).ok();
        }
        let (events, _) = broadcast::channel(TEST_EVENT_BUFFER);
        Self {
            inner: Arc::new(Inner {
                acton_executable: acton_executable.into(),
                project_root,
                studio_url: studio_url.into(),
                runs: RwLock::new(runs),
                sequences: Mutex::new(HashMap::new()),
                persistence: Mutex::new(()),
                cancellations: Mutex::new(HashMap::new()),
                events,
            }),
        }
    }

    async fn refresh_history(&self) -> Result<(), TestRunRuntimeError> {
        let project_root = self.inner.project_root.clone();
        let history = tokio::task::spawn_blocking(move || load_test_runs(&project_root))
            .await
            .map_err(|error| internal("test_history_join_failed", error.to_string()))?
            .map_err(|error| internal("test_history_read_failed", error.to_string()))?;
        let mut runs = self.inner.runs.write().await;
        for (id, run) in history {
            let should_replace = runs.get(&id).is_none_or(|existing| {
                run.status.is_finished()
                    && (!existing.status.is_finished() || run.finished_at > existing.finished_at)
            });
            if should_replace {
                runs.insert(id, run);
            }
        }
        drop(runs);
        Ok(())
    }

    async fn persist(&self, run: TestRunRecord) -> Result<(), TestRunRuntimeError> {
        let _persistence = self.inner.persistence.lock().await;
        let project_root = self.inner.project_root.clone();
        let persisted_run = run.clone();
        tokio::task::spawn_blocking(move || persist_test_run(&project_root, &persisted_run))
            .await
            .map_err(|error| internal("test_history_join_failed", error.to_string()))?
            .map_err(|error| internal("test_history_write_failed", error.to_string()))?;
        self.inner
            .events
            .send(TestRunStreamEvent::RunChanged { run: run.summary() })
            .ok();
        Ok(())
    }

    async fn monitor_process(
        &self,
        run_id: String,
        mut child: tokio::process::Child,
        mut cancellation: watch::Receiver<bool>,
        stdout_task: tokio::task::JoinHandle<()>,
        stderr_task: tokio::task::JoinHandle<()>,
    ) {
        let mut cancelled = false;
        let status = tokio::select! {
            result = child.wait() => result,
            result = cancellation.changed() => {
                if result.is_ok() && *cancellation.borrow() {
                    cancelled = true;
                    child.start_kill().ok();
                }
                child.wait().await
            }
        };
        let _ = stdout_task.await;
        let _ = stderr_task.await;
        self.inner.cancellations.lock().await.remove(&run_id);
        self.refresh_history().await.ok();

        let mut runs = self.inner.runs.write().await;
        let Some(run) = runs.get_mut(&run_id) else {
            return;
        };
        run.finished_at.get_or_insert_with(Utc::now);
        match status {
            Ok(status) => {
                run.exit_code = status.code();
                if cancelled {
                    run.status = TestRunStatus::Cancelled;
                    run.error = Some("Test run was cancelled".to_owned());
                } else if !status.success() {
                    run.status = TestRunStatus::Failed;
                    if run.error.is_none() && run.reports.is_empty() {
                        run.error = Some("acton test exited before producing a report".to_owned());
                    }
                } else if !run.status.is_finished() {
                    run.status = TestRunStatus::Passed;
                }
            }
            Err(error) => {
                run.status = TestRunStatus::Failed;
                run.error = Some(format!("Failed to wait for acton test: {error}"));
            }
        }
        let finished = run.clone();
        drop(runs);
        self.persist(finished).await.ok();
    }

    async fn read_run_output(&self, run_id: &str) -> Result<TestRunOutput, TestRunRuntimeError> {
        if !self.inner.runs.read().await.contains_key(run_id) {
            return Err(TestRunRuntimeError::NotFound {
                run_id: run_id.to_owned(),
            });
        }
        let (stdout_path, stderr_path) = test_output_paths(&self.inner.project_root, run_id);
        let (stdout, stderr) = tokio::join!(
            read_optional_text(stdout_path),
            read_optional_text(stderr_path)
        );
        Ok(TestRunOutput { stdout, stderr })
    }
}

impl TestRunRuntime for LocalProcessTestRunRuntime {
    fn list(&self) -> TestRunRuntimeFuture<'_, Vec<TestRunSummary>> {
        Box::pin(async move {
            self.refresh_history().await?;
            let mut runs = self
                .inner
                .runs
                .read()
                .await
                .values()
                .map(TestRunRecord::summary)
                .collect::<Vec<_>>();
            runs.sort_by_key(|run| Reverse(run.started_at));
            Ok(runs)
        })
    }

    fn get(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        let run_id = run_id.to_owned();
        Box::pin(async move {
            let cached_run = self.inner.runs.read().await.get(&run_id).cloned();
            if let Some(run) = cached_run {
                return Ok(run);
            }
            self.refresh_history().await?;
            self.inner
                .runs
                .read()
                .await
                .get(&run_id)
                .cloned()
                .ok_or_else(|| TestRunRuntimeError::NotFound {
                    run_id: run_id.clone(),
                })
        })
    }

    fn start(&self, request: StartTestRunRequest) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        Box::pin(async move {
            validate_request(&request)?;
            let run_id = new_test_run_id();
            let trace_dir = request
                .save_traces
                .then(|| test_trace_dir(&self.inner.project_root, &run_id));
            if let Some(trace_dir) = &trace_dir {
                tokio::fs::create_dir_all(trace_dir)
                    .await
                    .map_err(|error| internal("test_trace_directory_failed", error.to_string()))?;
            }
            let (stdout_path, stderr_path) = test_output_paths(&self.inner.project_root, &run_id);
            let output_dir = stdout_path.parent().ok_or_else(|| {
                internal(
                    "test_output_directory_failed",
                    "Test output path has no parent".to_owned(),
                )
            })?;
            tokio::fs::create_dir_all(output_dir)
                .await
                .map_err(|error| internal("test_output_directory_failed", error.to_string()))?;

            let args = build_test_args(&request, trace_dir.as_deref());
            let command_display = std::iter::once("acton".to_owned())
                .chain(args.iter().cloned())
                .collect::<Vec<_>>();
            let mut command = Command::new(&self.inner.acton_executable);
            command
                .arg("--project-root")
                .arg(&self.inner.project_root)
                .args(&args)
                .env("ACTON_STUDIO_URL", &self.inner.studio_url)
                .env("ACTON_INTERNAL_SKIP_BROWSER", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command
                .spawn()
                .map_err(|error| internal("test_process_start_failed", error.to_string()))?;
            let stdout = child.stdout.take().ok_or_else(|| {
                internal("test_stdout_unavailable", "stdout is not piped".to_owned())
            })?;
            let stderr = child.stderr.take().ok_or_else(|| {
                internal("test_stderr_unavailable", "stderr is not piped".to_owned())
            })?;

            let run = TestRunRecord::new(
                run_id.clone(),
                self.inner.project_root.clone(),
                TestRunSource::Studio,
                command_display,
                trace_dir,
            );
            self.inner
                .runs
                .write()
                .await
                .insert(run_id.clone(), run.clone());
            self.persist(run.clone()).await?;

            let stdout_task = tokio::spawn(capture_output(
                stdout,
                stdout_path,
                run_id.clone(),
                TestOutputStream::Stdout,
                self.inner.events.clone(),
            ));
            let stderr_task = tokio::spawn(capture_output(
                stderr,
                stderr_path,
                run_id.clone(),
                TestOutputStream::Stderr,
                self.inner.events.clone(),
            ));
            let (cancel, cancel_rx) = watch::channel(false);
            self.inner
                .cancellations
                .lock()
                .await
                .insert(run_id.clone(), cancel);
            let runtime = self.clone();
            tokio::spawn(async move {
                runtime
                    .monitor_process(run_id, child, cancel_rx, stdout_task, stderr_task)
                    .await;
            });
            Ok(run)
        })
    }

    fn cancel(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        let run_id = run_id.to_owned();
        Box::pin(async move {
            let cancellations = self.inner.cancellations.lock().await;
            let Some(cancel) = cancellations.get(&run_id) else {
                let run = self
                    .inner
                    .runs
                    .read()
                    .await
                    .get(&run_id)
                    .cloned()
                    .ok_or_else(|| TestRunRuntimeError::NotFound {
                        run_id: run_id.clone(),
                    })?;
                return Err(TestRunRuntimeError::Conflict {
                    code: "test_run_not_cancellable",
                    message: format!("Test run {} is already {:?}", run.id, run.status),
                });
            };
            cancel
                .send(true)
                .map_err(|_| TestRunRuntimeError::Conflict {
                    code: "test_run_not_cancellable",
                    message: format!("Test run {run_id} is already stopping"),
                })?;
            drop(cancellations);
            self.get(&run_id).await
        })
    }

    fn ingest(&self, envelope: TestRunEventEnvelope) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        Box::pin(async move {
            if envelope.format_version != STUDIO_TEST_RUN_FORMAT_VERSION {
                return Err(TestRunRuntimeError::InvalidRequest {
                    code: "unsupported_test_event_version",
                    message: format!(
                        "Unsupported test event format version {}",
                        envelope.format_version
                    ),
                });
            }
            if !is_valid_test_run_id(&envelope.run_id) {
                return Err(TestRunRuntimeError::InvalidRequest {
                    code: "invalid_test_run_id",
                    message: "Test run ID contains unsupported characters".to_owned(),
                });
            }
            match &envelope.event {
                TestRunEvent::RunStarted { run } | TestRunEvent::RunFinished { run }
                    if run.id != envelope.run_id =>
                {
                    return Err(TestRunRuntimeError::InvalidRequest {
                        code: "test_run_id_mismatch",
                        message: "The event payload run ID does not match its envelope".to_owned(),
                    });
                }
                _ => {}
            }
            let mut sequences = self.inner.sequences.lock().await;
            if sequences
                .get(&envelope.run_id)
                .is_some_and(|sequence| envelope.sequence <= *sequence)
            {
                drop(sequences);
                return self.get(&envelope.run_id).await;
            }
            sequences.insert(envelope.run_id.clone(), envelope.sequence);
            drop(sequences);

            let mut runs = self.inner.runs.write().await;
            let changed = match &envelope.event {
                TestRunEvent::RunStarted { run } => {
                    if !runs
                        .get(&envelope.run_id)
                        .is_some_and(|existing| existing.status.is_finished())
                    {
                        runs.insert(envelope.run_id.clone(), run.clone());
                        true
                    } else {
                        false
                    }
                }
                TestRunEvent::TestFinished { report } => {
                    let run = runs.get_mut(&envelope.run_id).ok_or_else(|| {
                        TestRunRuntimeError::NotFound {
                            run_id: envelope.run_id.clone(),
                        }
                    })?;
                    if let Some(existing) = run.reports.iter_mut().find(|candidate| {
                        candidate.file_path == report.file_path
                            && candidate.name == report.name
                            && candidate.row == report.row
                            && candidate.column == report.column
                    }) {
                        *existing = report.clone();
                    } else {
                        run.reports.push(report.clone());
                    }
                    true
                }
                TestRunEvent::RunFinished { run } => {
                    runs.insert(envelope.run_id.clone(), run.clone());
                    true
                }
                TestRunEvent::SuiteStarted { .. }
                | TestRunEvent::SuiteFinished { .. }
                | TestRunEvent::TestStarted { .. } => false,
            };
            let run = runs.get(&envelope.run_id).cloned().ok_or_else(|| {
                TestRunRuntimeError::NotFound {
                    run_id: envelope.run_id.clone(),
                }
            })?;
            drop(runs);
            self.inner
                .events
                .send(TestRunStreamEvent::ReporterEvent {
                    event: Box::new(envelope.clone()),
                })
                .ok();
            if run.status.is_finished() {
                self.persist(run.clone()).await?;
            } else if changed {
                self.inner
                    .events
                    .send(TestRunStreamEvent::RunChanged { run: run.summary() })
                    .ok();
            }
            Ok(run)
        })
    }

    fn output(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunOutput> {
        let run_id = run_id.to_owned();
        Box::pin(async move { self.read_run_output(&run_id).await })
    }

    fn subscribe(&self) -> broadcast::Receiver<TestRunStreamEvent> {
        self.inner.events.subscribe()
    }

    fn shutdown(&self) -> TestRunRuntimeFuture<'_, ()> {
        Box::pin(async move {
            let cancellations = self.inner.cancellations.lock().await;
            for cancel in cancellations.values() {
                cancel.send(true).ok();
            }
            drop(cancellations);
            Ok(())
        })
    }
}

fn validate_request(request: &StartTestRunRequest) -> Result<(), TestRunRuntimeError> {
    if request.paths.iter().any(|path| path.trim().is_empty()) {
        return Err(TestRunRuntimeError::InvalidRequest {
            code: "invalid_test_path",
            message: "Test paths cannot be empty".to_owned(),
        });
    }
    if request
        .filter
        .as_deref()
        .is_some_and(|filter| filter.trim().is_empty())
    {
        return Err(TestRunRuntimeError::InvalidRequest {
            code: "invalid_test_filter",
            message: "Test filter cannot be empty".to_owned(),
        });
    }
    Ok(())
}

fn build_test_args(request: &StartTestRunRequest, trace_dir: Option<&Path>) -> Vec<String> {
    let mut args = vec!["test".to_owned()];
    args.extend(request.paths.iter().cloned());
    if let Some(filter) = &request.filter {
        args.push("--filter".to_owned());
        args.push(filter.clone());
    }
    for include in &request.include {
        args.push("--include".to_owned());
        args.push(include.clone());
    }
    for exclude in &request.exclude {
        args.push("--exclude".to_owned());
        args.push(exclude.clone());
    }
    if request.fail_fast {
        args.push("--fail-fast".to_owned());
    }
    if let Some(trace_dir) = trace_dir {
        args.push("--save-test-trace".to_owned());
        args.push(trace_dir.to_string_lossy().into_owned());
    }
    args
}

async fn capture_output<R>(
    mut reader: R,
    output_path: PathBuf,
    run_id: String,
    stream: TestOutputStream,
    events: broadcast::Sender<TestRunStreamEvent>,
) where
    R: AsyncRead + Unpin,
{
    let Ok(mut output) = tokio::fs::File::create(output_path).await else {
        return;
    };
    let mut buffer = [0_u8; 8192];
    while let Ok(read) = reader.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        if output.write_all(&buffer[..read]).await.is_err() {
            break;
        }
        let chunk = String::from_utf8_lossy(&buffer[..read]).into_owned();
        events
            .send(TestRunStreamEvent::Output {
                run_id: run_id.clone(),
                stream,
                chunk,
            })
            .ok();
    }
}

async fn read_optional_text(path: PathBuf) -> String {
    match tokio::fs::read(path).await {
        Ok(contents) => String::from_utf8_lossy(&contents).into_owned(),
        Err(_) => String::new(),
    }
}

const fn internal(code: &'static str, message: String) -> TestRunRuntimeError {
    TestRunRuntimeError::Internal { code, message }
}
