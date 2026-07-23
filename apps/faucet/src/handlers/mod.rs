use axum::{
    Router,
    middleware::{self, from_fn_with_state},
    routing::{get, post},
};
use faucet_backend::middlewares::{require_airdrop_headers, require_pow_enabled};

use crate::AppState;

mod challenge;
mod claim;
mod health;
mod robots;
mod stats;

pub(crate) use claim::CreateClaim;

pub(crate) fn router(state: AppState) -> Router {
    let airdrop_routes = Router::new()
        .route("/challenge", post(challenge::create_challenge))
        .route("/claim", post(claim::create_claim))
        .route_layer(from_fn_with_state(
            state.config.clone(),
            require_pow_enabled,
        ))
        .route_layer(middleware::from_fn(require_airdrop_headers));

    Router::new()
        .route("/", get(health::root))
        .route("/robots.txt", get(robots::robots_txt))
        .route("/ready", get(health::ok))
        .route("/health", get(health::ok))
        .route("/metrics", get(health::ok))
        .route("/stats", get(stats::get_stats))
        .route("/version", get(health::version))
        .merge(airdrop_routes)
        .with_state(state)
}
