//! Adds browser CORS and Private Network Access response headers.
//!
//! Preflight responses allow the requested headers and common HTTP methods for
//! one day. Normal responses expose all headers. Both response types allow any
//! origin, opt in to private-network requests, and use a cross-origin resource
//! policy. The admin faucet and public V2 proxy share this middleware.

use axum::{
    extract::Request,
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};

pub(super) async fn preflight() -> StatusCode {
    StatusCode::NO_CONTENT
}

pub(super) async fn browser_headers(request: Request, next: Next) -> Response {
    let request_headers = request.headers().clone();
    let preflight = request.method() == axum::http::Method::OPTIONS;
    let mut response = next.run(request).await;
    apply_browser_headers(response.headers_mut(), &request_headers, preflight);
    response
}

pub(super) fn apply_browser_headers(
    headers: &mut HeaderMap,
    request_headers: &HeaderMap,
    preflight: bool,
) {
    headers.insert(
        HeaderName::from_static("access-control-allow-origin"),
        HeaderValue::from_static("*"),
    );
    headers.insert(
        HeaderName::from_static("access-control-allow-private-network"),
        HeaderValue::from_static("true"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-resource-policy"),
        HeaderValue::from_static("cross-origin"),
    );

    if preflight {
        headers.insert(
            HeaderName::from_static("access-control-allow-methods"),
            HeaderValue::from_static("GET, POST, PUT, PATCH, DELETE, OPTIONS"),
        );
        let requested_headers = request_headers
            .get("access-control-request-headers")
            .cloned()
            .unwrap_or_else(|| HeaderValue::from_static("content-type, authorization"));
        headers.insert(
            HeaderName::from_static("access-control-allow-headers"),
            requested_headers,
        );
        headers.insert(
            HeaderName::from_static("access-control-max-age"),
            HeaderValue::from_static("86400"),
        );
    } else {
        headers.insert(
            HeaderName::from_static("access-control-expose-headers"),
            HeaderValue::from_static("*"),
        );
    }
}
