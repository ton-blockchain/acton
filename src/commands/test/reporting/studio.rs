use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use acton_config::test::TestConfig;
use acton_studio::{
    STUDIO_TEST_RUN_FORMAT_VERSION, STUDIO_TEST_RUNS_PATH, StudioTestDuration,
    StudioTestExecutionLogs, StudioTestReport, TestDescriptorSummary, TestIdentity, TestRunEvent,
    TestRunEventEnvelope, TestRunRecord, TestRunSource, TestRunStats, TestRunStatus,
    is_valid_test_run_id, new_test_run_id, persist_test_run, test_trace_dir,
};
use chrono::Utc;
use crossbeam_channel::{Sender, TrySendError};
use serde_json::json;

use super::{TestReport, TestReporter, TestStatus, TestSuiteStats};
use crate::commands::test::TestDescriptor;
use crate::formatter::FormatterContext;
use crate::studio_discovery;

const STUDIO_EVENT_QUEUE_CAPACITY: usize = 8192;
const STUDIO_CONNECT_TIMEOUT: Duration = Duration::from_millis(100);
const STUDIO_REQUEST_TIMEOUT: Duration = Duration::from_millis(250);

pub(crate) struct StudioReporter {
    project_root: PathBuf,
    run: TestRunRecord,
    event_worker: Option<StudioEventWorker>,
    final_event: Option<TestRunEventEnvelope>,
    sequence: u64,
    finished: bool,
    dropped_events: Arc<AtomicUsize>,
}

struct StudioEventWorker {
    client: reqwest::blocking::Client,
    endpoint: String,
    events: Option<Sender<TestRunEventEnvelope>>,
}

impl StudioReporter {
    pub(crate) fn prepare(
        project_root: &Path,
        workspace_name: &str,
        config: &mut TestConfig,
    ) -> Option<Self> {
        if !config.studio_reporting {
            return None;
        }

        let run_id = std::env::var("ACTON_STUDIO_RUN_ID")
            .ok()
            .filter(|value| is_valid_test_run_id(value))
            .unwrap_or_else(new_test_run_id);

        let source = match std::env::var("ACTON_STUDIO_RUN_SOURCE").as_deref() {
            Ok("studio") => TestRunSource::Studio,
            _ => TestRunSource::Manual,
        };

        let project_root =
            dunce::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
        let dropped_events = Arc::new(AtomicUsize::new(0));
        let studio_url = studio_discovery::running_studio_url(&project_root, workspace_name)?;

        let event_worker =
            start_event_worker(studio_url, run_id.clone(), Arc::clone(&dropped_events))?;

        if source == TestRunSource::Manual && config.save_test_trace.is_none() {
            config.save_test_trace = Some(
                test_trace_dir(&project_root, &run_id)
                    .to_string_lossy()
                    .into_owned(),
            );
        }

        let trace_dir = config.save_test_trace.as_deref().map(PathBuf::from);

        let run = TestRunRecord::new(
            run_id,
            project_root.clone(),
            source,
            std::env::args().collect(),
            trace_dir,
        );

        Some(Self {
            project_root,
            run,
            event_worker: Some(event_worker),
            final_event: None,
            sequence: 0,
            finished: false,
            dropped_events,
        })
    }

    fn next_event(&mut self, event: TestRunEvent) -> TestRunEventEnvelope {
        self.sequence = self.sequence.saturating_add(1);
        TestRunEventEnvelope {
            format_version: STUDIO_TEST_RUN_FORMAT_VERSION,
            run_id: self.run.id.clone(),
            sequence: self.sequence,
            event,
        }
    }

    fn emit(&mut self, event: TestRunEvent) {
        let envelope = self.next_event(event);
        let Some(events) = self
            .event_worker
            .as_ref()
            .and_then(|worker| worker.events.as_ref())
        else {
            return;
        };
        if let Err(error) = events.try_send(envelope) {
            match error {
                TrySendError::Full(_) => {
                    self.dropped_events.fetch_add(1, Ordering::Relaxed);
                }
                TrySendError::Disconnected(_) => {
                    if let Some(worker) = &mut self.event_worker {
                        worker.events = None;
                    }
                }
            }
        }
    }

    fn flush_final_event(&mut self) {
        let Some(event) = self.final_event.take() else {
            return;
        };
        let Some(worker) = self.event_worker.take() else {
            return;
        };
        if let Err(error) = worker
            .client
            .post(&worker.endpoint)
            .json(&event)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
        {
            log::debug!(
                "Failed to publish the final Studio event for run {}: {error}",
                self.run.id
            );
        }
    }

    fn persist(&self) {
        if let Err(error) = persist_test_run(&self.project_root, &self.run) {
            log::warn!(
                "Failed to save Studio test history for run {}: {error}",
                self.run.id
            );
        }
    }

    fn finish(&mut self, stats: TestRunStats, status: TestRunStatus, error: Option<String>) {
        if self.finished {
            return;
        }
        let finished_at = Utc::now();
        self.run.stats = stats;
        self.run.status = status;
        self.run.finished_at = Some(finished_at);
        self.run.exit_code = Some(i32::from(status != TestRunStatus::Passed));
        self.run.error = error;
        self.persist();
        if self.event_worker.is_some() {
            let run = self.run.clone();
            self.final_event = Some(self.next_event(TestRunEvent::RunFinished { run }));
        }
        self.finished = true;
    }
}

impl TestReporter for StudioReporter {
    fn on_testing_started(&mut self) -> anyhow::Result<()> {
        if self.event_worker.is_some() {
            self.emit(TestRunEvent::RunStarted {
                run: self.run.clone(),
            });
        }
        Ok(())
    }

    fn on_testing_finished(&mut self, stats: &TestSuiteStats) -> anyhow::Result<()> {
        self.run.stats = test_stats(stats);
        Ok(())
    }

    fn on_run_finished(&mut self, success: bool) -> anyhow::Result<()> {
        let status = if success {
            TestRunStatus::Passed
        } else {
            TestRunStatus::Failed
        };
        self.finish(self.run.stats.clone(), status, None);
        Ok(())
    }

    fn on_suite_started(
        &mut self,
        file_path: &Path,
        tests: &[TestDescriptor],
    ) -> anyhow::Result<()> {
        if self.event_worker.is_none() {
            return Ok(());
        }
        self.emit(TestRunEvent::SuiteStarted {
            file_path: file_path.to_path_buf(),
            tests: tests
                .iter()
                .map(|test| TestDescriptorSummary {
                    name: test.name.to_string(),
                    row: test.pos.row,
                    column: test.pos.column,
                })
                .collect(),
        });
        Ok(())
    }

    fn on_suite_finished(
        &mut self,
        file_path: &Path,
        stats: &TestSuiteStats,
    ) -> anyhow::Result<()> {
        if self.event_worker.is_none() {
            return Ok(());
        }
        self.emit(TestRunEvent::SuiteFinished {
            file_path: file_path.to_path_buf(),
            stats: test_stats(stats),
        });
        Ok(())
    }

    fn on_test_started(&mut self, test: &TestReport) -> anyhow::Result<()> {
        if self.event_worker.is_none() {
            return Ok(());
        }
        self.emit(TestRunEvent::TestStarted {
            test: TestIdentity {
                name: test.name.to_string(),
                suite_name: test.suite_name.to_string(),
                file_path: test.file_path.clone(),
                row: test.row,
                column: test.column,
            },
        });
        Ok(())
    }

    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        if self.event_worker.is_none() {
            return Ok(());
        }

        let report = studio_test_report(test);
        self.run.reports.push(report.clone());
        self.emit(TestRunEvent::TestFinished { report });
        Ok(())
    }

    fn finalize(&mut self) -> anyhow::Result<()> {
        self.flush_final_event();
        let dropped = self.dropped_events.load(Ordering::Relaxed);
        if dropped > 0 {
            log::debug!("Studio reporter dropped {dropped} live events because its queue was full");
        }
        Ok(())
    }
}

impl Drop for StudioReporter {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(
                self.run.stats.clone(),
                TestRunStatus::Failed,
                Some("acton test ended before producing a final report".to_owned()),
            );
        }
        self.flush_final_event();
    }
}

fn start_event_worker(
    studio_url: String,
    run_id: String,
    dropped_events: Arc<AtomicUsize>,
) -> Option<StudioEventWorker> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(STUDIO_CONNECT_TIMEOUT)
        .timeout(STUDIO_REQUEST_TIMEOUT)
        .build()
        .ok()?;

    let endpoint = format!("{studio_url}{STUDIO_TEST_RUNS_PATH}/{run_id}/events");
    let (sender, receiver) = crossbeam_channel::bounded(STUDIO_EVENT_QUEUE_CAPACITY);
    let worker_client = client.clone();
    let worker_endpoint = endpoint.clone();

    std::thread::Builder::new()
        .name("acton-studio-reporter".to_owned())
        .spawn(move || {
            for event in receiver {
                if worker_client
                    .post(&worker_endpoint)
                    .json(&event)
                    .send()
                    .and_then(reqwest::blocking::Response::error_for_status)
                    .is_err()
                {
                    break;
                }
            }
            let dropped = dropped_events.load(Ordering::Relaxed);
            if dropped > 0 {
                log::debug!("Studio reporter worker observed {dropped} dropped live events");
            }
        })
        .ok()?;

    Some(StudioEventWorker {
        client,
        endpoint,
        events: Some(sender),
    })
}

fn test_stats(stats: &TestSuiteStats) -> TestRunStats {
    TestRunStats {
        total: stats.total,
        passed: stats.passed,
        failed: stats.failed,
        skipped: stats.skipped,
        todo: stats.todo,
        duration_ms: stats.duration.as_millis().try_into().unwrap_or(u64::MAX),
    }
}

fn studio_test_report(test: &TestReport) -> StudioTestReport {
    let execution_logs =
        test.execution
            .as_ref()
            .map_or_else(StudioTestExecutionLogs::default, |execution| {
                StudioTestExecutionLogs {
                    stdout: non_empty_text(&execution.stdout),
                    stderr: non_empty_text(&execution.stderr),
                    vm_log: execution.vm_log.as_deref().and_then(non_empty_text),
                }
            });
    let execution = test
        .execution
        .as_ref()
        .and_then(|execution| execution.fuzz.as_ref().map(|fuzz| json!({"fuzz": fuzz})));

    StudioTestReport {
        name: test.name.to_string(),
        suite_name: test.suite_name.to_string(),
        file_path: test.file_path.clone(),
        row: test.row,
        column: test.column,
        duration: StudioTestDuration {
            secs: test.duration.as_secs(),
            nanos: test.duration.subsec_nanos(),
        },
        status: test_status(&test.status).to_owned(),
        message: sanitize_optional_text(test.message.as_deref()),
        detailed_message: sanitize_optional_text(test.detailed_message.as_deref()),
        failed_transactions: test
            .failed_transactions
            .as_ref()
            .and_then(|transactions| serde_json::to_value(transactions).ok()),
        failed_transaction_context: test
            .failed_transaction_context
            .as_ref()
            .and_then(|context| serde_json::to_value(context).ok()),
        details: sanitize_optional_text(test.details.as_deref()),
        location: test
            .location
            .as_ref()
            .and_then(|location| serde_json::to_value(location).ok()),
        execution,
        trace_path: test.trace_path.clone(),
        execution_logs,
    }
}

const fn test_status(status: &TestStatus) -> &'static str {
    match status {
        TestStatus::Passed => "Passed",
        TestStatus::Failed => "Failed",
        TestStatus::Skipped => "Skipped",
        TestStatus::Todo => "Todo",
    }
}

fn sanitize_optional_text(value: Option<&str>) -> Option<String> {
    value.and_then(non_empty_text)
}

fn non_empty_text(value: &str) -> Option<String> {
    let sanitized = FormatterContext::strip_ansi_text(value);
    (!sanitized.trim().is_empty()).then_some(sanitized)
}
