//! Public observability API, signed peer exchange, block collector, and UI.

#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    net::Ipv4Addr,
    sync::Arc,
    time::Duration,
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
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::future::join_all;
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use tokio::{sync::RwLock, sync::watch, task::JoinHandle, time::MissedTickBehavior};
use ton::{block_tlb::Block, ton_core::traits::tlb::TLB};
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};
use tracing::{info, warn};
use utoipa::OpenApi;

#[cfg(not(debug_assertions))]
use axum::http::Uri;

use crate::{
    observability::{
        BlockObservation, ChainHead, ChainObservation, ElectionObservation, ExchangeRequest,
        ExchangeResponse, NetworkView, NodeObservation, ObservationPayload, ObservationStore,
        ObserverIdentity, ProductionView, ShardHead, SignedObservation, network_id,
    },
    operations::validators,
    storage::{Layout, Manifest, RuntimeState, Settings, unix_time},
    ton::{
        lite::{BlockRef, LocalLiteClient},
        toolchain::Toolchain,
    },
};

use super::{
    RunningService, cors,
    error::{ErrorResponse, HttpError},
    server,
};

const INITIAL_BACKFILL_BLOCKS: u32 = 64;
const MAX_CATCHUP_BLOCKS_PER_TICK: u32 = 128;
const MAX_RETAINED_BLOCKS: usize = 20_000;
const MAX_GOSSIP_BLOCK_OBSERVATIONS: usize = 64;
const MAX_EXCHANGE_BODY_BYTES: usize = 2 * 1024 * 1024;
const ELECTION_POLL_INTERVAL_SECONDS: u64 = 15;

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> =
    include_dir!("$CARGO_MANIFEST_DIR/../../packages/localton-ui/dist/.embedded");

type SharedStore = Arc<RwLock<ObservationStore>>;

#[derive(Clone)]
struct ObservabilityState {
    store: SharedStore,
}

pub(super) struct RunningObservability {
    pub service: RunningService,
    pub tasks: Vec<JoinHandle<()>>,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "localton Observability API",
        description = "Signed node health, independently observed block production, and peer exchange"
    ),
    paths(network_handler, local_observation_handler, exchange_handler, health_handler),
    components(schemas(
        NetworkView,
        SignedObservation,
        ExchangeRequest,
        ExchangeResponse,
        ErrorResponse
    )),
    tags((name = "observability", description = "Decentralized TON network observations"))
)]
struct ApiDoc;

pub(super) async fn start(
    layout: Layout,
    toolchain: Toolchain,
    settings: &Settings,
    owned_nodes: BTreeSet<String>,
    advertised_ip: Ipv4Addr,
    extra_peers: Vec<String>,
    shutdown: watch::Receiver<bool>,
) -> Result<RunningObservability> {
    remember_genesis_validator_key(&layout, settings);
    let observability = &settings.services.observability;
    let address = observability.socket_addr();
    let endpoint = format!("http://{advertised_ip}:{}", observability.port);
    let identity = ObserverIdentity::load_or_create(&layout.observability.join("identity.json"))?;
    let store = Arc::new(RwLock::new(ObservationStore::new(
        network_id(&layout.global_config)?,
        identity,
        observability.block_window_seconds,
    )));
    let state = ObservabilityState {
        store: Arc::clone(&store),
    };
    let api = Router::new()
        .route("/openapi.json", get(openapi_handler))
        .route("/network", get(network_handler))
        .route("/observation", get(local_observation_handler))
        .route("/exchange", post(exchange_handler))
        .route("/{*path}", options(cors::preflight));
    let app = Router::new()
        .nest("/api/v1", api)
        .route("/healthz", get(health_handler))
        .layer(DefaultBodyLimit::max(MAX_EXCHANGE_BODY_BYTES))
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
    let mut peers = settings.services.observability.peers.clone();
    peers.extend(extra_peers);
    peers.sort();
    peers.dedup();
    let collector = tokio::spawn(collector_loop(
        layout,
        toolchain,
        settings.clone(),
        owned_nodes,
        endpoint.clone(),
        Arc::clone(&store),
        shutdown.clone(),
    ));
    let gossip = tokio::spawn(gossip_loop(
        store,
        endpoint.clone(),
        peers,
        settings.services.observability.publish_interval_seconds,
        settings.services.observability.gossip_fanout,
        shutdown,
    ));
    info!(%endpoint, "observability API and UI started");
    Ok(RunningObservability {
        service,
        tasks: vec![collector, gossip],
    })
}

fn remember_genesis_validator_key(layout: &Layout, settings: &Settings) {
    if !layout.manifest.is_file() {
        return;
    }
    let result = (|| {
        let manifest = Manifest::load(&layout.manifest)?;
        let node = settings.node("genesis")?;
        let public_key = match manifest.validator_public_key {
            Some(public_key) => public_key,
            None => STANDARD.encode(std::fs::read(
                layout.validator_keyring.join("validator.pub"),
            )?),
        };
        RuntimeState::update_atomic(&layout.runtime, |runtime| {
            let node_runtime = runtime.nodes.entry(node.name.clone()).or_default();
            node_runtime.remember_validator_public_key(public_key);
            if node_runtime.liteserver_public_key.is_none() {
                node_runtime.liteserver_public_key = Some(manifest.liteserver_public_key.clone());
            }
            Ok(())
        })
    })();
    if let Err(error) = result {
        warn!(%error, "genesis validator identity observation failed");
    }
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
    Json(state.store.write().await.aggregate(unix_time()))
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
    path = "/api/v1/exchange",
    tag = "observability",
    request_body = ExchangeRequest,
    responses(
        (status = 200, description = "Peer versions and missing observations", body = ExchangeResponse),
        (status = 400, description = "A signed observation was invalid", body = ErrorResponse)
    )
)]
async fn exchange_handler(
    State(state): State<ObservabilityState>,
    Json(request): Json<ExchangeRequest>,
) -> Result<Json<ExchangeResponse>, HttpError> {
    let now = unix_time();
    let mut store = state.store.write().await;
    store.ingest(request.observations, now)?;
    Ok(Json(ExchangeResponse {
        observations: store.delta(&request.known, now),
        known: store.known(),
    }))
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

async fn collector_loop(
    layout: Layout,
    toolchain: Toolchain,
    settings: Settings,
    owned_nodes: BTreeSet<String>,
    endpoint: String,
    store: SharedStore,
    mut shutdown: watch::Receiver<bool>,
) {
    let interval_seconds = settings.services.observability.publish_interval_seconds;
    let ttl_seconds = settings.services.observability.observation_ttl_seconds;
    let block_window_seconds = settings.services.observability.block_window_seconds;
    let mut chain = ChainCollector::default();
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let publication = PublicationConfig {
        endpoint: &endpoint,
        ttl_seconds,
        block_window_seconds,
    };
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(error) = collect_and_publish(
                    &layout,
                    &toolchain,
                    &settings,
                    &owned_nodes,
                    &publication,
                    &mut chain,
                    &store,
                ).await {
                    warn!(%error, "observability collection failed");
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

struct PublicationConfig<'a> {
    endpoint: &'a str,
    ttl_seconds: u64,
    block_window_seconds: u64,
}

async fn collect_and_publish(
    layout: &Layout,
    toolchain: &Toolchain,
    settings: &Settings,
    owned_nodes: &BTreeSet<String>,
    publication: &PublicationConfig<'_>,
    chain: &mut ChainCollector,
    store: &SharedStore,
) -> Result<()> {
    let now = unix_time();
    if let Err(error) = chain
        .update(toolchain, now, publication.block_window_seconds)
        .await
    {
        warn!(%error, "chain observation update failed");
    }
    let runtime = RuntimeState::load(&layout.runtime)?;
    let network_head = chain.observation.as_ref().map(|chain| chain.head.seqno);
    let reports = owned_nodes
        .iter()
        .filter_map(|name| settings.node(name).ok().cloned())
        .map(|node| {
            let global_config = layout.global_config.clone();
            let runtime = runtime.nodes.get(&node.name).cloned().unwrap_or_default();
            async move {
                let head_seqno = if runtime.running {
                    match runtime.liteserver_public_key.as_deref() {
                        Some(public_key) => {
                            match LocalLiteClient::connect_node(
                                &global_config,
                                node.liteserver_port,
                                public_key,
                            )
                            .await
                            {
                                Ok(mut client) => client.last().await.ok().map(|head| head.seqno),
                                Err(_) => None,
                            }
                        }
                        None => None,
                    }
                } else {
                    None
                };
                let mut roles = vec!["full_node".to_owned()];
                if node.validator {
                    roles.push("validator".to_owned());
                }
                if node.liteserver {
                    roles.push("liteserver".to_owned());
                }
                NodeObservation {
                    name: node.name,
                    public_ip: node.public_ip.to_string(),
                    roles,
                    running: runtime.running,
                    process_id: runtime.pid,
                    status: runtime.status,
                    last_error: runtime.last_error,
                    head_seqno,
                    sync_lag_blocks: network_head
                        .zip(head_seqno)
                        .map(|(network, node)| network.saturating_sub(node)),
                    validator_public_key: runtime.validator_public_key.clone(),
                    validator_public_keys: runtime.validator_public_keys,
                    validator_adnl: runtime.validator_adnl,
                }
            }
        });
    let nodes = join_all(reports).await;
    let payload = ObservationPayload {
        endpoint: publication.endpoint.to_owned(),
        software: format!("localton/{}", env!("CARGO_PKG_VERSION")),
        launcher_started_at: runtime.started_at,
        nodes,
        chain: chain.observation.clone(),
    };
    store
        .write()
        .await
        .publish(payload, now, publication.ttl_seconds)?;
    Ok(())
}

#[derive(Default)]
struct ChainCollector {
    last_scanned_seqno: Option<u32>,
    last_election_update: Option<u64>,
    blocks: BTreeMap<String, BlockObservation>,
    election: Option<ElectionObservation>,
    observation: Option<ChainObservation>,
}

impl ChainCollector {
    async fn update(&mut self, toolchain: &Toolchain, now: u64, window_seconds: u64) -> Result<()> {
        let mut client = LocalLiteClient::connect(&toolchain.layout.global_config).await?;
        let network_head = client.last().await?;
        if self
            .last_election_update
            .is_none_or(|updated| now.saturating_sub(updated) >= ELECTION_POLL_INTERVAL_SECONDS)
        {
            self.last_election_update = Some(now);
            match validators::election_status(toolchain).await {
                Ok(info) => {
                    let elections_open_at = info
                        .current
                        .until
                        .saturating_sub(info.elections_start_before);
                    let elections_close_at =
                        info.current.until.saturating_sub(info.elections_end_before);
                    let next_validators = info.next.as_ref().map(|set| set.total);
                    let next_set_activation_at = info
                        .next
                        .as_ref()
                        .map_or(info.current.until, |set| set.since);
                    self.election = Some(ElectionObservation {
                        round_id: next_set_activation_at,
                        stage: election_stage(
                            now,
                            elections_open_at,
                            elections_close_at,
                            next_set_activation_at,
                            next_validators.is_some(),
                        )
                        .to_owned(),
                        validation_started_at: info.current.since,
                        elections_open_at,
                        elections_close_at,
                        next_set_activation_at,
                        validators_elected_for: info.validators_elected_for,
                        stake_held_for: info.stake_held_for,
                        current_validators: info.current.total,
                        current_main_validators: info.current.main,
                        next_validators,
                    });
                }
                Err(error) => warn!(%error, "election observation update failed"),
            }
        }
        if let Some(election) = &mut self.election {
            election.stage = election_stage(
                now,
                election.elections_open_at,
                election.elections_close_at,
                election.next_set_activation_at,
                election.next_validators.is_some(),
            )
            .to_owned();
        }
        let election = self.election.clone();
        let first = self.last_scanned_seqno.map_or_else(
            || {
                network_head
                    .seqno
                    .saturating_sub(INITIAL_BACKFILL_BLOCKS - 1)
                    .max(1)
            },
            |seqno| seqno.saturating_add(1).min(network_head.seqno),
        );
        let last = network_head
            .seqno
            .min(first.saturating_add(MAX_CATCHUP_BLOCKS_PER_TICK - 1));
        let mut latest = None;
        for seqno in first..=last {
            let (id, bytes) = match client.block(-1, "8000000000000000", seqno).await {
                Ok(block) => block,
                Err(error) => {
                    warn!(seqno, %error, "masterchain block observation skipped");
                    continue;
                }
            };
            let block = match parse_block(&id, bytes) {
                Ok(block) => block,
                Err(error) => {
                    warn!(seqno, %error, "invalid masterchain block observation skipped");
                    continue;
                }
            };
            let shard_ids = block
                .1
                .extra
                .mc_block_extra
                .as_ref()
                .map(|extra| extra.shard_ids())
                .unwrap_or_default();
            let shards = block
                .1
                .extra
                .mc_block_extra
                .as_ref()
                .map(|extra| {
                    extra
                        .shard_hashes
                        .iter()
                        .flat_map(|(workchain, shards)| {
                            shards.iter().map(|(prefix, shard)| ShardHead {
                                workchain: *workchain,
                                shard: format!("{:016x}", prefix.to_shard()),
                                seqno: shard.seqno,
                                root_hash: hex::encode(shard.root_hash.as_slice_sized()),
                                file_hash: hex::encode(shard.file_hash.as_slice_sized()),
                                gen_utime: shard.gen_utime,
                                before_split: shard.before_split,
                                before_merge: shard.before_merge,
                                want_split: shard.want_split,
                                want_merge: shard.want_merge,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            self.blocks.insert(block.0.id.clone(), block.0.clone());
            latest = Some((id.clone(), block.0.gen_utime, shards));
            for shard_id in shard_ids {
                if shard_id.seqno == 0 {
                    continue;
                }
                let shard = format!("{:016x}", shard_id.shard_ident.shard);
                let key = format!(
                    "{}:{shard}:{}",
                    shard_id.shard_ident.workchain, shard_id.seqno
                );
                if self.blocks.contains_key(&key) {
                    continue;
                }
                let (id, bytes) = match client
                    .block(shard_id.shard_ident.workchain, &shard, shard_id.seqno)
                    .await
                {
                    Ok(block) => block,
                    Err(error) => {
                        warn!(
                            workchain = shard_id.shard_ident.workchain,
                            shard,
                            seqno = shard_id.seqno,
                            %error,
                            "shard block observation skipped"
                        );
                        continue;
                    }
                };
                let parsed = match parse_block(&id, bytes) {
                    Ok(block) => block.0,
                    Err(error) => {
                        warn!(
                            workchain = shard_id.shard_ident.workchain,
                            shard,
                            seqno = shard_id.seqno,
                            %error,
                            "invalid shard block observation skipped"
                        );
                        continue;
                    }
                };
                self.blocks.insert(parsed.id.clone(), parsed);
            }
            self.last_scanned_seqno = Some(seqno);
        }
        let cutoff = now.saturating_sub(window_seconds);
        self.blocks
            .retain(|_, block| u64::from(block.gen_utime) >= cutoff);
        while self.blocks.len() > MAX_RETAINED_BLOCKS {
            let Some(first) = self.blocks.keys().next().cloned() else {
                break;
            };
            self.blocks.remove(&first);
        }
        if let Some((head, gen_utime, shards)) = latest {
            let window_started_at = self
                .blocks
                .values()
                .map(|block| u64::from(block.gen_utime))
                .min()
                .unwrap_or(now);
            let mut production = BTreeMap::<String, ProductionView>::new();
            for block in self.blocks.values() {
                let entry =
                    production
                        .entry(block.creator.clone())
                        .or_insert_with(|| ProductionView {
                            creator: block.creator.clone(),
                            masterchain_blocks: 0,
                            shard_blocks: 0,
                            last_block_at: 0,
                        });
                if block.workchain == -1 {
                    entry.masterchain_blocks = entry.masterchain_blocks.saturating_add(1);
                } else {
                    entry.shard_blocks = entry.shard_blocks.saturating_add(1);
                }
                entry.last_block_at = entry.last_block_at.max(block.gen_utime);
            }
            let mut recent_blocks = self.blocks.values().cloned().collect::<Vec<_>>();
            recent_blocks.sort_by_key(|block| {
                (
                    block.gen_utime,
                    block.workchain,
                    block.shard.clone(),
                    block.seqno,
                )
            });
            let recent_from = recent_blocks
                .len()
                .saturating_sub(MAX_GOSSIP_BLOCK_OBSERVATIONS);
            self.observation = Some(ChainObservation {
                head: ChainHead {
                    seqno: head.seqno,
                    root_hash: head.root_hash,
                    file_hash: head.file_hash,
                    gen_utime,
                    observed_at: now,
                    shard_count: shards.len(),
                },
                window_started_at,
                shards,
                election,
                production: production.into_values().collect(),
                blocks: recent_blocks.split_off(recent_from),
            });
        }
        Ok(())
    }
}

fn election_stage(
    now: u64,
    elections_open_at: u32,
    elections_close_at: u32,
    next_set_activation_at: u32,
    has_next_set: bool,
) -> &'static str {
    if now < u64::from(elections_open_at) {
        "validation"
    } else if now < u64::from(elections_close_at) {
        "accepting_entries"
    } else if now < u64::from(next_set_activation_at) {
        if has_next_set {
            "next_set_ready"
        } else {
            "finalizing"
        }
    } else if has_next_set {
        "activation_overdue"
    } else {
        "retrying"
    }
}

fn parse_block(id: &BlockRef, bytes: Vec<u8>) -> Result<(BlockObservation, Block)> {
    let block = Block::from_boc(bytes).context("failed to decode TON block")?;
    let info = &block.info;
    let observation = BlockObservation {
        id: format!("{}:{}:{}", id.workchain, id.shard, id.seqno),
        workchain: id.workchain,
        shard: id.shard.clone(),
        seqno: id.seqno,
        root_hash: id.root_hash.clone(),
        file_hash: id.file_hash.clone(),
        gen_utime: info.gen_utime,
        creator: hex::encode(block.extra.created_by.as_slice_sized()),
    };
    Ok((observation, block))
}

async fn gossip_loop(
    store: SharedStore,
    own_endpoint: String,
    bootstrap_peers: Vec<String>,
    interval_seconds: u64,
    fanout: usize,
    mut shutdown: watch::Receiver<bool>,
) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            warn!(%error, "failed to build observability gossip client");
            return;
        }
    };
    let mut peer_known = HashMap::<String, BTreeMap<String, u64>>::new();
    let mut round = 0_usize;
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let now = unix_time();
                let mut candidates = BTreeSet::from_iter(bootstrap_peers.iter().cloned());
                candidates.extend(store.read().await.endpoints(now));
                candidates.remove(&own_endpoint);
                let candidates = candidates.into_iter().collect::<Vec<_>>();
                if !candidates.is_empty() {
                    for offset in 0..fanout.min(candidates.len()) {
                        let peer = candidates[(round + offset) % candidates.len()].clone();
                        if let Err(error) = exchange_with_peer(
                            &client,
                            &store,
                            &peer,
                            peer_known.entry(peer.clone()).or_default(),
                        ).await {
                            warn!(peer, %error, "observability peer exchange failed");
                        }
                    }
                    round = round.wrapping_add(fanout.max(1));
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
            }
        }
    }
}

async fn exchange_with_peer(
    client: &reqwest::Client,
    store: &SharedStore,
    peer: &str,
    peer_known: &mut BTreeMap<String, u64>,
) -> Result<()> {
    let now = unix_time();
    let request = {
        let store = store.read().await;
        ExchangeRequest {
            known: store.known(),
            observations: store.delta(peer_known, now),
        }
    };
    let url = format!("{}/api/v1/exchange", peer.trim_end_matches('/'));
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .with_context(|| format!("failed to request {url}"))?
        .error_for_status()
        .with_context(|| format!("peer rejected {url}"))?
        .json::<ExchangeResponse>()
        .await
        .context("peer returned an invalid exchange response")?;
    store.write().await.ingest(response.observations, now)?;
    *peer_known = response.known;
    Ok(())
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
    fn openapi_exposes_exchange_and_network_view() {
        let document = serde_json::to_value(ApiDoc::openapi()).unwrap();
        assert!(document["paths"]["/api/v1/network"]["get"].is_object());
        assert!(document["paths"]["/api/v1/exchange"]["post"].is_object());
        assert!(document["components"]["schemas"]["NetworkView"].is_object());
    }

    #[test]
    fn exchange_payload_shape_is_stable() {
        let request = ExchangeRequest {
            known: BTreeMap::from([("observer".to_owned(), 4)]),
            observations: Vec::new(),
        };
        expect_test::expect![[r#"
            {
              "known": {
                "observer": 4
              },
              "observations": []
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&request).unwrap());
    }

    #[test]
    fn election_without_a_next_set_is_reported_as_retrying() {
        expect_test::expect!["retrying"].assert_eq(election_stage(121, 30, 90, 120, false));
        expect_test::expect!["activation_overdue"]
            .assert_eq(election_stage(121, 30, 90, 120, true));
    }
}
