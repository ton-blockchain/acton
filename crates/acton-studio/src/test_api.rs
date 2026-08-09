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
use utoipa::{OpenApi, ToSchema};

use crate::{
    StartTestRunRequest, StudioApiErrorBody, StudioState, StudioTestExecutionLogs,
    StudioTestReport, TestRunEventEnvelope, TestRunRuntimeError, test_contract_artifact_file_name,
};

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    TestApiDoc::openapi()
}

#[derive(OpenApi)]
#[openapi(
    paths(
        list_test_runs,
        start_test_run,
        test_run_events,
        get_test_run,
        cancel_test_run,
        ingest_test_run_event,
        get_test_run_output,
        get_test_reports,
        get_test_logs,
        get_test_trace,
        get_test_contract,
        get_test_source_file,
        coverage_artifact,
        gas_profile_artifact,
        get_test_artifact_config,
        test_artifact_health
    ),
    components(schemas(
        StartTestRunRequest,
        crate::StudioTestDuration,
        StudioTestExecutionLogs,
        StudioTestReport,
        crate::TestDescriptorSummary,
        crate::TestIdentity,
        crate::TestOutputStream,
        crate::TestRunEvent,
        TestRunEventEnvelope,
        crate::TestRunOutput,
        crate::TestRunRecord,
        crate::TestRunSource,
        crate::TestRunStats,
        crate::TestRunStatus,
        crate::TestRunStreamEvent,
        crate::TestRunSummary,
        TestArtifactConfig,
        StudioApiErrorBody
    ))
)]
struct TestApiDoc;

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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs",
    responses(
        (status = 200, description = "Saved and active test runs", body = [crate::TestRunSummary]),
        (status = 500, description = "Failed to list test runs", body = StudioApiErrorBody)
    ),
    tag = "test runs"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/test-runs",
    request_body = StartTestRunRequest,
    responses(
        (status = 201, description = "Test run started", body = crate::TestRunRecord),
        (status = 400, description = "Invalid test run request", body = StudioApiErrorBody),
        (status = 409, description = "Test run conflicts with current state", body = StudioApiErrorBody),
        (status = 500, description = "Failed to start the test run", body = StudioApiErrorBody)
    ),
    tag = "test runs"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses(
        (status = 200, description = "Test run details", body = crate::TestRunRecord),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read the test run", body = StudioApiErrorBody)
    ),
    tag = "test runs"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/test-runs/{run_id}/cancel",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses(
        (status = 200, description = "Test run cancelled", body = crate::TestRunRecord),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 409, description = "Test run cannot be cancelled", body = StudioApiErrorBody),
        (status = 500, description = "Failed to cancel the test run", body = StudioApiErrorBody)
    ),
    tag = "test runs"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/test-runs/{run_id}/events",
    params(("run_id" = String, Path, description = "Test run ID")),
    request_body = TestRunEventEnvelope,
    responses(
        (status = 200, description = "Updated test run", body = crate::TestRunRecord),
        (status = 400, description = "Invalid event", body = StudioApiErrorBody),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 409, description = "Event conflicts with the current run", body = StudioApiErrorBody),
        (status = 500, description = "Failed to ingest the event", body = StudioApiErrorBody)
    ),
    tag = "test runs"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/output",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses(
        (status = 200, description = "Captured standard output and error", body = crate::TestRunOutput),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read test output", body = StudioApiErrorBody)
    ),
    tag = "test runs"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/events",
    responses((
        status = 200,
        description = "Test run event stream. Each event uses the test-run event name.",
        body = crate::TestRunStreamEvent,
        content_type = "text/event-stream"
    )),
    tag = "test runs"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/reports",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses(
        (status = 200, description = "Test reports", body = [StudioTestReport]),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read reports", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/test-logs",
    params(
        ("run_id" = String, Path, description = "Test run ID"),
        ("file_path" = String, Query, description = "Test source file path"),
        ("name" = String, Query, description = "Test name"),
        ("row" = usize, Query, description = "Test row"),
        ("column" = usize, Query, description = "Test column")
    ),
    responses(
        (status = 200, description = "Logs for one test", body = StudioTestExecutionLogs),
        (status = 404, description = "Test run or test not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read test logs", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/trace/{name}",
    params(
        ("run_id" = String, Path, description = "Test run ID"),
        ("name" = String, Path, description = "Trace artifact path")
    ),
    responses(
        (status = 200, description = "Trace artifact", body = Object),
        (status = 204, description = "Trace artifact is not available"),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read the trace artifact", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/contract/{name}",
    params(
        ("run_id" = String, Path, description = "Test run ID"),
        ("name" = String, Path, description = "Contract name")
    ),
    responses(
        (status = 200, description = "Contract artifact", body = Object),
        (status = 204, description = "Contract artifact is not available"),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read the contract artifact", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/file",
    params(
        ("run_id" = String, Path, description = "Test run ID"),
        ("path" = String, Query, description = "Source file path inside the project")
    ),
    responses(
        (status = 200, description = "Source file", body = String, content_type = "text/plain"),
        (status = 403, description = "The source path is outside the project", body = StudioApiErrorBody),
        (status = 404, description = "Test run or source file not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read the source file", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[derive(Serialize, ToSchema)]
struct TestArtifactConfig {
    project_root: String,
    coverage_available: bool,
    gas_profile_available: bool,
}

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/config",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses(
        (status = 200, description = "Artifact capabilities for the test run", body = TestArtifactConfig),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to read artifact configuration", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/health",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses(
        (status = 204, description = "Test artifacts are available"),
        (status = 404, description = "Test run not found", body = StudioApiErrorBody),
        (status = 500, description = "Failed to check test artifacts", body = StudioApiErrorBody)
    ),
    tag = "test artifacts"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/coverage.lcov",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses((status = 204, description = "Coverage data is not available")),
    tag = "test artifacts"
)]
#[allow(dead_code, reason = "documentation-only OpenAPI path")]
const fn coverage_artifact() {}

#[utoipa::path(
    get,
    path = "/api/v1/test-runs/{run_id}/artifacts/gas-profile",
    params(("run_id" = String, Path, description = "Test run ID")),
    responses((status = 204, description = "Gas profile data is not available")),
    tag = "test artifacts"
)]
#[allow(dead_code, reason = "documentation-only OpenAPI path")]
const fn gas_profile_artifact() {}

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
            Json(StudioApiErrorBody {
                error: crate::StudioApiErrorDetails {
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
