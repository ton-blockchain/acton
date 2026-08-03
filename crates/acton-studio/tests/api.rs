use acton_studio::{
    STUDIO_HEALTH_PATH, STUDIO_INFO_PATH, StudioServer, StudioServerConfig, StudioWorkspace,
};
use axum::body::to_bytes;
use axum::http::Request;
use expect_test::expect;
use tower::ServiceExt;

fn server() -> StudioServer {
    StudioServer::new(
        StudioServerConfig::new("test-version").with_workspace(
            StudioWorkspace::new("counter", "/private/workspaces/counter")
                .with_wallet_names(vec!["deployer".to_owned(), "treasury".to_owned()]),
        ),
    )
}

#[tokio::test]
async fn info_contract_does_not_expose_host_paths_or_deployment_mode() {
    let response = server()
        .router()
        .oneshot(
            Request::get(STUDIO_INFO_PATH)
                .body(axum::body::Body::empty())
                .expect("info request must be valid"),
        )
        .await
        .expect("info request must succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("info body must be readable");
    let actual = format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body));

    expect![[r#"status: 200 OK
body: {"protocolVersion":1,"serverVersion":"test-version","workspace":{"name":"counter","walletNames":["deployer","treasury"]}}"#]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn health_contract_is_minimal() {
    let response = server()
        .router()
        .oneshot(
            Request::get(STUDIO_HEALTH_PATH)
                .body(axum::body::Body::empty())
                .expect("health request must be valid"),
        )
        .await
        .expect("health request must succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("health body must be readable");
    let actual = format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body));

    expect![[r"status: 204 No Content
body: "]]
    .assert_eq(&actual);
}

#[tokio::test]
async fn unknown_api_routes_never_fall_back_to_the_studio_ui() {
    let response = server()
        .router()
        .oneshot(
            Request::get("/api/v2/unknown")
                .body(axum::body::Body::empty())
                .expect("unknown API request must be valid"),
        )
        .await
        .expect("unknown API request must succeed");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("unknown API body must be readable");
    let actual = format!("status: {status}\nbody: {}", String::from_utf8_lossy(&body));

    expect![[r"status: 404 Not Found
body: "]]
    .assert_eq(&actual);
}
