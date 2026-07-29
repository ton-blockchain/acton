use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex as StdMutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Request, State};
#[cfg(not(debug_assertions))]
use axum::http::Uri;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get, post};
use futures::StreamExt;
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use ton::ton_core::types::TonAddress;
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};

mod contract_facade;
mod contract_registry;
mod contract_source_artifact;
mod environment;
mod environment_catalog;
mod environment_store;
mod full_ton_network;
mod local_artifacts;
mod local_process;
mod local_test_process;
mod test_api;
mod test_run;
mod test_runtime;
mod wallet;

pub use contract_registry::ContractRegistryStore;
pub use contract_source_artifact::{
    CONTRACT_SOURCE_HISTORY_PATH, ContractSourceArtifact, ContractSourceArtifactError,
    ContractSourceArtifactStore,
};
pub use environment::{
    CreateEnvironmentConfig, CreateEnvironmentRequest, EnvironmentCapability, EnvironmentConfig,
    EnvironmentEndpoints, EnvironmentLifecycle, EnvironmentNetwork, EnvironmentRuntime,
    EnvironmentRuntimeError, EnvironmentRuntimeFuture, EnvironmentStatus, PublicTonNetwork,
    StudioEnvironment, UpdateEnvironmentRequest,
};
pub use environment_catalog::{
    MAINNET_ENVIRONMENT_ID, PUBLIC_TON_ENVIRONMENT_IDS, TESTNET_ENVIRONMENT_ID,
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

const MAX_DEPLOYMENT_SUBMISSION_BODY_BYTES: usize = 4 * 1024 * 1024;
const TONCENTER_API_KEY_HEADER: &str = "x-api-key";

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

#[derive(Clone)]
pub struct StudioServerConfig {
    server_version: String,
    workspace: Option<StudioWorkspace>,
    toncenter_api_keys: PublicToncenterApiKeys,
}

impl StudioServerConfig {
    pub fn new(server_version: impl Into<String>) -> Self {
        Self {
            server_version: server_version.into(),
            workspace: None,
            toncenter_api_keys: PublicToncenterApiKeys::from_environment(),
        }
    }

    #[must_use]
    pub fn with_workspace(mut self, workspace: StudioWorkspace) -> Self {
        self.workspace = Some(workspace);
        self
    }

    #[must_use]
    pub fn with_toncenter_api_key(
        mut self,
        network: PublicTonNetwork,
        api_key: impl AsRef<str>,
    ) -> Self {
        self.toncenter_api_keys
            .set(network, sensitive_header_value(api_key.as_ref()));
        self
    }
}

#[derive(Clone, Default)]
struct PublicToncenterApiKeys {
    testnet: Option<HeaderValue>,
    mainnet: Option<HeaderValue>,
}

impl PublicToncenterApiKeys {
    fn from_environment() -> Self {
        let mut keys = Self::default();
        for descriptor in environment_catalog::PUBLIC_TON_NETWORKS {
            let value = std::env::var(descriptor.api_key_environment_variable)
                .ok()
                .and_then(|value| sensitive_header_value(&value));
            keys.set(descriptor.network, value);
        }
        keys
    }

    fn set(&mut self, network: PublicTonNetwork, value: Option<HeaderValue>) {
        match network {
            PublicTonNetwork::Testnet => self.testnet = value,
            PublicTonNetwork::Mainnet => self.mainnet = value,
        }
    }

    const fn get(&self, network: PublicTonNetwork) -> Option<&HeaderValue> {
        match network {
            PublicTonNetwork::Testnet => self.testnet.as_ref(),
            PublicTonNetwork::Mainnet => self.mainnet.as_ref(),
        }
    }

    fn for_environment(&self, environment: &StudioEnvironment) -> Option<&HeaderValue> {
        self.get(public_ton_network(environment)?)
    }
}

#[derive(Clone)]
pub struct StudioServer {
    config: StudioServerConfig,
    contract_registry: ContractRegistryStore,
    environment_runtime: Arc<dyn EnvironmentRuntime>,
    test_run_runtime: Arc<dyn TestRunRuntime>,
    wallet_runtime: Arc<dyn WalletRuntime>,
}

impl StudioServer {
    #[must_use]
    pub fn new(config: StudioServerConfig) -> Self {
        let managed_environment_runtime: Arc<dyn EnvironmentRuntime> =
            Arc::new(environment::EmptyEnvironmentRuntime);
        Self {
            config,
            contract_registry: ContractRegistryStore::ephemeral(),
            environment_runtime: Arc::new(environment_catalog::EnvironmentCatalogRuntime::new(
                managed_environment_runtime,
            )),
            test_run_runtime: Arc::new(test_runtime::EmptyTestRunRuntime::new()),
            wallet_runtime: Arc::new(wallet::EmptyWalletRuntime),
        }
    }

    #[must_use]
    pub fn with_contract_registry(mut self, contract_registry: ContractRegistryStore) -> Self {
        self.contract_registry = contract_registry;
        self
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
        self.environment_runtime = Arc::new(environment_catalog::EnvironmentCatalogRuntime::new(
            Arc::new(environment_runtime),
        ));
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
            contract_registry: self.contract_registry.clone(),
            environment_runtime: Arc::clone(&self.environment_runtime),
            test_run_runtime: Arc::clone(&self.test_run_runtime),
            wallet_runtime: Arc::clone(&self.wallet_runtime),
            http_client: reqwest::Client::new(),
            toncenter_api_keys: self.config.toncenter_api_keys.clone(),
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
    contract_registry: ContractRegistryStore,
    environment_runtime: Arc<dyn EnvironmentRuntime>,
    test_run_runtime: Arc<dyn TestRunRuntime>,
    wallet_runtime: Arc<dyn WalletRuntime>,
    http_client: reqwest::Client,
    toncenter_api_keys: PublicToncenterApiKeys,
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
    let toncenter_api_key = state.toncenter_api_keys.for_environment(&environment);

    if contract_facade::handles(request.method(), &path) {
        return Ok(contract_facade::handle(
            &state.contract_registry,
            &state.http_client,
            &environment,
            toncenter_api_key,
            &path,
            request,
        )
        .await);
    }

    let submission_kind = deployment_submission_kind(request.method(), &path);
    let (parts, body) = request.into_parts();
    let (body, captured_submission_body) = if submission_kind.is_some() {
        let (body, capture) = capture_request_body(body);
        (body, Some(capture))
    } else {
        (reqwest::Body::wrap_stream(body.into_data_stream()), None)
    };
    let mut upstream_url = environment_upstream_url(&environment, &path)?;
    if let Some(query) = parts.uri.query() {
        upstream_url.push('?');
        upstream_url.push_str(query);
    }

    let is_public_toncenter = public_ton_network(&environment).is_some();
    let upstream_request = apply_upstream_headers(
        state.http_client.request(parts.method, upstream_url),
        &parts.headers,
        &environment,
        toncenter_api_key,
    );

    let upstream_response = upstream_request
        .body(body)
        .send()
        .await
        .map_err(proxy_error)?;
    let status = upstream_response.status();
    let headers = upstream_response.headers().clone();
    let mut response = Response::builder().status(status);
    for (name, value) in &headers {
        if should_forward_upstream_response_header(name, is_public_toncenter) {
            response = response.header(name, value);
        }
    }

    let response_body =
        if let (Some(kind), Some(capture)) = (submission_kind, captured_submission_body) {
            let bytes = upstream_response.bytes().await.map_err(proxy_error)?;
            let request_body = capture
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .body();
            if deployment_submission_accepted(status, &bytes)
                && let Some(request_body) = request_body
            {
                record_deployment_submission(
                    &state.contract_registry,
                    &environment.id,
                    kind,
                    parts.uri.query(),
                    &request_body,
                )
                .await;
            }
            Body::from(bytes)
        } else {
            Body::from_stream(upstream_response.bytes_stream())
        };
    response.body(response_body).map_err(|error| {
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

fn public_ton_network(environment: &StudioEnvironment) -> Option<PublicTonNetwork> {
    if environment.lifecycle != EnvironmentLifecycle::External {
        return None;
    }
    match &environment.config {
        EnvironmentConfig::RemoteTonNetwork { network } => Some(*network),
        EnvironmentConfig::ActonLocalnet { .. } | EnvironmentConfig::FullTonNetwork { .. } => None,
    }
}

fn is_safe_external_request_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "accept"
            | "accept-encoding"
            | "cache-control"
            | "content-encoding"
            | "content-length"
            | "content-type"
            | "if-match"
            | "if-modified-since"
            | "if-none-match"
            | "if-range"
            | "if-unmodified-since"
            | "pragma"
            | "range"
            | "user-agent"
    )
}

fn should_forward_upstream_request_header(name: &HeaderName, is_public_toncenter: bool) -> bool {
    if is_public_toncenter {
        return is_safe_external_request_header(name);
    }
    !is_hop_by_hop_header(name) && name != axum::http::header::HOST
}

fn should_forward_upstream_response_header(name: &HeaderName, is_public_toncenter: bool) -> bool {
    !(is_hop_by_hop_header(name)
        || is_public_toncenter && matches!(name.as_str(), TONCENTER_API_KEY_HEADER | "set-cookie"))
}

pub(crate) fn apply_environment_upstream_auth(
    mut request: reqwest::RequestBuilder,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
) -> reqwest::RequestBuilder {
    if public_ton_network(environment).is_some()
        && let Some(api_key) = toncenter_api_key
    {
        request = request.header(TONCENTER_API_KEY_HEADER, api_key);
    }
    request
}

fn apply_upstream_headers(
    mut request: reqwest::RequestBuilder,
    headers: &HeaderMap,
    environment: &StudioEnvironment,
    toncenter_api_key: Option<&HeaderValue>,
) -> reqwest::RequestBuilder {
    let is_public_toncenter = public_ton_network(environment).is_some();
    for (name, value) in headers {
        if should_forward_upstream_request_header(name, is_public_toncenter) {
            request = request.header(name, value);
        }
    }
    apply_environment_upstream_auth(request, environment, toncenter_api_key)
}

fn sensitive_header_value(value: &str) -> Option<HeaderValue> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mut value = HeaderValue::from_str(value).ok()?;
    value.set_sensitive(true);
    Some(value)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeploymentSubmissionKind {
    Direct,
    JsonRpc,
}

fn deployment_submission_kind(
    method: &axum::http::Method,
    path: &str,
) -> Option<DeploymentSubmissionKind> {
    if method != axum::http::Method::POST {
        return None;
    }
    match path.trim_matches('/') {
        "api/v2/sendBoc"
        | "api/v2/sendBocReturnHash"
        | "api/v3/message"
        | "acton_sendInternalMessage" => Some(DeploymentSubmissionKind::Direct),
        "api/v2" | "api/v2/jsonRPC" | "api/v2/v2/jsonRPC" => {
            Some(DeploymentSubmissionKind::JsonRpc)
        }
        _ => None,
    }
}

#[derive(Debug, Default)]
struct CapturedSubmissionBody {
    bytes: Vec<u8>,
    overflowed: bool,
}

impl CapturedSubmissionBody {
    fn extend(&mut self, bytes: &[u8]) {
        if self.overflowed {
            return;
        }
        if bytes.len() > MAX_DEPLOYMENT_SUBMISSION_BODY_BYTES.saturating_sub(self.bytes.len()) {
            self.bytes.clear();
            self.overflowed = true;
            return;
        }
        self.bytes.extend_from_slice(bytes);
    }

    fn body(&self) -> Option<Vec<u8>> {
        (!self.overflowed).then(|| self.bytes.clone())
    }
}

type SharedSubmissionBody = Arc<StdMutex<CapturedSubmissionBody>>;

fn capture_request_body(body: Body) -> (reqwest::Body, SharedSubmissionBody) {
    let capture = Arc::new(StdMutex::new(CapturedSubmissionBody::default()));
    let stream_capture = Arc::clone(&capture);
    let stream = body.into_data_stream().map(move |chunk| {
        if let Ok(bytes) = &chunk {
            stream_capture
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend(bytes);
        }
        chunk
    });
    (reqwest::Body::wrap_stream(stream), capture)
}

fn deployment_submission_accepted(status: StatusCode, body: &[u8]) -> bool {
    if !status.is_success() {
        return false;
    }
    let Ok(serde_json::Value::Object(response)) = serde_json::from_slice::<serde_json::Value>(body)
    else {
        return false;
    };
    !matches!(response.get("ok"), Some(serde_json::Value::Bool(false)))
        && response.get("error").is_none_or(serde_json::Value::is_null)
}

fn deployment_submission_boc(
    kind: DeploymentSubmissionKind,
    query: Option<&str>,
    body: &[u8],
) -> Option<String> {
    match kind {
        DeploymentSubmissionKind::Direct => json_boc(body)
            .or_else(|| form_boc(body))
            .or_else(|| query.and_then(|query| form_boc(query.as_bytes()))),
        DeploymentSubmissionKind::JsonRpc => {
            let value = serde_json::from_slice::<serde_json::Value>(body).ok()?;
            let method = value.get("method")?.as_str()?;
            if !matches!(method, "sendBoc" | "sendBocReturnHash") {
                return None;
            }
            value.get("params")?.get("boc")?.as_str().map(str::to_owned)
        }
    }
}

fn json_boc(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()?
        .get("boc")?
        .as_str()
        .map(str::to_owned)
}

fn form_boc(encoded: &[u8]) -> Option<String> {
    url::form_urlencoded::parse(encoded)
        .find_map(|(name, value)| (name == "boc").then(|| value.into_owned()))
}

async fn record_deployment_submission(
    store: &ContractRegistryStore,
    environment_id: &str,
    kind: DeploymentSubmissionKind,
    query: Option<&str>,
    body: &[u8],
) {
    let Some(boc) = deployment_submission_boc(kind, query, body) else {
        return;
    };
    let candidates = match ton_api::extract_deployment_candidates(&boc) {
        Ok(candidates) => candidates,
        Err(error) => {
            tracing::debug!(
                environment_id,
                error = %error,
                "Ignored an invalid deployment submission observed through Studio RPC"
            );
            return;
        }
    };
    let candidates = candidates
        .into_iter()
        .filter_map(|candidate| {
            let address = TonAddress::from_str(&candidate.address).ok()?;
            Some(contract_registry::DeploymentCandidateRegistration {
                canonical_address: address.to_hex(),
                display_address: address.to_base64(false, true, true),
                code_hash: candidate.code_hash,
            })
        })
        .collect();
    if let Err(error) = store
        .record_deployment_candidates(environment_id, candidates)
        .await
    {
        tracing::warn!(
            environment_id,
            error = %error,
            "Failed to persist deployment candidates observed through Studio RPC"
        );
    }
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

#[cfg(test)]
mod proxy_header_tests {
    use std::fmt::Write as _;

    use axum::http::header::{CONTENT_TYPE, HOST, SET_COOKIE};
    use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
    use expect_test::expect;

    use super::{
        CapturedSubmissionBody, DeploymentSubmissionKind, MAX_DEPLOYMENT_SUBMISSION_BODY_BYTES,
        PublicToncenterApiKeys, TONCENTER_API_KEY_HEADER, apply_environment_upstream_auth,
        apply_upstream_headers, deployment_submission_accepted, deployment_submission_boc,
        sensitive_header_value, should_forward_upstream_response_header,
    };
    use crate::{
        EnvironmentConfig, EnvironmentEndpoints, EnvironmentStatus, PublicTonNetwork,
        StudioEnvironment,
    };

    #[test]
    fn public_toncenter_only_forwards_safe_request_headers_and_uses_server_api_key() {
        let mut incoming = HeaderMap::new();
        incoming.insert(HOST, HeaderValue::from_static("studio.local"));
        incoming.insert(
            TONCENTER_API_KEY_HEADER,
            HeaderValue::from_static("client-key"),
        );
        incoming.insert(
            "authorization",
            HeaderValue::from_static("Bearer browser-session"),
        );
        incoming.insert("cookie", HeaderValue::from_static("studio=session"));
        incoming.insert("forwarded", HeaderValue::from_static("for=192.0.2.10"));
        incoming.insert("x-forwarded-for", HeaderValue::from_static("192.0.2.10"));
        incoming.insert(
            "x-forwarded-host",
            HeaderValue::from_static("studio.example"),
        );
        incoming.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        incoming.insert("origin", HeaderValue::from_static("https://studio.example"));
        incoming.insert(
            "referer",
            HeaderValue::from_static("https://studio.example/testnet"),
        );
        incoming.insert(
            "cf-access-jwt-assertion",
            HeaderValue::from_static("cloudflare-access-token"),
        );
        incoming.insert("x-test-marker", HeaderValue::from_static("forwarded"));
        incoming.insert("accept", HeaderValue::from_static("application/json"));
        incoming.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        incoming.insert("content-length", HeaderValue::from_static("12"));
        incoming.insert("content-encoding", HeaderValue::from_static("gzip"));
        incoming.insert("user-agent", HeaderValue::from_static("Acton Studio"));
        incoming.insert("if-none-match", HeaderValue::from_static("\"revision\""));
        let server_key = sensitive_header_value("server-key").expect("server key must be valid");
        let client = reqwest::Client::new();
        let testnet = remote_environment(PublicTonNetwork::Testnet, true);
        let mainnet = remote_environment(PublicTonNetwork::Mainnet, true);
        let managed = remote_environment(PublicTonNetwork::Testnet, false);

        let testnet_request = apply_upstream_headers(
            client.get("https://testnet.toncenter.com/api/v2"),
            &incoming,
            &testnet,
            Some(&server_key),
        )
        .build()
        .expect("testnet request must build");
        let mainnet_request = apply_upstream_headers(
            client.get("https://toncenter.com/api/v2"),
            &incoming,
            &mainnet,
            Some(&server_key),
        )
        .build()
        .expect("mainnet request must build");
        let managed_request = apply_upstream_headers(
            client.get("http://127.0.0.1:5411/api/v2"),
            &incoming,
            &managed,
            Some(&server_key),
        )
        .build()
        .expect("managed request must build");

        assert_eq!(testnet_request.headers(), mainnet_request.headers());
        let actual = request_headers_snapshot(&testnet_request, &managed_request);

        expect![[r#"PUBLIC TONCENTER
api key: server-key
api key sensitive: true
host: <missing>
authorization: <missing>
cookie: <missing>
forwarded: <missing>
x-forwarded-for: <missing>
x-forwarded-host: <missing>
x-forwarded-proto: <missing>
origin: <missing>
referer: <missing>
cf-access-jwt-assertion: <missing>
marker: <missing>
accept: application/json
content-type: application/json
content-length: 12
content-encoding: gzip
user-agent: Acton Studio
if-none-match: "revision"

MANAGED
api key: client-key
host: <missing>
authorization: Bearer browser-session
cookie: studio=session
forwarded: for=192.0.2.10
x-forwarded-for: 192.0.2.10
x-forwarded-host: studio.example
x-forwarded-proto: https
origin: https://studio.example
referer: https://studio.example/testnet
cf-access-jwt-assertion: cloudflare-access-token
marker: forwarded
accept: application/json
content-type: application/json
content-length: 12
content-encoding: gzip
user-agent: Acton Studio
if-none-match: "revision""#]]
        .assert_eq(&actual);
    }

    #[test]
    fn direct_public_toncenter_requests_use_their_network_api_key() {
        let mut api_keys = PublicToncenterApiKeys::default();
        api_keys.set(
            PublicTonNetwork::Testnet,
            sensitive_header_value("testnet-key"),
        );
        api_keys.set(
            PublicTonNetwork::Mainnet,
            sensitive_header_value("mainnet-key"),
        );
        let client = reqwest::Client::new();
        let testnet = remote_environment(PublicTonNetwork::Testnet, true);
        let mainnet = remote_environment(PublicTonNetwork::Mainnet, true);
        let managed = remote_environment(PublicTonNetwork::Testnet, false);
        let testnet_request = apply_environment_upstream_auth(
            client.get("https://testnet.toncenter.com/api/v3/accountStates"),
            &testnet,
            api_keys.for_environment(&testnet),
        )
        .build()
        .expect("testnet request must build");
        let mainnet_request = apply_environment_upstream_auth(
            client.get("https://toncenter.com/api/v3/accountStates"),
            &mainnet,
            api_keys.for_environment(&mainnet),
        )
        .build()
        .expect("mainnet request must build");
        let managed_request = apply_environment_upstream_auth(
            client.get("http://127.0.0.1:5411/api/v3/accountStates"),
            &managed,
            api_keys.get(PublicTonNetwork::Testnet),
        )
        .build()
        .expect("managed request must build");
        let actual = format!(
            "testnet api key: {}\ntestnet api key sensitive: {}\nmainnet api key: {}\nmainnet api key sensitive: {}\nmanaged api key: {}",
            header(&testnet_request, TONCENTER_API_KEY_HEADER),
            testnet_request
                .headers()
                .get(TONCENTER_API_KEY_HEADER)
                .is_some_and(HeaderValue::is_sensitive),
            header(&mainnet_request, TONCENTER_API_KEY_HEADER),
            mainnet_request
                .headers()
                .get(TONCENTER_API_KEY_HEADER)
                .is_some_and(HeaderValue::is_sensitive),
            header(&managed_request, TONCENTER_API_KEY_HEADER),
        );

        expect![[r"testnet api key: testnet-key
testnet api key sensitive: true
mainnet api key: mainnet-key
mainnet api key sensitive: true
managed api key: <missing>"]]
        .assert_eq(&actual);
    }

    #[test]
    fn public_toncenter_response_cannot_set_credentials() {
        let actual = format!(
            "PUBLIC TONCENTER\ncontent-type: {}\nset-cookie: {}\nx-api-key: {}\n\nMANAGED\ncontent-type: {}\nset-cookie: {}\nx-api-key: {}",
            should_forward_upstream_response_header(&CONTENT_TYPE, true),
            should_forward_upstream_response_header(&SET_COOKIE, true),
            should_forward_upstream_response_header(
                &HeaderName::from_static(TONCENTER_API_KEY_HEADER),
                true,
            ),
            should_forward_upstream_response_header(&CONTENT_TYPE, false),
            should_forward_upstream_response_header(&SET_COOKIE, false),
            should_forward_upstream_response_header(
                &HeaderName::from_static(TONCENTER_API_KEY_HEADER),
                false,
            ),
        );

        expect![[r"PUBLIC TONCENTER
content-type: true
set-cookie: false
x-api-key: false

MANAGED
content-type: true
set-cookie: true
x-api-key: true"]]
        .assert_eq(&actual);
    }

    #[test]
    fn deployment_acceptance_requires_a_successful_json_envelope() {
        let actual = [
            (
                "success",
                StatusCode::OK,
                br#"{"ok":true,"result":{}}"#.as_slice(),
            ),
            ("ok false", StatusCode::OK, br#"{"ok":false}"#.as_slice()),
            (
                "json-rpc error",
                StatusCode::OK,
                br#"{"error":{"code":-32000}}"#.as_slice(),
            ),
            (
                "http error",
                StatusCode::BAD_REQUEST,
                br#"{"ok":true}"#.as_slice(),
            ),
            ("not json", StatusCode::OK, b"accepted".as_slice()),
        ]
        .map(|(label, status, body)| {
            format!("{label}: {}", deployment_submission_accepted(status, body))
        })
        .join("\n");

        expect![[r"success: true
ok false: false
json-rpc error: false
http error: false
not json: false"]]
        .assert_eq(&actual);
    }

    #[test]
    fn deployment_capture_discards_oversized_body_and_json_rpc_reads() {
        let mut capture = CapturedSubmissionBody {
            bytes: vec![0; MAX_DEPLOYMENT_SUBMISSION_BODY_BYTES - 1],
            overflowed: false,
        };
        capture.extend(&[1, 2]);
        assert!(capture.body().is_none());
        assert!(capture.bytes.is_empty());

        let read_body = br#"{
            "method":"runGetMethod",
            "params":{"boc":"not-a-deployment"}
        }"#;
        assert_eq!(
            deployment_submission_boc(DeploymentSubmissionKind::JsonRpc, None, read_body),
            None
        );
    }

    fn remote_environment(network: PublicTonNetwork, external: bool) -> StudioEnvironment {
        let descriptor = crate::environment_catalog::PUBLIC_TON_NETWORKS
            .iter()
            .find(|descriptor| descriptor.network == network)
            .expect("public TON network descriptor");
        let config = EnvironmentConfig::RemoteTonNetwork { network };
        if external {
            StudioEnvironment::new_external(
                descriptor.environment_id,
                descriptor.display_name,
                EnvironmentStatus::Running,
                config,
                EnvironmentEndpoints::default(),
            )
        } else {
            StudioEnvironment::new(
                "managed",
                "Managed",
                EnvironmentStatus::Running,
                config,
                EnvironmentEndpoints::default(),
            )
        }
    }

    fn request_headers_snapshot(public: &reqwest::Request, managed: &reqwest::Request) -> String {
        const HEADERS: &[(&str, &str)] = &[
            ("api key", TONCENTER_API_KEY_HEADER),
            ("host", "host"),
            ("authorization", "authorization"),
            ("cookie", "cookie"),
            ("forwarded", "forwarded"),
            ("x-forwarded-for", "x-forwarded-for"),
            ("x-forwarded-host", "x-forwarded-host"),
            ("x-forwarded-proto", "x-forwarded-proto"),
            ("origin", "origin"),
            ("referer", "referer"),
            ("cf-access-jwt-assertion", "cf-access-jwt-assertion"),
            ("marker", "x-test-marker"),
            ("accept", "accept"),
            ("content-type", "content-type"),
            ("content-length", "content-length"),
            ("content-encoding", "content-encoding"),
            ("user-agent", "user-agent"),
            ("if-none-match", "if-none-match"),
        ];
        let mut output = format!(
            "PUBLIC TONCENTER\napi key: {}\napi key sensitive: {}",
            header(public, TONCENTER_API_KEY_HEADER),
            public
                .headers()
                .get(TONCENTER_API_KEY_HEADER)
                .is_some_and(HeaderValue::is_sensitive),
        );
        for (label, name) in &HEADERS[1..] {
            let _ = write!(output, "\n{label}: {}", header(public, name));
        }
        output.push_str("\n\nMANAGED");
        for (label, name) in HEADERS {
            let _ = write!(output, "\n{label}: {}", header(managed, name));
        }
        output
    }

    fn header<'a>(request: &'a reqwest::Request, name: &str) -> &'a str {
        request
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("<missing>")
    }
}
