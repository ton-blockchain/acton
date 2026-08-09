//! Actonscan backend HTTP API and network indexer.

pub mod config;
mod indexer;
pub mod opcodes;
pub mod stats;
mod storage;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderValue, Method, StatusCode, header::CACHE_CONTROL},
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use tower_http::{compression::CompressionLayer, cors::CorsLayer};
use utoipa::OpenApi;

use crate::{
    config::IndexerConfig,
    opcodes::{OpcodeCount, OpcodeSnapshot},
    stats::{TpsSnapshot, TpsStatus, TpsWindow},
};

pub use config::Config;
pub use opcodes::OpcodeStats;
pub use stats::TpsStats;
pub use storage::SqliteStorage;

const DEFAULT_OPCODE_LIMIT: usize = 100;
const DEFAULT_OPCODE_MIN_MESSAGES: u64 = 2;
const MAX_OPCODE_LIMIT: usize = 1_000;

#[derive(Clone)]
struct AppState {
    tps: TpsStats,
    opcodes: OpcodeStats,
}

#[derive(Deserialize)]
struct OpcodeStatsQuery {
    limit: Option<usize>,
    min_messages: Option<u64>,
}

#[derive(OpenApi)]
#[openapi(
    paths(tps, opcode_stats),
    components(schemas(TpsSnapshot, TpsStatus, TpsWindow, OpcodeSnapshot, OpcodeCount))
)]
struct ApiDoc;

/// Builds the public Actonscan backend router.
pub fn app(tps_stats: TpsStats, opcode_state: OpcodeStats) -> Router {
    let api = Router::new()
        .route("/stats/tps", get(tps))
        .route("/stats/opcodes", get(opcode_stats));
    Router::new()
        .route("/healthz", get(health))
        .route("/openapi.json", get(openapi))
        .nest("/api/v1", api)
        .with_state(AppState {
            tps: tps_stats,
            opcodes: opcode_state,
        })
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([Method::GET, Method::OPTIONS])
                .allow_headers(tower_http::cors::Any),
        )
        .layer(CompressionLayer::new())
}

/// Starts the LiteServer-backed statistics indexer in the current Tokio runtime.
#[must_use]
pub fn spawn_indexer(
    config: IndexerConfig,
    tps_stats: TpsStats,
    opcode_stats: OpcodeStats,
    storage: SqliteStorage,
) -> tokio::task::JoinHandle<()> {
    indexer::spawn(config, tps_stats, opcode_stats, storage)
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/tps",
    responses((status = 200, description = "Rolling network TPS", body = TpsSnapshot))
)]
async fn tps(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=1, stale-while-revalidate=4"),
        )],
        Json(state.tps.snapshot().await),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/opcodes",
    params(
        ("limit" = Option<usize>, Query, description = "Maximum number of opcodes in the response", minimum = 1, maximum = 1_000),
        ("min_messages" = Option<u64>, Query, description = "Minimum message count for each opcode", minimum = 1)
    ),
    responses((status = 200, description = "Most frequent all-time message opcodes", body = OpcodeSnapshot))
)]
async fn opcode_stats(
    State(state): State<AppState>,
    Query(query): Query<OpcodeStatsQuery>,
) -> impl IntoResponse {
    let limit = query
        .limit
        .unwrap_or(DEFAULT_OPCODE_LIMIT)
        .clamp(1, MAX_OPCODE_LIMIT);
    let min_messages = query
        .min_messages
        .unwrap_or(DEFAULT_OPCODE_MIN_MESSAGES)
        .max(1);
    (
        [(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=1, stale-while-revalidate=4"),
        )],
        Json(state.opcodes.snapshot(limit, min_messages).await),
    )
}
