use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    middleware::from_fn_with_state,
    routing::post,
};
use faucet::middlewares::require_faucet_writable;
use std::sync::Arc;
use tower::ServiceExt;

use super::pow::config;

#[tokio::test]
async fn writable_faucet_allows_airdrop_routes() {
    for path in ["/challenge", "/claim"] {
        let response = request(path, false).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_body(response).await, "ok");
    }
}

#[tokio::test]
async fn read_only_faucet_rejects_airdrop_routes() {
    for path in ["/challenge", "/claim"] {
        let response = request(path, true).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response_body(response).await,
            r#"{"error":"Faucet is in read-only mode"}"#
        );
    }
}

async fn request(path: &'static str, read_only: bool) -> axum::response::Response {
    let mut config = config(true);
    config.faucet.read_only = read_only;
    let app = Router::new()
        .route("/challenge", post(|| async { "ok" }))
        .route("/claim", post(|| async { "ok" }))
        .route_layer(from_fn_with_state(
            Arc::new(config),
            require_faucet_writable,
        ));

    app.oneshot(
        Request::builder()
            .method("POST")
            .uri(path)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn response_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}
