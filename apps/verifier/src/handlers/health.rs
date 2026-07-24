use axum::{Json, response::IntoResponse};
use serde::Serialize;

const LONG_VERSION: &str = env!("VERIFIER_LONG_VERSION");

pub async fn handler() -> impl IntoResponse {
    Json(HealthResponse { ok: true })
}

pub async fn version() -> &'static str {
    LONG_VERSION
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
}
