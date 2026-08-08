//! Serves the browser-facing TON HTTP API V2 endpoint.
//!
//! [`start`] binds the configured public address and forwards requests to the
//! loopback V2 backend. The proxy preserves the HTTP method, path, query, body,
//! status, and end-to-end headers. Transport-only headers are removed, OPTIONS
//! returns HTTP 204, and the shared CORS/PNA middleware decorates responses.

use std::net::Ipv4Addr;

use anyhow::Result;
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Request, State as AxumState},
    http::{
        HeaderMap, StatusCode,
        header::{CONNECTION, CONTENT_LENGTH, HOST, TRANSFER_ENCODING},
    },
    middleware,
    response::{IntoResponse, Response},
};
use serde_json::json;
use tokio::sync::watch;
use tracing::info;

use crate::storage::Settings;

use super::{RunningService, cors, server};

#[derive(Clone)]
struct State {
    backend: String,
    client: reqwest::Client,
}

pub(super) async fn start(
    settings: &Settings,
    bind: Ipv4Addr,
    shutdown: watch::Receiver<bool>,
) -> Result<RunningService> {
    let service = &settings.services.ton_http_api;
    let address = (bind, service.port).into();
    let app = router(format!("http://127.0.0.1:{}", service.backend_port));
    let endpoint_host = if bind.is_unspecified() {
        Ipv4Addr::LOCALHOST
    } else {
        bind
    };
    let endpoint = format!("http://{endpoint_host}:{}/api/v2", service.port);
    let running = server::start(
        "TON HTTP API public proxy",
        address,
        app,
        shutdown,
        endpoint.clone(),
    )
    .await?;
    info!(
        %endpoint,
        %bind,
        backend_port = service.backend_port,
        "TON HTTP API public CORS proxy started"
    );
    Ok(running)
}

pub(super) fn router(backend: String) -> Router {
    Router::new()
        .fallback(proxy)
        .layer(middleware::from_fn(cors::browser_headers))
        .with_state(State {
            backend,
            client: reqwest::Client::new(),
        })
}

async fn proxy(AxumState(state): AxumState<State>, request: Request) -> Result<Response, Response> {
    if request.method() == axum::http::Method::OPTIONS {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let url = format!("{}{path_and_query}", state.backend);
    let body = to_bytes(body, 32 * 1024 * 1024).await.map_err(|error| {
        proxy_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("failed to read request body: {error}"),
        )
    })?;

    let mut headers = parts.headers;
    remove_hop_by_hop_headers(&mut headers);
    let upstream = state
        .client
        .request(parts.method, url)
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|error| {
            proxy_error(
                StatusCode::BAD_GATEWAY,
                format!("TON HTTP API backend request failed: {error}"),
            )
        })?;

    let status = upstream.status();
    let mut headers = upstream.headers().clone();
    remove_hop_by_hop_headers(&mut headers);
    let body = upstream.bytes().await.map_err(|error| {
        proxy_error(
            StatusCode::BAD_GATEWAY,
            format!("failed to read TON HTTP API backend response: {error}"),
        )
    })?;
    Ok((status, headers, body).into_response())
}

fn remove_hop_by_hop_headers(headers: &mut HeaderMap) {
    headers.remove(HOST);
    headers.remove(CONNECTION);
    headers.remove(CONTENT_LENGTH);
    headers.remove(TRANSFER_ENCODING);
}

fn proxy_error(status: StatusCode, message: String) -> Response {
    (status, Json(json!({ "ok": false, "error": message }))).into_response()
}
