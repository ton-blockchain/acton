use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use futures::stream;
use serde::{Deserialize, Serialize};

use crate::{
    StartTestRunRequest, StudioState, StudioTestExecutionLogs, StudioTestReport,
    TestRunEventEnvelope, TestRunRuntimeError, test_contract_artifact_file_name,
};

pub(super) fn router() -> Router<StudioState> {
    Router::new()
        .route("/test-runs", get(list_test_runs).post(start_test_run))
        .route("/test-runs/events", get(test_run_events))
        .route("/test-runs/{run_id}", get(get_test_run))
        .route("/test-runs/{run_id}/cancel", post(cancel_test_run))
        .route("/test-runs/{run_id}/events", post(ingest_test_run_event))
        .route("/test-runs/{run_id}/output", get(get_test_run_output))
        .route(
            "/test-runs/{run_id}/artifacts/reports",
            get(get_test_reports),
        )
        .route(
            "/test-runs/{run_id}/artifacts/test-logs",
            get(get_test_logs),
        )
        .route(
            "/test-runs/{run_id}/artifacts/trace/{*name}",
            get(get_test_trace),
        )
        .route(
            "/test-runs/{run_id}/artifacts/contract/{name}",
            get(get_test_contract),
        )
        .route(
            "/test-runs/{run_id}/artifacts/file",
            get(get_test_source_file),
        )
        .route(
            "/test-runs/{run_id}/artifacts/coverage.lcov",
            get(no_optional_artifact),
        )
        .route(
            "/test-runs/{run_id}/artifacts/gas-profile",
            get(no_optional_artifact),
        )
        .route(
            "/test-runs/{run_id}/artifacts/config",
            get(get_test_artifact_config),
        )
        .route(
            "/test-runs/{run_id}/artifacts/health",
            get(test_artifact_health),
        )
}

async fn list_test_runs(
    State(state): State<StudioState>,
) -> Result<Json<Vec<crate::TestRunSummary>>, TestRunApiError> {
    state
        .test_run_runtime
        .list()
        .await
        .map(Json)
        .map_err(TestRunApiError)
}

async fn start_test_run(
    State(state): State<StudioState>,
    Json(request): Json<StartTestRunRequest>,
) -> Result<(StatusCode, Json<crate::TestRunRecord>), TestRunApiError> {
    state
        .test_run_runtime
        .start(request)
        .await
        .map(|run| (StatusCode::CREATED, Json(run)))
        .map_err(TestRunApiError)
}

async fn get_test_run(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<Json<crate::TestRunRecord>, TestRunApiError> {
    state
        .test_run_runtime
        .get(&run_id)
        .await
        .map(Json)
        .map_err(TestRunApiError)
}

async fn cancel_test_run(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<Json<crate::TestRunRecord>, TestRunApiError> {
    state
        .test_run_runtime
        .cancel(&run_id)
        .await
        .map(Json)
        .map_err(TestRunApiError)
}

async fn ingest_test_run_event(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
    Json(event): Json<TestRunEventEnvelope>,
) -> Result<Json<crate::TestRunRecord>, TestRunApiError> {
    if event.run_id != run_id {
        return Err(TestRunApiError(TestRunRuntimeError::InvalidRequest {
            code: "test_run_id_mismatch",
            message: "The event run ID does not match the request path".to_owned(),
        }));
    }
    state
        .test_run_runtime
        .ingest(event)
        .await
        .map(Json)
        .map_err(TestRunApiError)
}

async fn get_test_run_output(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<Json<crate::TestRunOutput>, TestRunApiError> {
    state
        .test_run_runtime
        .output(&run_id)
        .await
        .map(Json)
        .map_err(TestRunApiError)
}

async fn test_run_events(
    State(state): State<StudioState>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.test_run_runtime.subscribe();
    let events = stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    let Ok(data) = serde_json::to_string(&event) else {
                        continue;
                    };
                    return Some((Ok(Event::default().event("test-run").data(data)), receiver));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn get_test_reports(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<Json<Vec<StudioTestReport>>, TestRunApiError> {
    state
        .test_run_runtime
        .get(&run_id)
        .await
        .map(|run| Json(run.reports))
        .map_err(TestRunApiError)
}

#[derive(Deserialize)]
struct TestLogsQuery {
    file_path: PathBuf,
    name: String,
    row: usize,
    column: usize,
}

async fn get_test_logs(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
    Query(query): Query<TestLogsQuery>,
) -> Result<Json<StudioTestExecutionLogs>, TestRunApiError> {
    let run = state
        .test_run_runtime
        .get(&run_id)
        .await
        .map_err(TestRunApiError)?;
    let report = run
        .reports
        .iter()
        .find(|report| {
            report.file_path == query.file_path
                && report.name == query.name
                && report.row == query.row
                && report.column == query.column
        })
        .ok_or_else(|| {
            TestRunApiError(TestRunRuntimeError::NotFound {
                run_id: format!("{run_id}/test"),
            })
        })?;
    Ok(Json(report.execution_logs.clone()))
}

async fn get_test_trace(
    State(state): State<StudioState>,
    AxumPath((run_id, name)): AxumPath<(String, String)>,
) -> Result<Response, TestRunApiError> {
    let run = state
        .test_run_runtime
        .get(&run_id)
        .await
        .map_err(TestRunApiError)?;
    let trace_dir = run.trace_dir.ok_or_else(|| {
        TestRunApiError(TestRunRuntimeError::NotFound {
            run_id: format!("{run_id}/trace"),
        })
    })?;
    read_json_artifact(&trace_dir, Path::new(&name), true).await
}

async fn get_test_contract(
    State(state): State<StudioState>,
    AxumPath((run_id, name)): AxumPath<(String, String)>,
) -> Result<Response, TestRunApiError> {
    let run = state
        .test_run_runtime
        .get(&run_id)
        .await
        .map_err(TestRunApiError)?;
    let trace_dir = run.trace_dir.ok_or_else(|| {
        TestRunApiError(TestRunRuntimeError::NotFound {
            run_id: format!("{run_id}/contract"),
        })
    })?;
    let file_name = test_contract_artifact_file_name(&name);
    read_json_artifact(&trace_dir.join("contracts"), Path::new(&file_name), false).await
}

#[derive(Deserialize)]
struct FileQuery {
    path: PathBuf,
}

async fn get_test_source_file(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
    Query(query): Query<FileQuery>,
) -> Result<Response, TestRunApiError> {
    let run = state
        .test_run_runtime
        .get(&run_id)
        .await
        .map_err(TestRunApiError)?;
    let path = resolve_existing_path(&run.project_root, &query.path)
        .await
        .ok_or_else(|| forbidden("test_source_outside_project", "Access denied"))?;
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(contents.into_response()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(StatusCode::NOT_FOUND.into_response())
        }
        Err(error) => Err(internal("test_source_read_failed", error.to_string())),
    }
}

#[derive(Serialize)]
struct TestArtifactConfig {
    project_root: String,
    coverage_available: bool,
    gas_profile_available: bool,
}

async fn get_test_artifact_config(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<Json<TestArtifactConfig>, TestRunApiError> {
    let run = state
        .test_run_runtime
        .get(&run_id)
        .await
        .map_err(TestRunApiError)?;
    let mut project_root = run.project_root.to_string_lossy().into_owned();
    if !project_root.ends_with(std::path::MAIN_SEPARATOR) {
        project_root.push(std::path::MAIN_SEPARATOR);
    }
    Ok(Json(TestArtifactConfig {
        project_root,
        coverage_available: false,
        gas_profile_available: false,
    }))
}

async fn test_artifact_health(
    State(state): State<StudioState>,
    AxumPath(run_id): AxumPath<String>,
) -> Result<StatusCode, TestRunApiError> {
    state
        .test_run_runtime
        .get(&run_id)
        .await
        .map_err(TestRunApiError)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn no_optional_artifact() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn read_json_artifact(
    root: &Path,
    requested: &Path,
    empty_is_no_content: bool,
) -> Result<Response, TestRunApiError> {
    let Some(path) = resolve_existing_path(root, requested).await else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };
    let contents = tokio::fs::read_to_string(path)
        .await
        .map_err(|error| internal("test_artifact_read_failed", error.to_string()))?;
    if empty_is_no_content && contents.trim().is_empty() {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    let value = serde_json::from_str::<serde_json::Value>(&contents)
        .map_err(|error| internal("test_artifact_invalid_json", error.to_string()))?;
    Ok(Json(value).into_response())
}

async fn resolve_existing_path(root: &Path, requested: &Path) -> Option<PathBuf> {
    let root = tokio::fs::canonicalize(root).await.ok()?;
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let candidate = tokio::fs::canonicalize(candidate).await.ok()?;
    candidate.starts_with(&root).then_some(candidate)
}

struct TestRunApiError(TestRunRuntimeError);

#[derive(Serialize)]
struct TestRunApiErrorBody {
    error: TestRunApiErrorDetails,
}

#[derive(Serialize)]
struct TestRunApiErrorDetails {
    code: &'static str,
    message: String,
}

impl IntoResponse for TestRunApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            TestRunRuntimeError::InvalidRequest { code, .. } => (StatusCode::BAD_REQUEST, *code),
            TestRunRuntimeError::Conflict { code, .. } => (StatusCode::CONFLICT, *code),
            TestRunRuntimeError::NotFound { .. } => (StatusCode::NOT_FOUND, "test_run_not_found"),
            TestRunRuntimeError::Internal { code, .. } => {
                (StatusCode::INTERNAL_SERVER_ERROR, *code)
            }
        };
        (
            status,
            Json(TestRunApiErrorBody {
                error: TestRunApiErrorDetails {
                    code,
                    message: self.0.to_string(),
                },
            }),
        )
            .into_response()
    }
}

fn forbidden(code: &'static str, message: &str) -> TestRunApiError {
    TestRunApiError(TestRunRuntimeError::InvalidRequest {
        code,
        message: message.to_owned(),
    })
}

const fn internal(code: &'static str, message: String) -> TestRunApiError {
    TestRunApiError(TestRunRuntimeError::Internal { code, message })
}
