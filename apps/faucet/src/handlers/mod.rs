use axum::{
    Router,
    http::{
        HeaderValue, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware::{self, from_fn_with_state},
    routing::{get, post},
};
use faucet_backend::middlewares::{
    ACTON_CLIENT_HEADER, DEVICE_UID_HEADER, require_airdrop_headers, require_pow_enabled,
};
use reqwest::Url;
use tower_http::cors::CorsLayer;

use crate::AppState;

mod auth;
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

    let browser_auth_routes = Router::new()
        .route("/auth/status", get(auth::status))
        .route("/auth/exchange", post(auth::exchange_grant))
        .route(
            "/auth/session",
            get(auth::get_session).delete(auth::delete_session),
        )
        .route_layer(middleware::from_fn(require_airdrop_headers));

    Router::new()
        .route("/", get(health::root))
        .route("/robots.txt", get(robots::robots_txt))
        .route("/ready", get(health::ok))
        .route("/health", get(health::ok))
        .route("/metrics", get(health::ok))
        .route("/stats", get(stats::get_stats))
        .route("/version", get(health::version))
        .route("/auth/github/start", get(auth::github_start))
        .route("/auth/github/callback", get(auth::github_callback))
        .merge(browser_auth_routes)
        .merge(airdrop_routes)
        .with_state(state)
}

pub(crate) fn airdrop_cors_layer(frontend_url: &str) -> anyhow::Result<CorsLayer> {
    let frontend_url = Url::parse(frontend_url)
        .map_err(|error| anyhow::anyhow!("Invalid frontend URL: {error}"))?;
    let frontend_origin = HeaderValue::from_str(&frontend_url.origin().ascii_serialization())
        .map_err(|error| anyhow::anyhow!("Invalid frontend origin: {error}"))?;
    let mut origins = vec![
        HeaderValue::from_static("https://actonscan.com"),
        HeaderValue::from_static("http://localhost:3007"),
        HeaderValue::from_static("http://127.0.0.1:3007"),
    ];
    if !origins.contains(&frontend_origin) {
        origins.push(frontend_origin);
    }

    Ok(CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers([
            CONTENT_TYPE,
            AUTHORIZATION,
            DEVICE_UID_HEADER,
            ACTON_CLIENT_HEADER,
        ]))
}

#[cfg(test)]
mod tests {
    use super::airdrop_cors_layer;
    use axum::{
        Router,
        body::Body,
        extract::Request,
        http::{
            Method, StatusCode,
            header::{
                ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
                ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
                ACCESS_CONTROL_REQUEST_METHOD, ORIGIN,
            },
        },
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::post,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn actonscan_preflight_bypasses_inner_rate_limit() {
        let app = Router::new()
            .route("/challenge", post(|| async { "ok" }))
            .route_layer(middleware::from_fn(always_rate_limited))
            .layer(
                airdrop_cors_layer("https://actonscan.com/faucet")
                    .expect("valid CORS configuration"),
            );
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/challenge")
            .header(ORIGIN, "https://actonscan.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(
                ACCESS_CONTROL_REQUEST_HEADERS,
                "content-type,authorization,x-device-uid,x-acton-client",
            )
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "https://actonscan.com"
        );
        assert_eq!(
            response
                .headers()
                .get(ACCESS_CONTROL_ALLOW_METHODS)
                .unwrap(),
            "GET,POST,DELETE"
        );
        assert_eq!(
            response
                .headers()
                .get(ACCESS_CONTROL_ALLOW_HEADERS)
                .unwrap(),
            "content-type,authorization,x-device-uid,x-acton-client"
        );
    }

    async fn always_rate_limited(_request: Request, _next: Next) -> Response {
        StatusCode::TOO_MANY_REQUESTS.into_response()
    }

    #[tokio::test]
    async fn rejects_unknown_browser_origin() {
        let app = Router::new()
            .route("/challenge", post(|| async { "ok" }))
            .layer(
                airdrop_cors_layer("https://actonscan.com/faucet")
                    .expect("valid CORS configuration"),
            );
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/challenge")
            .header(ORIGIN, "https://example.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }

    #[tokio::test]
    async fn allows_configured_frontend_origin() {
        let app = Router::new()
            .route("/auth/exchange", post(|| async { "ok" }))
            .layer(
                airdrop_cors_layer("https://staging.example.com/faucet?network=testnet")
                    .expect("valid CORS configuration"),
            );
        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/auth/exchange")
            .header(ORIGIN, "https://staging.example.com")
            .header(ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(),
            "https://staging.example.com"
        );
    }
}
