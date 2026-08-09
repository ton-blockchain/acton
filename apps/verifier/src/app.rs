use axum::{Router, http::Method, routing::get};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
};

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
    Router::<AppState>::new()
        .route("/healthz", get(handlers::health::handler))
        .route("/robots.txt", get(handlers::robots::handler))
        .route("/version", get(handlers::health::version))
        .nest("/api/v1", handlers::api::v1::router())
        .fallback(handlers::frontend::handler)
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers(Any),
        )
        .layer(CompressionLayer::new())
}
