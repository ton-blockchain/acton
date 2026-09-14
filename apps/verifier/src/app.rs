use axum::{Router, extract::DefaultBodyLimit, routing::get};
use tower_http::compression::CompressionLayer;

use crate::{
    config::Config,
    handlers,
    state::{AppState, StateError},
};

/// Builds a router with default configuration.
///
/// # Errors
///
/// Returns an error when application state cannot be initialized.
pub fn router() -> Result<Router, StateError> {
    Ok(router_with_state(
        AppState::from_config(&Config::default())?,
    ))
}

pub fn router_with_state(state: AppState) -> Router {
    let max_request_bytes = state.max_request_bytes();
    Router::<AppState>::new()
        .route("/healthz", get(handlers::health::handler))
        .route("/robots.txt", get(handlers::robots::handler))
        .route("/version", get(handlers::health::version))
        .nest("/api/v1", handlers::api::v1::router())
        .fallback(handlers::frontend::handler)
        .with_state(state)
        .layer(DefaultBodyLimit::max(max_request_bytes))
        .layer(CompressionLayer::new())
}
