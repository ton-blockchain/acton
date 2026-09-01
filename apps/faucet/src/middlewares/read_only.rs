use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use faucet_config::Config;
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub async fn require_faucet_writable(
    State(config): State<Arc<Config>>,
    request: Request,
    next: Next,
) -> Response {
    if !config.faucet.read_only {
        return next.run(request).await;
    }

    response_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "Faucet is in read-only mode",
    )
    .into_response()
}

fn response_error(status: StatusCode, error: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error }))
}
