use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::header::{ACCEPT, CONTENT_TYPE, RETRY_AFTER, USER_AGENT};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use futures::StreamExt;
use serde_json::json;

use crate::StudioState;

const DEVICE_UID_HEADER: &str = "x-device-uid";
const MAX_FAUCET_REQUEST_BYTES: usize = 16 * 1024;
const MAX_FAUCET_RESPONSE_BYTES: usize = 64 * 1024;
const FAUCET_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn router() -> Router<StudioState> {
    Router::new()
        .route("/testnet-faucet/auth/status", get(auth_status))
        .route("/testnet-faucet/challenge", post(challenge))
        .route("/testnet-faucet/claim", post(claim))
}

#[utoipa::path(
    get,
    path = "/api/v1/testnet-faucet/auth/status",
    responses(
        (status = 200, description = "Guest Testnet faucet limits", body = Object),
        (status = 400, description = "Missing or invalid Studio device identity", body = crate::StudioApiErrorBody),
        (status = 502, description = "Failed to reach the Testnet faucet", body = crate::StudioApiErrorBody)
    ),
    tag = "testnet faucet"
)]
pub(crate) async fn auth_status(
    State(state): State<StudioState>,
    headers: HeaderMap,
) -> Result<Response, TestnetFaucetProxyError> {
    proxy_request(state, Method::GET, "auth/status", headers, None).await
}

#[utoipa::path(
    post,
    path = "/api/v1/testnet-faucet/challenge",
    request_body(content = Object, description = "Guest proof-of-work challenge request"),
    responses(
        (status = 200, description = "Proof-of-work challenge", body = Object),
        (status = 400, description = "Invalid Testnet address or request", body = Object),
        (status = 429, description = "Guest faucet limit reached", body = Object),
        (status = 502, description = "Failed to reach the Testnet faucet", body = crate::StudioApiErrorBody)
    ),
    tag = "testnet faucet"
)]
pub(crate) async fn challenge(
    State(state): State<StudioState>,
    request: Request,
) -> Result<Response, TestnetFaucetProxyError> {
    proxy_json_request(state, "challenge", request).await
}

#[utoipa::path(
    post,
    path = "/api/v1/testnet-faucet/claim",
    request_body(content = Object, description = "Solved guest proof-of-work challenge"),
    responses(
        (status = 200, description = "Queued Testnet faucet claim", body = Object),
        (status = 400, description = "Invalid or expired challenge", body = Object),
        (status = 403, description = "Destination is not eligible for funding", body = Object),
        (status = 429, description = "Guest faucet limit reached", body = Object),
        (status = 502, description = "Failed to reach the Testnet faucet", body = crate::StudioApiErrorBody)
    ),
    tag = "testnet faucet"
)]
pub(crate) async fn claim(
    State(state): State<StudioState>,
    request: Request,
) -> Result<Response, TestnetFaucetProxyError> {
    proxy_json_request(state, "claim", request).await
}

async fn proxy_json_request(
    state: StudioState,
    path: &'static str,
    request: Request,
) -> Result<Response, TestnetFaucetProxyError> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, MAX_FAUCET_REQUEST_BYTES)
        .await
        .map_err(|_| TestnetFaucetProxyError::invalid_request("Faucet request is too large"))?;

    proxy_request(state, Method::POST, path, parts.headers, Some(body)).await
}

async fn proxy_request(
    state: StudioState,
    method: Method,
    path: &'static str,
    headers: HeaderMap,
    body: Option<axum::body::Bytes>,
) -> Result<Response, TestnetFaucetProxyError> {
    let device_uid = device_uid(&headers)?;
    let url = state
        .testnet_faucet_url
        .join(path)
        .map_err(|error| TestnetFaucetProxyError::internal(error.to_string()))?;
    let mut upstream = state
        .http_client
        .request(method, url)
        .timeout(FAUCET_REQUEST_TIMEOUT)
        .header(ACCEPT, "application/json")
        .header(USER_AGENT, format!("acton/{}", state.info.server_version))
        .header(DEVICE_UID_HEADER, device_uid);
    if let Some(body) = body {
        upstream = upstream.header(CONTENT_TYPE, "application/json").body(body);
    }

    let upstream = upstream
        .send()
        .await
        .map_err(TestnetFaucetProxyError::upstream)?;
    let status = upstream.status();
    let content_type = upstream.headers().get(CONTENT_TYPE).cloned();
    let retry_after = upstream.headers().get(RETRY_AFTER).cloned();
    let mut response_body = Vec::new();
    let mut stream = upstream.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(TestnetFaucetProxyError::upstream)?;
        if chunk.len() > MAX_FAUCET_RESPONSE_BYTES.saturating_sub(response_body.len()) {
            return Err(TestnetFaucetProxyError::bad_gateway(
                "Testnet faucet response exceeded the Studio limit",
            ));
        }
        response_body.extend_from_slice(&chunk);
    }

    let mut response = Response::builder().status(status);
    if let Some(content_type) = content_type {
        response = response.header(CONTENT_TYPE, content_type);
    }
    if let Some(retry_after) = retry_after {
        response = response.header(RETRY_AFTER, retry_after);
    }
    response
        .body(Body::from(response_body))
        .map_err(|error| TestnetFaucetProxyError::internal(error.to_string()))
}

fn device_uid(headers: &HeaderMap) -> Result<HeaderValue, TestnetFaucetProxyError> {
    let value = headers.get(DEVICE_UID_HEADER).ok_or_else(|| {
        TestnetFaucetProxyError::invalid_request("Missing Studio device identity")
    })?;
    let valid = value.to_str().is_ok_and(|value| {
        value == "default"
            || matches!(value.len(), 32 | 36)
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });
    if !valid {
        return Err(TestnetFaucetProxyError::invalid_request(
            "Invalid Studio device identity",
        ));
    }

    Ok(value.clone())
}

pub(crate) struct TestnetFaucetProxyError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl TestnetFaucetProxyError {
    fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "testnet_faucet_invalid_request",
            message: message.into(),
        }
    }

    fn upstream(error: reqwest::Error) -> Self {
        Self::bad_gateway(format!("Failed to reach the Testnet faucet: {error}"))
    }

    fn bad_gateway(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: "testnet_faucet_unavailable",
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "testnet_faucet_proxy_failed",
            message: message.into(),
        }
    }
}

impl IntoResponse for TestnetFaucetProxyError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({
                "error": {
                    "code": self.code,
                    "message": self.message,
                }
            })),
        )
            .into_response()
    }
}
