//! Serves the local administrative HTTP API.
//!
//! Read routes return runtime state, settings, wallets, and supervised
//! processes. Node routes start and stop additional validators. When V2 is
//! enabled, `POST /acton_fundAccount` signs a transfer with the genesis wallet,
//! submits it through the V2 backend, and returns the confirmed message hash.
//! `/openapi.json` describes every administrative route and data model.

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{FromRef, Path, State as AxumState},
    middleware,
    routing::{get, post},
};
use tokio::sync::watch;
use tracing::info;
use utoipa::OpenApi;

use crate::{
    bootstrap::LauncherControl,
    operations::wallets,
    runtime::ProcessInfo,
    storage::Settings,
    storage::{NodeRuntime, RuntimeState},
};

use super::{
    FUND_ACCOUNT_PATH, RunningService, cors,
    error::{ErrorResponse, HttpError},
    faucet, server,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "localton Administrative API",
        description = "Runtime state, settings, wallets, processes, node control, and account funding."
    ),
    paths(
        status_handler,
        settings_handler,
        wallets_handler,
        processes_handler,
        start_node_handler,
        stop_node_handler,
        faucet::fund_account_handler
    ),
    components(schemas(
        RuntimeState,
        Settings,
        crate::operations::wallets::PublicWallet,
        ProcessInfo,
        NodeRuntime,
        faucet::FundAccountRequest,
        faucet::FundAccountResponse,
        faucet::FundAccountErrorResponse,
        ErrorResponse
    )),
    tags((name = "administration", description = "Local network administration"))
)]
struct ApiDoc;

#[derive(Clone)]
struct AdminState {
    control: LauncherControl,
    faucet: faucet::State,
}

impl FromRef<AdminState> for faucet::State {
    fn from_ref(state: &AdminState) -> Self {
        state.faucet.clone()
    }
}

pub(super) async fn start(
    control: LauncherControl,
    settings: &Settings,
    shutdown: watch::Receiver<bool>,
) -> Result<RunningService> {
    let address = settings.services.admin_http.socket_addr();
    let backend = format!(
        "http://127.0.0.1:{}",
        settings.services.ton_http_api.backend_port
    );
    let state_dir = control.layout().root.clone();
    let state = AdminState {
        control,
        faucet: faucet::State::new(backend, state_dir),
    };
    let mut app = Router::new()
        .route("/openapi.json", get(openapi_handler))
        .route("/v1/status", get(status_handler))
        .route("/v1/settings", get(settings_handler))
        .route("/v1/wallets", get(wallets_handler))
        .route("/v1/processes", get(processes_handler))
        .route("/v1/nodes/{name}/start", post(start_node_handler))
        .route("/v1/nodes/{name}/stop", post(stop_node_handler));
    if settings.services.ton_http_api.enabled {
        app = app.route(
            FUND_ACCOUNT_PATH,
            post(faucet::fund_account_handler).options(cors::preflight),
        );
    }
    let app = app
        .layer(middleware::from_fn(cors::browser_headers))
        .with_state(state);
    let endpoint = format!("http://{address}");
    let running = server::start(
        "admin HTTP service",
        address,
        app,
        shutdown,
        endpoint.clone(),
    )
    .await?;
    info!(%endpoint, "admin HTTP service started");
    Ok(running)
}

async fn openapi_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[utoipa::path(
    get,
    path = "/v1/status",
    tag = "administration",
    responses(
        (status = 200, description = "Current launcher and network state", body = RuntimeState),
        (status = 400, description = "Runtime state could not be read", body = ErrorResponse)
    )
)]
async fn status_handler(
    AxumState(state): AxumState<AdminState>,
) -> Result<Json<RuntimeState>, HttpError> {
    Ok(Json(RuntimeState::load(&state.control.layout().runtime)?))
}

#[utoipa::path(
    get,
    path = "/v1/settings",
    tag = "administration",
    responses(
        (status = 200, description = "Persistent local network settings", body = Settings),
        (status = 400, description = "Settings could not be read", body = ErrorResponse)
    )
)]
async fn settings_handler(
    AxumState(state): AxumState<AdminState>,
) -> Result<Json<Settings>, HttpError> {
    Ok(Json(Settings::load_or_create(
        &state.control.layout().settings,
    )?))
}

#[utoipa::path(
    get,
    path = "/v1/wallets",
    tag = "administration",
    responses(
        (status = 200, description = "Managed wallets without private keys", body = [crate::operations::wallets::PublicWallet]),
        (status = 400, description = "Wallet registry could not be read", body = ErrorResponse)
    )
)]
async fn wallets_handler(
    AxumState(state): AxumState<AdminState>,
) -> Result<Json<Vec<wallets::PublicWallet>>, HttpError> {
    Ok(Json(wallets::load_public(state.control.layout())?))
}

#[utoipa::path(
    get,
    path = "/v1/processes",
    tag = "administration",
    responses((status = 200, description = "Supervised child processes", body = [ProcessInfo]))
)]
async fn processes_handler(AxumState(state): AxumState<AdminState>) -> Json<Vec<ProcessInfo>> {
    Json(state.control.process_info().await)
}

#[utoipa::path(
    post,
    path = "/v1/nodes/{name}/start",
    tag = "administration",
    params(("name" = String, Path, description = "Configured node name")),
    responses(
        (status = 200, description = "Updated node runtime state", body = NodeRuntime),
        (status = 400, description = "Node could not be started", body = ErrorResponse)
    )
)]
async fn start_node_handler(
    AxumState(state): AxumState<AdminState>,
    Path(name): Path<String>,
) -> Result<Json<NodeRuntime>, HttpError> {
    Ok(Json(state.control.start_node(&name).await?))
}

#[utoipa::path(
    post,
    path = "/v1/nodes/{name}/stop",
    tag = "administration",
    params(("name" = String, Path, description = "Configured node name")),
    responses(
        (status = 200, description = "Updated node runtime state", body = NodeRuntime),
        (status = 400, description = "Node could not be stopped", body = ErrorResponse)
    )
)]
async fn stop_node_handler(
    AxumState(state): AxumState<AdminState>,
    Path(name): Path<String>,
) -> Result<Json<NodeRuntime>, HttpError> {
    Ok(Json(state.control.stop_node(&name).await?))
}
