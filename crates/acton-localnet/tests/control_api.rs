//! HTTP contract snapshots exercise the real router and durable runtime together.

use acton_localnet::{CreateNetwork, Runtime, catalog, http};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request},
};
use expect_test::expect;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;
use tower::ServiceExt;

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    body: Value,
    token: Option<&str>,
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("Content-Type", "application/json");

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }

    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).expect("request"))
        .await
        .expect("response");
    let status = response.status().as_u16();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");

    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn authenticated_api_is_scoped_to_one_network() {
    let root = tempfile::tempdir().expect("state directory");
    let first = catalog::create(
        root.path(),
        CreateNetwork {
            name: "first".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("first network");
    let second = catalog::create(
        root.path(),
        CreateNetwork {
            name: "second".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("second network");
    let runtime = Runtime::open(&first.path).await.expect("runtime");
    let app = http::router(
        runtime.clone(),
        "secret".to_owned(),
        Arc::new(Notify::new()),
    );
    let unauthorized = request(&app, Method::GET, "/v1/network", Value::Null, None).await;
    let network = request(
        &app,
        Method::GET,
        "/v1/network",
        Value::Null,
        Some("secret"),
    )
    .await;
    let health = request(&app, Method::GET, "/v1/health", Value::Null, Some("secret")).await;
    let catalog_route = request(
        &app,
        Method::GET,
        "/v1/networks",
        Value::Null,
        Some("secret"),
    )
    .await;
    let create_route = request(
        &app,
        Method::POST,
        "/v1/networks",
        json!({"name":"third"}),
        Some("secret"),
    )
    .await;
    let other_network = request(
        &app,
        Method::POST,
        &format!("/v1/networks/{}/start", second.network.id),
        Value::Null,
        Some("secret"),
    )
    .await;
    let snapshots = request(
        &app,
        Method::GET,
        "/v1/network/snapshots",
        Value::Null,
        Some("secret"),
    )
    .await;
    expect![[r#"
        {
          "catalog": 404,
          "create": 404,
          "health": 200,
          "name": "first",
          "network": 200,
          "otherNetwork": 404,
          "snapshots": [],
          "unauthorized": 401
        }"#]]
    .assert_eq(
        &serde_json::to_string_pretty(&json!({
            "unauthorized": unauthorized.0, "network": network.0, "name": network.1["name"],
            "health": health.0, "catalog": catalog_route.0, "create": create_route.0,
            "otherNetwork": other_network.0, "snapshots": snapshots.1,
        }))
        .expect("API snapshot"),
    );

    let second_runtime = Runtime::open(&second.path)
        .await
        .expect("independent ownership");
    let duplicate = Runtime::open(&first.path)
        .await
        .err()
        .expect("exclusive ownership");
    expect![["service_already_running"]].assert_eq(match duplicate {
        acton_localnet::Error::Conflict { code, .. } => code,
        _ => "unexpected error",
    });
    runtime.shutdown().await.expect("first shutdown");
    expect![["second"]].assert_eq(&second_runtime.get().await.name);
    second_runtime.shutdown().await.expect("second shutdown");
}

#[tokio::test]
async fn accepted_operations_survive_client_disconnect_and_service_shutdown_rejects_writes() {
    let root = tempfile::tempdir().expect("state directory");
    let location = catalog::create(
        root.path(),
        CreateNetwork {
            name: "bad-image".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    let runtime = Runtime::open(&location.path).await.expect("runtime");

    // Pin a malformed image to fail before invoking Docker. This exercises the
    // actual asynchronous operation path without depending on a Docker daemon.
    let descriptor = json!({"version":2, "image":"bad image", "dockerTarget":{"kind":"context","value":"test"}, "projectName":"acton-localnet-test"});
    std::fs::write(location.path.join("runtime.json"), descriptor.to_string()).expect("descriptor");
    let app = http::router(
        runtime.clone(),
        "secret".to_owned(),
        Arc::new(Notify::new()),
    );
    let (status, accepted) = request(
        &app,
        Method::POST,
        "/v1/network/start",
        Value::Null,
        Some("secret"),
    )
    .await;
    let operation_id = accepted["id"].as_str().expect("operation id");

    drop(app);
    let operation = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let operation = runtime
                .operation(operation_id)
                .await
                .expect("durable operation");
            if operation.status != acton_localnet::OperationStatus::Running {
                break operation;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("operation finished");

    expect![["202:start:Failed:failed"]].assert_eq(&format!(
        "{status}:{}:{:?}:{}",
        operation.kind, operation.status, operation.phase
    ));
    expect![["true:true"]].assert_eq(&format!(
        "{}:{}",
        operation
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("valid container image"),
        operation
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Full log:")
    ));

    runtime
        .prepare_shutdown()
        .await
        .expect("stop accepting mutations");
    let app = http::router(
        runtime.clone(),
        "secret".to_owned(),
        Arc::new(Notify::new()),
    );
    let stopped = request(
        &app,
        Method::POST,
        "/v1/network/start",
        Value::Null,
        Some("secret"),
    )
    .await;
    expect![[r#"
        [
          409,
          {
            "code": "service_stopping",
            "message": "The localnet service is stopping"
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&stopped).expect("snapshot"));
}

#[tokio::test]
async fn health_compares_live_v2_and_v3_heads_and_retains_recent_samples() {
    let root = tempfile::tempdir().expect("state directory");
    let location = catalog::create(
        root.path(),
        CreateNetwork {
            name: "observed".to_owned(),
            block_time_ms: Some(400),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    let ports = location.network.config.ports();
    let block_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs()
        .saturating_sub(1);
    let v2 = Router::new()
        .route(
            "/api/v2/getMasterchainInfo",
            axum::routing::get(|| async {
                axum::Json(json!({"ok": true, "result": {"last": {"seqno": 42}}}))
            }),
        )
        .route(
            "/api/v2/getBlockHeader",
            axum::routing::get(move || async move {
                axum::Json(json!({"ok": true, "result": {"gen_utime": block_time}}))
            }),
        );
    let v3 = Router::new()
        .route("/healthcheck", axum::routing::get(|| async { "OK" }))
        .route(
            "/api/v3/masterchainInfo",
            axum::routing::get(|| async { axum::Json(json!({"last": {"seqno": 39}})) }),
        );
    let v2_listener = tokio::net::TcpListener::bind(("127.0.0.1", ports.api_v2))
        .await
        .expect("v2 listener");
    let v3_listener = tokio::net::TcpListener::bind(("127.0.0.1", ports.api_v3))
        .await
        .expect("v3 listener");
    let v2_server = tokio::spawn(async move { axum::serve(v2_listener, v2).await });
    let v3_server = tokio::spawn(async move { axum::serve(v3_listener, v3).await });
    let runtime = Runtime::open(&location.path).await.expect("runtime");
    let app = http::router(
        runtime.clone(),
        "secret".to_owned(),
        Arc::new(Notify::new()),
    );

    let first = request(
        &app,
        Method::GET,
        "/v1/network/health",
        Value::Null,
        Some("secret"),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(1050)).await;
    let second = request(
        &app,
        Method::GET,
        "/v1/network/health",
        Value::Null,
        Some("secret"),
    )
    .await;
    let summary = json!({
        "statusCode": second.0,
        "status": second.1["status"],
        "apiV2": second.1["apiV2"]["status"],
        "apiV3": second.1["apiV3"]["status"],
        "v2Seqno": second.1["apiV2"]["masterchainSeqno"],
        "v3Seqno": second.1["apiV3"]["masterchainSeqno"],
        "lagBlocks": second.1["indexerLagBlocks"],
        "estimatedLagMs": second.1["estimatedIndexerLagMs"],
        "historyPoints": second.1["history"].as_array().map(Vec::len),
        "latencyRecorded": second.1["apiV2"]["latencyMs"].is_number()
            && second.1["apiV3"]["latencyMs"].is_number(),
        "blockAgeRecorded": second.1["apiV2"]["blockAgeMs"].is_number(),
        "firstHistoryPoints": first.1["history"].as_array().map(Vec::len),
    });

    expect![[r#"
        {
          "apiV2": "ready",
          "apiV3": "syncing",
          "blockAgeRecorded": true,
          "estimatedLagMs": 1200,
          "firstHistoryPoints": 1,
          "historyPoints": 2,
          "lagBlocks": 3,
          "latencyRecorded": true,
          "status": "syncing",
          "statusCode": 200,
          "v2Seqno": 42,
          "v3Seqno": 39
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&summary).expect("health snapshot"));

    v2_server.abort();
    v3_server.abort();
    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn admin_api_validates_before_touching_a_stopped_network() {
    let root = tempfile::tempdir().expect("state");
    let location = catalog::create(
        root.path(),
        CreateNetwork {
            name: "admin-api".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    let runtime = Runtime::open(&location.path).await.expect("runtime");
    let app = http::router(runtime, "secret".to_owned(), Arc::new(Notify::new()));
    let edit = json!({"kind":"accounts", "id":uuid::Uuid::new_v4().to_string(),
        "edits":[{"address":format!("0:{}", "11".repeat(32)), "type":"balance", "balance":"1"}]});
    let unauthorized = request(&app, Method::POST, "/v1/network/admin", edit.clone(), None).await;
    assert_eq!(unauthorized.0, 401);
    let stopped = request(
        &app,
        Method::POST,
        "/v1/network/admin",
        edit,
        Some("secret"),
    )
    .await;
    assert_eq!(stopped.0, 409);
    assert_eq!(stopped.1["code"], "admin_unavailable");
    assert!(!location.path.join("runtime.json").exists());
    let current = request(
        &app,
        Method::GET,
        "/v1/network/admin",
        Value::Null,
        Some("secret"),
    )
    .await;
    assert_eq!(current, (200, Value::Null));
}
