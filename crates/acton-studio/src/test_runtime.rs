use std::future::Future;
use std::pin::Pin;

use tokio::sync::broadcast;

use crate::{
    StartTestRunRequest, TestRunEventEnvelope, TestRunOutput, TestRunRecord, TestRunStreamEvent,
    TestRunSummary,
};

#[derive(Debug, thiserror::Error)]
pub enum TestRunRuntimeError {
    #[error("{message}")]
    InvalidRequest { code: &'static str, message: String },
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("Test run {run_id} was not found")]
    NotFound { run_id: String },
    #[error("{message}")]
    Internal { code: &'static str, message: String },
}

pub type TestRunRuntimeFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, TestRunRuntimeError>> + Send + 'a>>;

pub trait TestRunRuntime: Send + Sync {
    fn list(&self) -> TestRunRuntimeFuture<'_, Vec<TestRunSummary>>;

    fn get(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord>;

    fn start(&self, request: StartTestRunRequest) -> TestRunRuntimeFuture<'_, TestRunRecord>;

    fn cancel(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord>;

    fn ingest(&self, event: TestRunEventEnvelope) -> TestRunRuntimeFuture<'_, TestRunRecord>;

    fn output(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunOutput>;

    fn subscribe(&self) -> broadcast::Receiver<TestRunStreamEvent>;

    fn shutdown(&self) -> TestRunRuntimeFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) struct EmptyTestRunRuntime {
    events: broadcast::Sender<TestRunStreamEvent>,
}

impl EmptyTestRunRuntime {
    pub(crate) fn new() -> Self {
        let (events, _) = broadcast::channel(1);
        Self { events }
    }
}

impl TestRunRuntime for EmptyTestRunRuntime {
    fn list(&self) -> TestRunRuntimeFuture<'_, Vec<TestRunSummary>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn get(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        let run_id = run_id.to_owned();
        Box::pin(async move { Err(TestRunRuntimeError::NotFound { run_id }) })
    }

    fn start(&self, _request: StartTestRunRequest) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        Box::pin(async {
            Err(TestRunRuntimeError::Internal {
                code: "test_runtime_unavailable",
                message: "Test runtime is not configured".to_owned(),
            })
        })
    }

    fn cancel(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        let run_id = run_id.to_owned();
        Box::pin(async move { Err(TestRunRuntimeError::NotFound { run_id }) })
    }

    fn ingest(&self, _event: TestRunEventEnvelope) -> TestRunRuntimeFuture<'_, TestRunRecord> {
        Box::pin(async {
            Err(TestRunRuntimeError::Internal {
                code: "test_runtime_unavailable",
                message: "Test runtime is not configured".to_owned(),
            })
        })
    }

    fn output(&self, run_id: &str) -> TestRunRuntimeFuture<'_, TestRunOutput> {
        let run_id = run_id.to_owned();
        Box::pin(async move { Err(TestRunRuntimeError::NotFound { run_id }) })
    }

    fn subscribe(&self) -> broadcast::Receiver<TestRunStreamEvent> {
        self.events.subscribe()
    }
}
