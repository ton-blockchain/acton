use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header::USER_AGENT},
    middleware,
    routing::post,
};
use faucet_backend::middlewares::require_airdrop_headers;
use tower::ServiceExt;

#[tokio::test]
async fn requires_airdrop_headers_on_protected_route() {
    let response = request_with_headers(Some("acton/0.1.0"), Some("default")).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_body(response).await, "ok");

    let response = request_with_headers(
        Some("acton/0.1.0"),
        Some("87c4bc1848a84471997203ee530d2fda"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = request_with_headers(
        Some("acton/0.1.0"),
        Some("550e8400-e29b-41d4-a716-446655440000"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = request_with_headers(
        Some("acton/0.1.0"),
        Some("550E8400-E29B-41D4-A716-446655440000"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let response = request_with_headers(None, Some("default")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("acton/"), Some("default")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("faucet/0.1.0"), Some("default")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("acton/0.1.0"), None).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("acton/0.1.0"), Some("")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("acton/0.1.0"), Some(" ")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("acton/0.1.0"), Some("device-1")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = request_with_headers(Some("acton/0.1.0"), Some("another")).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

async fn request_with_headers(
    user_agent: Option<&str>,
    device_uid: Option<&str>,
) -> axum::response::Response {
    let app = Router::new()
        .route("/challenge", post(|| async { "ok" }))
        .route_layer(middleware::from_fn(require_airdrop_headers));

    let mut request = Request::builder().method(Method::POST).uri("/challenge");

    if let Some(user_agent) = user_agent {
        request = request.header(USER_AGENT, user_agent);
    }
    if let Some(device_uid) = device_uid {
        request = request.header("x-device-uid", device_uid);
    }

    app.oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn response_body(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}
