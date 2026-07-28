use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Request, State};
#[cfg(not(debug_assertions))]
use axum::http::Uri;
use axum::http::{HeaderName, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get, post};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};

mod contract_source_artifact;
mod environment;
mod environment_store;
mod full_ton_network;
mod local_artifacts;
mod local_process;
mod local_test_process;
mod test_api;
mod test_run;
mod test_runtime;
mod wallet;

pub use contract_source_artifact::{
    CONTRACT_SOURCE_HISTORY_PATH, ContractSourceArtifact, ContractSourceArtifactError,
    ContractSourceArtifactStore,
};
pub use environment::{
    CreateEnvironmentConfig, CreateEnvironmentRequest, EnvironmentCapability, EnvironmentConfig,
    EnvironmentEndpoints, EnvironmentNetwork, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentStatus, StudioEnvironment, UpdateEnvironmentRequest,
};
pub use local_process::LocalProcessEnvironmentRuntime;
pub use local_test_process::LocalProcessTestRunRuntime;
pub use test_run::{
    STUDIO_TEST_RUN_FORMAT_VERSION, STUDIO_TEST_RUNS_PATH, StartTestRunRequest,
    StudioDaemonDescriptor, StudioTestDuration, StudioTestExecutionLogs, StudioTestReport,
    TestDescriptorSummary, TestIdentity, TestOutputStream, TestRunEvent, TestRunEventEnvelope,
    TestRunOutput, TestRunRecord, TestRunSource, TestRunStats, TestRunStatus, TestRunStreamEvent,
    TestRunSummary, is_valid_test_run_id, load_studio_daemon_descriptor, load_test_runs,
    new_test_run_id, persist_studio_daemon_descriptor, persist_test_run,
    remove_studio_daemon_descriptor, studio_daemon_descriptor_path,
    test_contract_artifact_file_name, test_history_dir, test_output_paths, test_trace_dir,
};
pub use test_runtime::{TestRunRuntime, TestRunRuntimeError, TestRunRuntimeFuture};
pub use wallet::{
    SignWalletRequest, SignWalletResponse, StudioWallet, WalletRuntime, WalletRuntimeError,
    WalletRuntimeFuture,
};

pub const DEFAULT_STUDIO_PORT: u16 = 3015;
pub const STUDIO_API_VERSION: u32 = 1;
pub const STUDIO_ENVIRONMENTS_PATH: &str = "/api/v1/environments";
pub const STUDIO_HEALTH_PATH: &str = "/api/v1/health";
pub const STUDIO_INFO_PATH: &str = "/api/v1/info";
pub const STUDIO_WALLETS_PATH_SUFFIX: &str = "/wallets";

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/studio-ui/dist");

#[derive(Clone, Debug)]
pub struct StudioWorkspace {
    name: String,
    root: PathBuf,
    wallet_names: Vec<String>,
}

impl StudioWorkspace {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root: root.into(),
            wallet_names: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_wallet_names(mut self, wallet_names: Vec<String>) -> Self {
        self.wallet_names = wallet_names;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn wallet_names(&self) -> &[String] {
        &self.wallet_names
    }
}

#[derive(Clone, Debug)]
pub struct StudioServerConfig {
    server_version: String,
    workspace: Option<StudioWorkspace>,
}

impl StudioServerConfig {
    pub fn new(server_version: impl Into<String>) -> Self {
        Self {
            server_version: server_version.into(),
            workspace: None,
        }
    }

    #[must_use]
    pub fn with_workspace(mut self, workspace: StudioWorkspace) -> Self {
        self.workspace = Some(workspace);
        self
    }
}

#[derive(Clone)]
pub struct StudioServer {
    config: StudioServerConfig,
    environment_runtime: Arc<dyn EnvironmentRuntime>,
    test_run_runtime: Arc<dyn TestRunRuntime>,
    wallet_runtime: Arc<dyn WalletRuntime>,
}

impl StudioServer {
    #[must_use]
    pub fn new(config: StudioServerConfig) -> Self {
        Self {
            config,
            environment_runtime: Arc::new(environment::EmptyEnvironmentRuntime),
            test_run_runtime: Arc::new(test_runtime::EmptyTestRunRuntime::new()),
            wallet_runtime: Arc::new(wallet::EmptyWalletRuntime),
        }
    }

    #[must_use]
    pub fn with_test_run_runtime<R>(mut self, test_run_runtime: R) -> Self
    where
        R: TestRunRuntime + 'static,
    {
        self.test_run_runtime = Arc::new(test_run_runtime);
        self
    }

    #[must_use]
    pub fn with_environment_runtime<R>(mut self, environment_runtime: R) -> Self
    where
        R: EnvironmentRuntime + 'static,
    {
        self.environment_runtime = Arc::new(environment_runtime);
        self
    }

    #[must_use]
    pub fn with_wallet_runtime<R>(mut self, wallet_runtime: R) -> Self
    where
        R: WalletRuntime + 'static,
    {
        self.wallet_runtime = Arc::new(wallet_runtime);
        self
    }

    #[must_use]
    pub const fn workspace(&self) -> Option<&StudioWorkspace> {
        self.config.workspace.as_ref()
    }

    pub fn router(&self) -> Router {
        let state = StudioState {
            info: StudioInfo {
                protocol_version: STUDIO_API_VERSION,
                server_version: self.config.server_version.clone(),
                workspace: self
                    .config
                    .workspace
                    .as_ref()
                    .map(|workspace| WorkspaceInfo {
                        name: workspace.name.clone(),
                        wallet_names: workspace.wallet_names.clone(),
                    }),
            },
            environment_runtime: Arc::clone(&self.environment_runtime),
            test_run_runtime: Arc::clone(&self.test_run_runtime),
            wallet_runtime: Arc::clone(&self.wallet_runtime),
            http_client: reqwest::Client::new(),
        };
        let api = Router::new()
            .route("/health", get(health))
            .route("/info", get(info))
            .route(
                "/environments",
                get(list_environments).post(create_environment),
            )
            .route(
                "/environments/{environment_id}",
                get(get_environment)
                    .patch(update_environment)
                    .delete(delete_environment),
            )
            .route(
                "/environments/{environment_id}/stop",
                post(stop_environment),
            )
            .route(
                "/environments/{environment_id}/restart",
                post(restart_environment),
            )
            .route("/environments/{environment_id}/wallets", get(list_wallets))
            .route(
                "/environments/{environment_id}/wallets/{wallet_name}/sign",
                post(sign_wallet),
            )
            .route(
                "/environments/{environment_id}/rpc",
                any(proxy_environment_rpc_root),
            )
            .route(
                "/environments/{environment_id}/rpc/{*path}",
                any(proxy_environment_rpc),
            )
            .merge(test_api::router())
            .fallback(api_not_found);
        let app = Router::new()
            .nest("/api/v1", api)
            .route("/api", any(api_not_found))
            .route("/api/{*path}", any(api_not_found))
            .with_state(state);

        #[cfg(debug_assertions)]
        {
            let dist_path = PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/studio-ui/dist"
            ));
            app.fallback_service(
                ServeDir::new(&dist_path).fallback(ServeFile::new(dist_path.join("index.html"))),
            )
        }

        #[cfg(not(debug_assertions))]
        {
            app.fallback(embedded_ui)
        }
    }

    pub async fn serve<F>(
        &self,
        listener: TcpListener,
        shutdown: F,
    ) -> Result<(), StudioServerError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let serve_result = axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|source| StudioServerError::Serve { source });
        let shutdown_result = self.environment_runtime.shutdown().await;
        let test_shutdown_result = self.test_run_runtime.shutdown().await;

        serve_result?;
        shutdown_result.map_err(|source| StudioServerError::EnvironmentShutdown { source })?;
        test_shutdown_result.map_err(|source| StudioServerError::TestRunShutdown { source })
    }
}

#[derive(Clone)]
pub(crate) struct StudioState {
    info: StudioInfo,
    environment_runtime: Arc<dyn EnvironmentRuntime>,
    test_run_runtime: Arc<dyn TestRunRuntime>,
    wallet_runtime: Arc<dyn WalletRuntime>,
    http_client: reqwest::Client,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioInfo {
    pub protocol_version: u32,
    pub server_version: String,
    pub workspace: Option<WorkspaceInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wallet_names: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StudioServerError {
    #[error("Studio server stopped with an error")]
    Serve { source: io::Error },
    #[error("Studio environments failed to stop")]
    EnvironmentShutdown { source: EnvironmentRuntimeError },
    #[error("Studio test runs failed to stop")]
    TestRunShutdown { source: TestRunRuntimeError },
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn info(State(state): State<StudioState>) -> Json<StudioInfo> {
    Json(state.info)
}

async fn list_environments(
    State(state): State<StudioState>,
) -> Result<Json<Vec<StudioEnvironment>>, StudioApiError> {
    state
        .environment_runtime
        .list()
        .await
        .map(|environments| Json(environments.into_iter().map(public_environment).collect()))
        .map_err(StudioApiError)
}

async fn create_environment(
    State(state): State<StudioState>,
    Json(request): Json<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<StudioEnvironment>), StudioApiError> {
    state
        .environment_runtime
        .create(request)
        .await
        .map(|environment| (StatusCode::CREATED, Json(public_environment(environment))))
        .map_err(StudioApiError)
}

async fn get_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<Json<StudioEnvironment>, StudioApiError> {
    state
        .environment_runtime
        .get(&environment_id)
        .await
        .map(|environment| Json(public_environment(environment)))
        .map_err(StudioApiError)
}

async fn update_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
    Json(request): Json<UpdateEnvironmentRequest>,
) -> Result<Json<StudioEnvironment>, StudioApiError> {
    state
        .environment_runtime
        .update(&environment_id, request)
        .await
        .map(|environment| Json(public_environment(environment)))
        .map_err(StudioApiError)
}

async fn delete_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<StatusCode, StudioApiError> {
    state
        .environment_runtime
        .delete(&environment_id)
        .await
        .map(|()| StatusCode::NO_CONTENT)
        .map_err(StudioApiError)
}

async fn stop_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<Json<StudioEnvironment>, StudioApiError> {
    state
        .environment_runtime
        .stop(&environment_id)
        .await
        .map(|environment| Json(public_environment(environment)))
        .map_err(StudioApiError)
}

async fn restart_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<Json<StudioEnvironment>, StudioApiError> {
    state
        .environment_runtime
        .restart(&environment_id)
        .await
        .map(|environment| Json(public_environment(environment)))
        .map_err(StudioApiError)
}

async fn list_wallets(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<Json<Vec<StudioWallet>>, WalletApiError> {
    let environment = wallet_environment(&state, &environment_id).await?;
    state
        .wallet_runtime
        .list(&environment)
        .await
        .map(Json)
        .map_err(WalletApiError::Wallet)
}

async fn sign_wallet(
    State(state): State<StudioState>,
    AxumPath((environment_id, wallet_name)): AxumPath<(String, String)>,
    Json(request): Json<SignWalletRequest>,
) -> Result<Json<SignWalletResponse>, WalletApiError> {
    const MAX_SIGNING_PAYLOAD_BYTES: usize = 64 * 1024;

    let environment = wallet_environment(&state, &environment_id).await?;
    let bytes = request.bytes.strip_prefix("0x").ok_or_else(|| {
        WalletApiError::Wallet(WalletRuntimeError::InvalidRequest {
            code: "wallet_signing_payload_invalid",
            message: "Signing bytes must be a 0x-prefixed hexadecimal string".to_owned(),
        })
    })?;
    let bytes = hex::decode(bytes).map_err(|error| {
        WalletApiError::Wallet(WalletRuntimeError::InvalidRequest {
            code: "wallet_signing_payload_invalid",
            message: format!("Signing bytes are not valid hexadecimal: {error}"),
        })
    })?;
    if bytes.len() > MAX_SIGNING_PAYLOAD_BYTES {
        return Err(WalletApiError::Wallet(WalletRuntimeError::InvalidRequest {
            code: "wallet_signing_payload_too_large",
            message: format!("Signing payload exceeds the {MAX_SIGNING_PAYLOAD_BYTES} byte limit"),
        }));
    }

    state
        .wallet_runtime
        .sign(&environment, &wallet_name, bytes)
        .await
        .map(|signature| {
            Json(SignWalletResponse {
                signature: format!("0x{}", hex::encode(signature)),
            })
        })
        .map_err(WalletApiError::Wallet)
}

async fn wallet_environment(
    state: &StudioState,
    environment_id: &str,
) -> Result<StudioEnvironment, WalletApiError> {
    let environment = state
        .environment_runtime
        .get(environment_id)
        .await
        .map_err(WalletApiError::Environment)?;
    if environment.status != EnvironmentStatus::Running {
        return Err(WalletApiError::Environment(
            EnvironmentRuntimeError::Conflict {
                code: "environment_not_running",
                message: format!("Environment {} is not running", environment.name),
            },
        ));
    }
    if !environment
        .capabilities
        .contains(&EnvironmentCapability::Wallets)
    {
        return Err(WalletApiError::Wallet(WalletRuntimeError::InvalidRequest {
            code: "environment_wallets_unavailable",
            message: format!("Wallets are not available in {}", environment.name),
        }));
    }
    Ok(environment)
}

fn public_environment(mut environment: StudioEnvironment) -> StudioEnvironment {
    let proxy_root = format!(
        "{STUDIO_ENVIRONMENTS_PATH}/{}/rpc",
        urlencoding::encode(&environment.id)
    );
    environment.rpc_url.clone_from(&proxy_root);
    environment.endpoints = EnvironmentEndpoints {
        api_v2: environment
            .runtime_endpoints
            .api_v2
            .as_ref()
            .map(|_| format!("{proxy_root}/api/v2")),
        api_v3: environment
            .runtime_endpoints
            .api_v3
            .as_ref()
            .map(|_| format!("{proxy_root}/api/v3")),
        control: environment
            .runtime_endpoints
            .control
            .as_ref()
            .map(|_| proxy_root),
    };
    environment
}

async fn proxy_environment_rpc_root(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
    request: Request,
) -> Result<Response, StudioApiError> {
    proxy_environment_request(state, environment_id, String::new(), request).await
}

async fn proxy_environment_rpc(
    State(state): State<StudioState>,
    AxumPath((environment_id, path)): AxumPath<(String, String)>,
    request: Request,
) -> Result<Response, StudioApiError> {
    proxy_environment_request(state, environment_id, path, request).await
}

async fn proxy_environment_request(
    state: StudioState,
    environment_id: String,
    path: String,
    request: Request,
) -> Result<Response, StudioApiError> {
    let environment = state
        .environment_runtime
        .get(&environment_id)
        .await
        .map_err(StudioApiError)?;
    if environment.status != EnvironmentStatus::Running {
        return Err(StudioApiError(EnvironmentRuntimeError::Conflict {
            code: "environment_not_running",
            message: format!("Environment {} is not running", environment.name),
        }));
    }

    let (parts, body) = request.into_parts();
    let mut upstream_url = environment_upstream_url(&environment, &path)?;
    if let Some(query) = parts.uri.query() {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }

    let mut upstream_request = state.http_client.request(parts.method, upstream_url);
    for (name, value) in &parts.headers {
        if !is_hop_by_hop_header(name) && name != axum::http::header::HOST {
            upstream_request = upstream_request.header(name, value);
        }
    }

    let upstream_response = upstream_request
        .body(reqwest::Body::wrap_stream(body.into_data_stream()))
        .send()
        .await
        .map_err(proxy_error)?;
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let mut response = Response::builder().status(status);
    for (name, value) in &headers {
        if !is_hop_by_hop_header(name) {
            response = response.header(name, value);
        }
    }

    response
        .body(Body::from_stream(upstream_response.bytes_stream()))
        .map_err(|error| {
            StudioApiError(EnvironmentRuntimeError::Internal {
                code: "environment_proxy_response_failed",
                message: format!("Failed to build the environment response: {error}"),
            })
        })
}

fn environment_upstream_url(
    environment: &StudioEnvironment,
    path: &str,
) -> Result<String, StudioApiError> {
    let path = path.trim_start_matches('/');
    let (endpoint, remaining_path) =
        if let Some(remaining_path) = endpoint_relative_path(path, "api/v2") {
            (
                environment.runtime_endpoints.api_v2.as_deref(),
                remaining_path,
            )
        } else if let Some(remaining_path) = endpoint_relative_path(path, "api/v3") {
            (
                environment.runtime_endpoints.api_v3.as_deref(),
                remaining_path,
            )
        } else if path == "acton_fundAccount"
            && matches!(
                &environment.config,
                EnvironmentConfig::FullTonNetwork { .. }
            )
        {
            (
                environment
                    .runtime_endpoints
                    .api_v2
                    .as_deref()
                    .and_then(|endpoint| endpoint.strip_suffix("/api/v2")),
                path,
            )
        } else {
            (environment.runtime_endpoints.control.as_deref(), path)
        };
    let Some(endpoint) = endpoint else {
        return Err(StudioApiError(EnvironmentRuntimeError::Conflict {
            code: "environment_endpoint_unavailable",
            message: format!("This endpoint is not available in {}", environment.name),
        }));
    };

    Ok(if remaining_path.is_empty() {
        endpoint.to_owned()
    } else {
        format!(
            "{}/{}",
            endpoint.trim_end_matches('/'),
            remaining_path.trim_start_matches('/')
        )
    })
}

fn endpoint_relative_path<'a>(path: &'a str, endpoint: &str) -> Option<&'a str> {
    if path == endpoint {
        return Some("");
    }
    path.strip_prefix(endpoint)
        .and_then(|remaining| remaining.strip_prefix('/'))
}

fn is_hop_by_hop_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn proxy_error(error: reqwest::Error) -> StudioApiError {
    StudioApiError(EnvironmentRuntimeError::Internal {
        code: "environment_proxy_failed",
        message: format!("Failed to reach the virtual environment: {error}"),
    })
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

struct StudioApiError(EnvironmentRuntimeError);

enum WalletApiError {
    Environment(EnvironmentRuntimeError),
    Wallet(WalletRuntimeError),
}

#[derive(Serialize)]
struct StudioApiErrorBody {
    error: StudioApiErrorDetails,
}

#[derive(Serialize)]
struct StudioApiErrorDetails {
    code: &'static str,
    message: String,
}

impl IntoResponse for StudioApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            EnvironmentRuntimeError::InvalidRequest { .. } => StatusCode::BAD_REQUEST,
            EnvironmentRuntimeError::Conflict { .. } => StatusCode::CONFLICT,
            EnvironmentRuntimeError::NotFound { .. } => StatusCode::NOT_FOUND,
            EnvironmentRuntimeError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let code = match &self.0 {
            EnvironmentRuntimeError::InvalidRequest { code, .. }
            | EnvironmentRuntimeError::Conflict { code, .. }
            | EnvironmentRuntimeError::Internal { code, .. } => *code,
            EnvironmentRuntimeError::NotFound { .. } => "environment_not_found",
        };
        (
            status,
            Json(StudioApiErrorBody {
                error: StudioApiErrorDetails {
                    code,
                    message: self.0.to_string(),
                },
            }),
        )
            .into_response()
    }
}

impl IntoResponse for WalletApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Environment(error) => {
                let status = match &error {
                    EnvironmentRuntimeError::InvalidRequest { .. } => StatusCode::BAD_REQUEST,
                    EnvironmentRuntimeError::Conflict { .. } => StatusCode::CONFLICT,
                    EnvironmentRuntimeError::NotFound { .. } => StatusCode::NOT_FOUND,
                    EnvironmentRuntimeError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                };
                let code = match &error {
                    EnvironmentRuntimeError::InvalidRequest { code, .. }
                    | EnvironmentRuntimeError::Conflict { code, .. }
                    | EnvironmentRuntimeError::Internal { code, .. } => *code,
                    EnvironmentRuntimeError::NotFound { .. } => "environment_not_found",
                };
                (status, code, error.to_string())
            }
            Self::Wallet(error) => {
                let (status, code) = match &error {
                    WalletRuntimeError::InvalidRequest { code, .. } => {
                        (StatusCode::BAD_REQUEST, *code)
                    }
                    WalletRuntimeError::NotFound { .. } => {
                        (StatusCode::NOT_FOUND, "wallet_not_found")
                    }
                    WalletRuntimeError::Internal { code, .. } => {
                        (StatusCode::INTERNAL_SERVER_ERROR, *code)
                    }
                };
                (status, code, error.to_string())
            }
        };
        (
            status,
            Json(StudioApiErrorBody {
                error: StudioApiErrorDetails { code, message },
            }),
        )
            .into_response()
    }
}

#[cfg(not(debug_assertions))]
async fn embedded_ui(uri: Uri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');
    let requested_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };

    if let Some(file) = UI_DIR.get_file(requested_path) {
        return ui_file_response(requested_path, file.contents());
    }

    UI_DIR
        .get_file("index.html")
        .map(|index| ui_file_response("index.html", index.contents()))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

#[cfg(not(debug_assertions))]
fn ui_file_response(path: &str, contents: &'static [u8]) -> Response {
    let content_type = match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };

    ([("content-type", content_type)], contents).into_response()
}
