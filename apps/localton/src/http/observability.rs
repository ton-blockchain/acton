//! Public network dashboard and collector for signed host telemetry.

#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::{
    net::Ipv4Addr,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::{get, options, post},
};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use tokio::{sync::RwLock, sync::watch, task::JoinHandle, time::MissedTickBehavior};
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};
use utoipa::OpenApi;

#[cfg(not(debug_assertions))]
use axum::http::Uri;

use crate::{
    observability::{
        NetworkView, NodeCapability, NodeTelemetry, ObservationStore, ObserverIdentity,
        SignedObservation, VerifiedNetworkState, network_id,
    },
    storage::{Layout, NodeRole, RuntimeState, Settings, unix_time},
    ton::toolchain::Toolchain,
};

use super::{
    RunningService, cors,
    error::{ErrorResponse, HttpError},
    server,
};

mod geoip;
mod network;

use geoip::GeoIpResolver;
use network::NodeHeadSample;

const MAX_TELEMETRY_BODY_BYTES: usize = 256 * 1024;

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../packages/localton-ui/dist/.embedded");

type SharedStore = Arc<RwLock<ObservationStore>>;
type SharedGeoIp = Arc<RwLock<Option<GeoIpResolver>>>;

#[derive(Clone)]
struct ObservabilityState {
    store: SharedStore,
    network: watch::Receiver<Option<VerifiedNetworkState>>,
    local_node_is_network_source: bool,
    geoip: SharedGeoIp,
}

/// HTTP listener and background tasks that share one observation shutdown signal.
pub(super) struct RunningObservability {
    pub service: RunningService,
    pub tasks: Vec<JoinHandle<()>>,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "localton Observability API",
        description = "Signed host telemetry combined with TON state read through a local liteserver"
    ),
    paths(network_handler, local_observation_handler, collect_handler, health_handler),
    components(schemas(
        NetworkView,
        SignedObservation,
        ErrorResponse
    )),
    tags((name = "observability", description = "TON network state and Localton host telemetry"))
)]
struct ApiDoc;

/// Starts one observer after durable node identity and runtime state are available.
///
/// The first signed heartbeat is published before the listener becomes visible.
/// Network reading and publication remain separate tasks so slow TON queries cannot
/// make a healthy Localton process disappear from the collector.
pub(super) async fn start(
    layout: Layout,
    toolchain: Toolchain,
    settings: &Settings,
    advertised_ip: Ipv4Addr,
    collector: Option<String>,
    shutdown: watch::Receiver<bool>,
) -> Result<RunningObservability> {
    let observability = &settings.services.observability;
    let address = observability.socket_addr();
    let endpoint = format!("http://{advertised_ip}:{}", observability.port);
    let collector = collector.map(CollectorClient::new);

    let identity = ObserverIdentity::load_or_create(&layout.observability.join("identity.json"))?;
    let store = Arc::new(RwLock::new(ObservationStore::new(
        network_id(&layout.global_config)?,
        identity,
        observability.block_window_seconds,
    )));

    publish_runtime_observation(
        &layout,
        observability.observation_ttl_seconds,
        None,
        &endpoint,
        &store,
    )
    .await?;

    let (network_updates, network_snapshot) = watch::channel(None);
    let (node_updates, node_snapshot) = watch::channel(None);
    let local_node_is_network_source = settings.node.role == NodeRole::Genesis;
    let geoip = Arc::new(RwLock::new(None));

    let state = ObservabilityState {
        store: Arc::clone(&store),
        network: network_snapshot,
        local_node_is_network_source,
        geoip: Arc::clone(&geoip),
    };

    let api = Router::new()
        .route("/openapi.json", get(openapi_handler))
        .route("/network", get(network_handler))
        .route("/observation", get(local_observation_handler))
        .route("/observations", post(collect_handler))
        .route("/{*path}", options(cors::preflight));

    let app = Router::new()
        .nest("/api/v1", api)
        .route("/healthz", get(health_handler))
        .layer(DefaultBodyLimit::max(MAX_TELEMETRY_BODY_BYTES))
        .layer(middleware::from_fn(cors::browser_headers))
        .with_state(state);
    #[cfg(debug_assertions)]
    let app = {
        let dist_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/localton-ui/dist"
        ));
        app.fallback_service(
            ServeDir::new(&dist_path).fallback(ServeFile::new(dist_path.join("index.html"))),
        )
    };
    #[cfg(not(debug_assertions))]
    let app = app.fallback(embedded_ui);

    let service = server::start(
        "observability HTTP service",
        address,
        app,
        shutdown.clone(),
        endpoint.clone(),
    )
    .await?;

    let publisher = tokio::spawn(publication_loop(
        layout.clone(),
        settings.clone(),
        endpoint.clone(),
        Arc::clone(&store),
        collector,
        node_snapshot,
        shutdown.clone(),
    ));
    let network_reader = tokio::spawn(network::collection_loop(
        toolchain,
        settings.services.observability.publish_interval_seconds,
        settings.services.observability.block_window_seconds,
        network_updates,
        local_node_is_network_source.then(|| node_updates.clone()),
        shutdown.clone(),
    ));
    let geoip_loader = tokio::spawn(async move {
        if let Ok(resolver) = GeoIpResolver::load().await {
            *geoip.write().await = Some(resolver);
        }
    });
    let mut tasks = vec![publisher, network_reader, geoip_loader];
    if !local_node_is_network_source {
        tasks.push(tokio::spawn(network::node_collection_loop(
            layout,
            settings.services.observability.publish_interval_seconds,
            node_updates,
            shutdown,
        )));
    }

    info!(%endpoint, "observability API and UI started");

    Ok(RunningObservability { service, tasks })
}

async fn openapi_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(
    get,
    path = "/api/v1/network",
    tag = "observability",
    responses((status = 200, description = "Aggregated network view", body = NetworkView))
)]
async fn network_handler(State(state): State<ObservabilityState>) -> Json<NetworkView> {
    let network = state.network.borrow().clone();
    let mut view = state.store.write().await.aggregate(
        unix_time(),
        network.as_ref(),
        state.local_node_is_network_source,
    );

    let geoip = state.geoip.read().await;
    for node in &mut view.nodes {
        node.location = geoip.as_ref().map_or_else(
            || geoip::location_without_database(&node.telemetry.public_ip),
            |geoip| geoip.locate(&node.telemetry.public_ip),
        );
    }

    Json(view)
}

#[utoipa::path(
    get,
    path = "/api/v1/observation",
    tag = "observability",
    responses(
        (status = 200, description = "This observer's signed report", body = SignedObservation),
        (status = 503, description = "The first report is not ready", body = String)
    )
)]
async fn local_observation_handler(State(state): State<ObservabilityState>) -> Response {
    match state.store.read().await.local() {
        Some(observation) => Json(observation).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "COLLECTING").into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/observations",
    tag = "observability",
    request_body = SignedObservation,
    responses(
        (status = 204, description = "Signed host telemetry accepted"),
        (status = 400, description = "Signed host telemetry was invalid", body = ErrorResponse)
    )
)]
async fn collect_handler(
    State(state): State<ObservabilityState>,
    Json(observation): Json<SignedObservation>,
) -> Result<StatusCode, HttpError> {
    state.store.write().await.ingest(observation, unix_time())?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/healthz",
    tag = "observability",
    responses(
        (status = 200, description = "A current local observation is available", body = String),
        (status = 503, description = "The local observation is missing or stale", body = String)
    )
)]
async fn health_handler(State(state): State<ObservabilityState>) -> Response {
    let now = unix_time();
    match state.store.read().await.local() {
        Some(observation) if observation.expires_at > now => (StatusCode::OK, "OK").into_response(),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "STALE").into_response(),
    }
}

/// Publishes signed process heartbeats and optionally forwards them to one collector.
///
/// The latest host-local head is read from a watch snapshot, so neither the local
/// heartbeat nor collector delivery waits for an in-flight TON network query.
async fn publication_loop(
    layout: Layout,
    settings: Settings,
    endpoint: String,
    store: SharedStore,
    collector: Option<CollectorClient>,
    mut node_head: watch::Receiver<Option<NodeHeadSample>>,
    mut shutdown: watch::Receiver<bool>,
) {
    let interval_seconds = settings.services.observability.publish_interval_seconds;
    let ttl_seconds = settings.services.observability.observation_ttl_seconds;
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut collector_available = None;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            changed = node_head.changed() => {
                if changed.is_err() {
                    break;
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
        }

        let node_head = *node_head.borrow();
        match publish_runtime_observation(&layout, ttl_seconds, node_head, &endpoint, &store).await
        {
            Ok(observation) => {
                if let Some(collector) = &collector {
                    let started_at = Instant::now();
                    match collector.publish(&observation).await {
                        Ok(()) => {
                            if collector_available != Some(true) {
                                info!(
                                    operation = "observability_publish",
                                    target = collector.url,
                                    duration_ms = started_at.elapsed().as_millis(),
                                    outcome = "connected",
                                    "host telemetry collector is available"
                                );
                            }
                            collector_available = Some(true);
                        }
                        Err(error) => {
                            if collector_available != Some(false) {
                                warn!(
                                    operation = "observability_publish",
                                    target = collector.url,
                                    duration_ms = started_at.elapsed().as_millis(),
                                    outcome = "unavailable",
                                    error = %format_args!("{error:#}"),
                                    "host telemetry collector is unavailable"
                                );
                            }
                            collector_available = Some(false);
                        }
                    }
                }
            }
            Err(error) => warn!(%error, "host telemetry publication failed"),
        }
    }
}

struct CollectorClient {
    url: String,
    client: reqwest::Client,
}

impl CollectorClient {
    fn new(endpoint: String) -> Self {
        let url = format!("{}/api/v1/observations", endpoint.trim_end_matches('/'));

        Self {
            url,
            client: reqwest::Client::new(),
        }
    }

    async fn publish(&self, observation: &SignedObservation) -> Result<()> {
        self.client
            .post(&self.url)
            .timeout(Duration::from_secs(5))
            .json(observation)
            .send()
            .await
            .with_context(|| format!("failed to request {}", self.url))?
            .error_for_status()
            .with_context(|| format!("collector rejected {}", self.url))?;

        Ok(())
    }
}

/// Signs host-local state without including network-wide derived facts.
///
/// The node head belongs to this host and is safe to report. The network head,
/// validator membership, elections, and production remain owned by the reader.
async fn publish_runtime_observation(
    layout: &Layout,
    ttl_seconds: u64,
    node_head: Option<NodeHeadSample>,
    endpoint: &str,
    store: &SharedStore,
) -> Result<SignedObservation> {
    let now = unix_time();
    let settings = Settings::load(&layout.settings)?;
    let runtime = RuntimeState::load(&layout.runtime)?;
    let node = settings.node;
    let node_runtime = runtime.node;
    let head_seqno = node_head
        .map(|sample| sample.seqno)
        .or(node_runtime.head_seqno);
    let head_observed_at = node_head.map(|sample| sample.observed_at);
    let mut roles = vec![NodeCapability::FullNode];
    if node.validator {
        roles.push(NodeCapability::Validator);
    }
    if node.liteserver {
        roles.push(NodeCapability::Liteserver);
    }
    let telemetry = NodeTelemetry {
        software: format!("localton/{}", env!("CARGO_PKG_VERSION")),
        observability_endpoint: endpoint.to_owned(),
        instance_started_at: runtime.started_at,
        name: node.name,
        public_ip: node.public_ip.to_string(),
        roles,
        running: node_runtime.running,
        process_id: node_runtime.pid,
        status: node_runtime.status,
        last_error: node_runtime.last_error,
        head_seqno,
        head_observed_at,
        sync_initial_masterchain_block_time: node_runtime.sync_initial_masterchain_block_time,
        sync_masterchain_block_time: node_runtime.sync_masterchain_block_time,
        sync_target_time: node_runtime.sync_target_time,
        initial_sync_progress: node_runtime.initial_sync_progress,
        sync_progressed_at: node_head
            .map(|sample| sample.progressed_at)
            .or(node_runtime.sync_progressed_at),
        participate_in_elections: node.participate_in_elections,
        validator_public_key: node_runtime.validator_public_key.map(|key| key.to_hex()),
        validator_public_keys: node_runtime
            .validator_public_keys
            .into_iter()
            .map(|key| key.to_hex())
            .collect(),
        validator_adnl: node_runtime.validator_adnl.map(|key| key.to_hex()),
    };
    let observation = store.write().await.publish(telemetry, now, ttl_seconds)?;
    Ok(observation)
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
    (
        [
            ("content-type", content_type),
            ("content-encoding", "gzip"),
            ("vary", "Accept-Encoding"),
        ],
        contents,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_exposes_collection_and_network_view() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        assert!(document["paths"]["/api/v1/network"]["get"].is_object());
        assert!(document["paths"]["/api/v1/observations"]["post"].is_object());
        assert!(document["components"]["schemas"]["NetworkView"].is_object());
    }

    #[tokio::test]
    async fn heartbeat_keeps_publishing_without_chain_updates() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let layout = Layout::new(directory.path().join("observer"));
        layout.create_dirs().unwrap();
        let mut settings = Settings::default();
        settings.services.observability.publish_interval_seconds = 1;
        settings.save_atomic(&layout.settings).unwrap();
        let mut runtime = RuntimeState::new();
        runtime.node = crate::storage::NodeRuntime {
            initialized: true,
            running: true,
            status: "synchronizing".to_owned(),
            head_seqno: Some(40),
            network_head_seqno: Some(100),
            sync_initial_masterchain_block_time: Some(1_000),
            sync_masterchain_block_time: Some(1_020),
            sync_target_time: Some(1_040),
            sync_progressed_at: Some(50),
            ..crate::storage::NodeRuntime::default()
        };
        runtime.save_atomic(&layout.runtime).unwrap();

        let identity = ObserverIdentity::load_or_create(
            &layout.observability.join("heartbeat-test-identity.json"),
        )
        .unwrap();
        let store = Arc::new(RwLock::new(ObservationStore::new(
            "network".to_owned(),
            identity,
            60,
        )));
        let (_node_updates, node_head) = watch::channel(None);
        let (shutdown, shutdown_receiver) = watch::channel(false);
        let task = tokio::spawn(publication_loop(
            layout,
            settings,
            "http://127.0.0.1:18007".to_owned(),
            Arc::clone(&store),
            None,
            node_head,
            shutdown_receiver,
        ));

        let first_sequence = loop {
            if let Some(observation) = store.read().await.local() {
                break observation.sequence;
            }
            tokio::task::yield_now().await;
        };
        let second_sequence = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                tokio::time::sleep(Duration::from_millis(20)).await;
                if let Some(observation) = store.read().await.local()
                    && observation.sequence > first_sequence
                {
                    break observation.sequence;
                }
            }
        })
        .await
        .unwrap();

        assert!(second_sequence > first_sequence);
        let observation = store.read().await.local().unwrap();
        expect_test::expect![[r#"
            (
                Some(
                    40,
                ),
                Some(
                    1000,
                ),
                Some(
                    1020,
                ),
                Some(
                    1040,
                ),
                Some(
                    50,
                ),
                "http://127.0.0.1:18007",
                [
                    FullNode,
                    Validator,
                    Liteserver,
                ],
            )
        "#]]
        .assert_debug_eq(&(
            observation.payload.head_seqno,
            observation.payload.sync_initial_masterchain_block_time,
            observation.payload.sync_masterchain_block_time,
            observation.payload.sync_target_time,
            observation.payload.sync_progressed_at,
            observation.payload.observability_endpoint,
            observation.payload.roles,
        ));
        shutdown.send(true).unwrap();
        task.await.unwrap();
    }
}
