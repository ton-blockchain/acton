//! Serves network discovery, global config, and instance health endpoints.
//!
//! `/` returns the current readiness state and URLs of enabled services.
//! `/openapi.json` returns a generated OpenAPI description of this service.
//! `/localhost.global.config.json` and `/config` return the generated TON global
//! config. `/faucet` gives a new node an on-chain development balance. `/live`
//! and `/healthz` report instance readiness.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, watch};
use tracing::info;
use utoipa::{OpenApi, ToSchema};

use crate::{
    bootstrap::NodeController,
    operations::wallets,
    storage::{RuntimeState, Settings},
    ton::global_config::GlobalConfig,
};

use super::{
    FUND_ACCOUNT_PATH, RunningService,
    error::{ErrorResponse, HttpError},
    server,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "localton Configuration API",
        description = "Network discovery, health, and development funding"
    ),
    paths(
        root_handler,
        localhost_global_config_handler,
        global_config_handler,
        development_faucet_handler,
        live_handler,
        healthz_handler
    ),
    components(schemas(
        ConfigDocument,
        ConfigEndpoints,
        DevelopmentFaucetRequest,
        DevelopmentFaucetResponse,
        ErrorResponse
    )),
    tags((name = "configuration", description = "Local TON network configuration and health"))
)]
struct ApiDoc;

#[derive(Debug, Serialize, ToSchema)]
pub(super) struct ConfigDocument {
    /// Service name
    pub service: String,
    /// `true` when the local network can process requests
    pub ready: bool,
    /// Latest masterchain block number that the instance observed
    pub masterchain_seqno: Option<u32>,
    /// URLs of the services that Localton enabled
    pub endpoints: ConfigEndpoints,
}

#[derive(Debug, Serialize, ToSchema)]
pub(super) struct ConfigEndpoints {
    /// URL of the TON global config for local clients
    pub global_config: String,
    /// Short URL of the same TON global config
    pub config: String,
    /// Readiness probe URL
    pub live: String,
    /// Health probe URL
    pub healthz: String,
    /// Development faucet used to fund a node-owned wallet
    pub faucet: String,
    /// Admin API URL, if the service is enabled
    pub admin: Option<String>,
    /// Account funding URL, if the TON HTTP API is enabled
    pub fund_account: Option<String>,
    /// TON HTTP API v2 URL, if the service is enabled
    pub ton_http_api: Option<String>,
    /// TON HTTP API monitor URL, if the service is enabled
    pub ton_http_api_monitor: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct DevelopmentFaucetRequest {
    /// TON address that receives one development grant
    address: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct DevelopmentFaucetResponse {
    /// TON address that received the grant
    address: String,
    /// Grant amount in nanotons
    amount_nano: u64,
    /// Liteserver send status
    status: u32,
}

#[derive(Clone)]
struct ConfigState {
    control: NodeController,
    settings: Settings,
    faucet_lock: Arc<Mutex<()>>,
}

pub(super) async fn start(
    control: NodeController,
    settings: Settings,
    shutdown: watch::Receiver<bool>,
) -> Result<RunningService> {
    let address = settings.services.config_http.socket_addr();
    let app = Router::new()
        .route("/", get(root_handler))
        .route("/openapi.json", get(openapi_handler))
        .route(
            "/localhost.global.config.json",
            get(localhost_global_config_handler),
        )
        .route("/config", get(global_config_handler))
        .route("/faucet", post(development_faucet_handler))
        .route("/live", get(live_handler))
        .route("/healthz", get(healthz_handler))
        .with_state(ConfigState {
            control,
            settings,
            faucet_lock: Arc::new(Mutex::new(())),
        });
    let endpoint = format!("http://{address}");
    let running = server::start(
        "config HTTP service",
        address,
        app,
        shutdown,
        endpoint.clone(),
    )
    .await?;
    info!(%endpoint, "config HTTP service started");
    Ok(running)
}

async fn openapi_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

pub(super) fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

/// Get network status and service URLs
///
/// Use this endpoint to find the enabled Localton services
#[utoipa::path(
    get,
    path = "/",
    tag = "configuration",
    responses(
        (status = 200, description = "Network state and service endpoints", body = ConfigDocument),
        (status = 400, description = "Runtime state could not be read", body = ErrorResponse)
    )
)]
async fn root_handler(State(state): State<ConfigState>) -> Result<Json<ConfigDocument>, HttpError> {
    let runtime = RuntimeState::load(&state.control.layout().runtime)?;
    Ok(Json(root_document(&state.settings, &runtime)))
}

pub(super) fn root_document(settings: &Settings, runtime: &RuntimeState) -> ConfigDocument {
    let config_endpoint = format!("http://{}", settings.services.config_http.socket_addr());
    let admin_endpoint = settings
        .services
        .admin_http
        .enabled
        .then(|| format!("http://{}", settings.services.admin_http.socket_addr()));
    let fund_account_endpoint = (settings.services.admin_http.enabled
        && settings.services.ton_http_api.enabled)
        .then(|| {
            format!(
                "http://{}{}",
                settings.services.admin_http.socket_addr(),
                FUND_ACCOUNT_PATH
            )
        });
    let ton_http_api_endpoint = settings.services.ton_http_api.enabled.then(|| {
        format!(
            "http://127.0.0.1:{}/api/v2",
            settings.services.ton_http_api.port
        )
    });
    let ton_http_api_monitor = settings.services.ton_http_api.enabled.then(|| {
        format!(
            "http://127.0.0.1:{}",
            settings.services.ton_http_api.monitor_port
        )
    });
    ConfigDocument {
        service: "localton".to_owned(),
        ready: runtime.ready,
        masterchain_seqno: runtime.masterchain_seqno,
        endpoints: ConfigEndpoints {
            global_config: format!("{config_endpoint}/localhost.global.config.json"),
            config: format!("{config_endpoint}/config"),
            live: format!("{config_endpoint}/live"),
            healthz: format!("{config_endpoint}/healthz"),
            faucet: format!("{config_endpoint}/faucet"),
            admin: admin_endpoint,
            fund_account: fund_account_endpoint,
            ton_http_api: ton_http_api_endpoint,
            ton_http_api_monitor,
        },
    }
}

/// Get the TON global config for local clients
///
/// The response contains the liteserver and DHT entries for this network
#[utoipa::path(
    get,
    path = "/localhost.global.config.json",
    tag = "configuration",
    responses(
        (status = 200, description = "Generated TON global configuration", body = GlobalConfig),
        (status = 400, description = "Global configuration could not be read", body = ErrorResponse)
    )
)]
async fn localhost_global_config_handler(
    State(state): State<ConfigState>,
) -> Result<Json<GlobalConfig>, HttpError> {
    read_global_config(&state).await
}

/// Get the TON global config
///
/// This endpoint returns the same document as `/localhost.global.config.json`
#[utoipa::path(
    get,
    path = "/config",
    tag = "configuration",
    responses(
        (status = 200, description = "Generated TON global configuration", body = GlobalConfig),
        (status = 400, description = "Global configuration could not be read", body = ErrorResponse)
    )
)]
async fn global_config_handler(
    State(state): State<ConfigState>,
) -> Result<Json<GlobalConfig>, HttpError> {
    read_global_config(&state).await
}

async fn read_global_config(state: &ConfigState) -> Result<Json<GlobalConfig>, HttpError> {
    let bytes = tokio::fs::read(&state.control.layout().global_config)
        .await
        .context("failed to read global config")?;
    Ok(Json(GlobalConfig::from_json_bytes(&bytes)?))
}

/// Fund a node-owned wallet from the development faucet
///
/// The faucet only transfers test coins. The node keeps its wallet and
/// validator keys locally and submits election messages to Elector itself
#[utoipa::path(
    post,
    path = "/faucet",
    tag = "configuration",
    request_body = DevelopmentFaucetRequest,
    responses(
        (status = 200, description = "Development grant submitted", body = DevelopmentFaucetResponse),
        (status = 400, description = "Grant could not be submitted", body = ErrorResponse)
    )
)]
async fn development_faucet_handler(
    State(state): State<ConfigState>,
    Json(request): Json<DevelopmentFaucetRequest>,
) -> Result<Json<DevelopmentFaucetResponse>, HttpError> {
    let _guard = state.faucet_lock.lock().await;
    let amount_nano = state
        .settings
        .nodes
        .iter()
        .map(|node| node.initial_wallet_amount_nano)
        .max()
        .context("network has no node wallet grant configured")?;
    let amount = wallets::format_nano_grams(u128::from(amount_nano));
    let status = wallets::send(
        &state.control.toolchain(),
        wallets::SendRequest {
            from: "faucet",
            to: &request.address,
            amount: &amount,
            comment: None,
            body: None,
            state_init: None,
            mode: 3,
            bounce: false,
        },
    )
    .await?;
    Ok(Json(DevelopmentFaucetResponse {
        address: request.address,
        amount_nano,
        status,
    }))
}

/// Get the network readiness state
///
/// The endpoint returns `200` only when the instance and the network are ready
#[utoipa::path(
    get,
    path = "/live",
    tag = "configuration",
    responses(
        (status = 200, description = "Instance and network are ready", body = String),
        (status = 503, description = "Instance is still starting", body = String),
        (status = 500, description = "Runtime state could not be read", body = String)
    )
)]
async fn live_handler(State(state): State<ConfigState>) -> Response {
    live_response(&state).await
}

/// Get the network health state
///
/// This endpoint has the same readiness rules as `/live`
#[utoipa::path(
    get,
    path = "/healthz",
    tag = "configuration",
    responses(
        (status = 200, description = "Instance and network are ready", body = String),
        (status = 503, description = "Instance is still starting", body = String),
        (status = 500, description = "Runtime state could not be read", body = String)
    )
)]
async fn healthz_handler(State(state): State<ConfigState>) -> Response {
    live_response(&state).await
}

async fn live_response(state: &ConfigState) -> Response {
    match RuntimeState::load(&state.control.layout().runtime) {
        Ok(runtime) if runtime.ready && runtime.instance_pid.is_some() => {
            (StatusCode::OK, "OK").into_response()
        }
        Ok(_) => (StatusCode::SERVICE_UNAVAILABLE, "STARTING").into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("runtime state error: {error}"),
        )
            .into_response(),
    }
}
