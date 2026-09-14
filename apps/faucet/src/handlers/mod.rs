use axum::{
    Json, Router,
    middleware::{self, from_fn_with_state},
    routing::{get, post},
};
use faucet::middlewares::{require_airdrop_headers, require_faucet_writable, require_pow_enabled};
use utoipa::OpenApi;

use crate::AppState;

mod auth;
mod challenge;
mod claim;
mod health;
mod info;
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
        .route("/", get(info::root))
        .route("/openapi.json", get(openapi_handler))
        .route("/robots.txt", get(robots::robots_txt))
        .route("/readyz", get(info::ok))
        .route("/healthz", get(health::health))
        .route("/metrics", get(info::ok))
        .route("/stats", get(stats::get_stats))
        .route("/version", get(info::version))
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

#[cfg(test)]
mod tests {
    use super::openapi;

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
}
