use axum::{
    Json, Router,
    http::{
        HeaderValue, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    middleware::{self, from_fn_with_state},
    routing::{get, post},
};
use faucet::middlewares::{
    ACTON_CLIENT_HEADER, DEVICE_UID_HEADER, require_airdrop_headers, require_faucet_writable,
    require_pow_enabled,
};
use reqwest::Url;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;

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
            require_faucet_writable,
        ))
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
        .route("/openapi.json", get(openapi_handler))
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

async fn openapi_handler() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi())
}

fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "TON Testnet Faucet API",
        version = "0.1.0",
        description = "API for requesting testnet GRAM from the Acton faucet."
    ),
    paths(
        auth::status,
        auth::github_start,
        auth::github_callback,
        auth::exchange_grant,
        auth::get_session,
        auth::delete_session,
        challenge::create_challenge,
        claim::create_claim,
        stats::get_stats
    ),
    components(schemas(
        auth::GrantExchangeRequest,
        auth::AuthStatusResponse,
        auth::SessionResponse,
        auth::ErrorResponse,
        challenge::ChallengeRequest,
        challenge::ChallengeResponse,
        claim::CreateClaimRequest,
        claim::ClaimResponse,
        stats::StatsResponse,
        stats::AntifraudStatsResponse,
        crate::github_auth::FaucetTier
    )),
    tags(
        (name = "faucet", description = "Proof-of-work challenge and testnet GRAM claim endpoints"),
        (name = "authentication", description = "Optional GitHub authentication for higher faucet limits"),
        (name = "statistics", description = "Aggregate faucet usage statistics")
    )
)]
struct ApiDoc;

pub(crate) fn airdrop_cors_layer(frontend_url: Option<&str>) -> anyhow::Result<CorsLayer> {
    let mut origins = vec![
        HeaderValue::from_static("https://actonscan.com"),
        HeaderValue::from_static("http://localhost:3007"),
        HeaderValue::from_static("http://127.0.0.1:3007"),
    ];
    if let Some(frontend_url) = frontend_url {
        let frontend_url = Url::parse(frontend_url)
            .map_err(|error| anyhow::anyhow!("Invalid frontend URL: {error}"))?;
        let frontend_origin =
            HeaderValue::from_str(&frontend_url.origin().ascii_serialization())
                .map_err(|error| anyhow::anyhow!("Invalid frontend origin: {error}"))?;
        if !origins.contains(&frontend_origin) {
            origins.push(frontend_origin);
        }
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
    use super::{airdrop_cors_layer, openapi};
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

    #[test]
    fn openapi_json_documents_faucet_api() {
        let document = serde_json::to_value(openapi()).expect("OpenAPI document should serialize");

        assert_eq!(document["openapi"], "3.1.0");
        for path in [
            "/auth/status",
            "/auth/github/start",
            "/auth/github/callback",
            "/auth/exchange",
            "/auth/session",
            "/challenge",
            "/claim",
            "/stats",
        ] {
            assert!(
                document["paths"][path].is_object(),
                "OpenAPI document is missing {path}"
            );
        }
        for schema in [
            "AuthStatusResponse",
            "SessionResponse",
            "ChallengeRequest",
            "ChallengeResponse",
            "CreateClaimRequest",
            "ClaimResponse",
            "StatsResponse",
            "FaucetTier",
        ] {
            assert!(
                document["components"]["schemas"][schema].is_object(),
                "OpenAPI document is missing {schema}"
            );
        }
    }

    #[tokio::test]
    async fn actonscan_preflight_bypasses_inner_rate_limit() {
        let app = Router::new()
            .route("/challenge", post(|| async { "ok" }))
            .route_layer(middleware::from_fn(always_rate_limited))
            .layer(airdrop_cors_layer(None).expect("valid CORS configuration"));
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
            .layer(airdrop_cors_layer(None).expect("valid CORS configuration"));
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
                airdrop_cors_layer(Some("https://staging.example.com/faucet?network=testnet"))
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
