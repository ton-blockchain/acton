use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode},
    middleware::from_fn_with_state,
    routing::post,
};
use faucet_backend::middlewares::require_pow_enabled;
use faucet_config::{
    AntifraudConfig, ClaimRateLimitConfig, Config, DatabaseConfig, DefaultRateLimitConfig,
    FaucetConfig, PowClientConfig, PowConfig, RateLimitConfig, SentAmountWindowCheckConfig,
    ServerConfig, SuccessfulClaimWindowCheckConfig, ToncenterConfig, ValkeyConfig,
    WalletBalanceCheckConfig, WorkerConfig,
};
use std::sync::Arc;
use tower::ServiceExt;

#[tokio::test]
async fn pow_enabled_allows_protected_routes() {
    for (method, path) in [(Method::POST, "/challenge"), (Method::POST, "/claim")] {
        let response = request_with_pow(method, path, true).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_body(response).await, "ok");
    }
}

#[tokio::test]
async fn pow_disabled_rejects_protected_routes() {
    for (method, path) in [(Method::POST, "/challenge"), (Method::POST, "/claim")] {
        let response = request_with_pow(method, path, false).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response_body(response).await,
            r#"{"error":"PoW is disabled"}"#
        );
    }
}

async fn request_with_pow(
    method: Method,
    path: &'static str,
    pow_enabled: bool,
) -> axum::response::Response {
    let app = Router::new()
        .route("/challenge", post(|| async { "ok" }))
        .route("/claim", post(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            Arc::new(config(pow_enabled)),
            require_pow_enabled,
        ));

    app.oneshot(
        Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

fn config(pow_enabled: bool) -> Config {
    Config {
        database: DatabaseConfig {
            url: "sqlite::memory:".to_string(),
        },
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 3001,
        },
        rate_limit: RateLimitConfig {
            default: DefaultRateLimitConfig {
                window_seconds: 1,
                max_requests: 5,
            },
            claim: ClaimRateLimitConfig {
                window_seconds: 86_400,
                max_requests: 100,
            },
        },
        toncenter: ToncenterConfig {
            api_key: None,
            url: "https://testnet.toncenter.com".to_string(),
            timeout_seconds: 10,
            connect_timeout_seconds: 5,
            max_retries: 3,
            retry_base_delay_ms: 500,
        },
        worker: WorkerConfig {
            max_retries: 2,
            retry_base_delay_ms: 1_000,
        },
        faucet: FaucetConfig {
            mnemonic: "unused".to_string(),
            amount: 1_000_000,
            message: "Testnet faucet".to_string(),
        },
        pow: PowConfig {
            enabled: pow_enabled,
            difficulty: 21,
            challenge_ttl_seconds: 300,
            max_challenges: 10_000,
            client: PowClientConfig {
                max_solve_ttl_seconds: 300,
                max_nonce_attempts: 1_000_000_000,
            },
        },
        valkey: ValkeyConfig {
            uri: "redis://127.0.0.1:6379".to_string(),
        },
        antifraud: AntifraudConfig {
            enabled: true,
            wallet_balance: WalletBalanceCheckConfig {
                enabled: true,
                max_wallet_balance: 25_000_000_000,
            },
            sent_amount_window: SentAmountWindowCheckConfig {
                enabled: true,
                max_amount: 10_000_000_000,
                window_seconds: 60,
            },
            successful_claim_window: SuccessfulClaimWindowCheckConfig {
                enabled: true,
                max_requests: 2,
                window_seconds: 86_400,
            },
        },
    }
}

async fn response_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}
