use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::state::AppState;

const LONG_VERSION: &str = env!("VERIFIER_LONG_VERSION");

pub async fn handler(State(state): State<AppState>) -> Response {
    if state.payment_verifier().is_ready() {
        return Json(HealthResponse {
            ok: true,
            payment_recovery: None,
        })
        .into_response();
    }

    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(HealthResponse {
            ok: false,
            payment_recovery: Some("rebuilding"),
        }),
    )
        .into_response()
}

pub async fn version() -> &'static str {
    LONG_VERSION
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    payment_recovery: Option<&'static str>,
}
