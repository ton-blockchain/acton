//! Config recovery uses the real control service; only Localton's admin API is mocked.

use super::{Service, api_listener, cli};
use acton_localnet::{Network, Operation, OperationStatus, UpdateNetworkConfig, client::Client};
use axum::{Json, Router, http::StatusCode, routing::post};
use expect_test::expect;
use reqwest::Method;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;

async fn completed(client: &Client, id: &str) -> Operation {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let operation = client.operation(id).await.expect("config operation");
            if operation.status != OperationStatus::Running {
                return operation;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("config operation deadline")
}

#[tokio::test]
async fn successful_config_update_clears_environment_error_and_preserves_failed_operation() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let ports = service.network.network.config.ports();
    let v2 = api_listener(ports.api_v2).await;
    let v3 = api_listener(ports.api_v3).await;
    let confirming = Arc::new(Notify::new());
    let confirmed = Arc::new(Notify::new());
    let admin = Router::new().route(
        "/v1/network/config",
        post({
            let confirming = Arc::clone(&confirming);
            let confirmed = Arc::clone(&confirmed);
            move |Json(request): Json<UpdateNetworkConfig>| {
                let confirming = Arc::clone(&confirming);
                let confirmed = Arc::clone(&confirmed);
                async move {
                    if request.expected_hash.is_none() {
                        return (
                            StatusCode::CONFLICT,
                            Json(json!({"error": "Parameter changed since it was loaded"})),
                        );
                    }

                    confirming.notify_one();
                    confirmed.notified().await;
                    (
                        StatusCode::OK,
                        Json(json!({"index": request.index, "masterchainSeqno": 42})),
                    )
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", ports.admin))
        .await
        .expect("Localton admin listener");
    let admin = tokio::spawn(async move { axum::serve(listener, admin).await });
    cli(&service.state(), &["start", "integration"]).await;

    let mut request = UpdateNetworkConfig {
        index: 4,
        boc: "te6ccgEBAQEAAgAAAA==".to_owned(),
        expected_hash: None,
    };
    let rejected = client
        .update_network_config(&request)
        .await
        .expect("failed update accepted for processing");
    let failed = completed(&client, &rejected.id).await;
    let failure = failed.error.as_ref().expect("stale config diagnosis");
    let before: Network = client
        .request(Method::GET, "/v1/network", None)
        .await
        .expect("network after failed update");

    request.expected_hash = Some("42".repeat(32));
    let accepted = client
        .update_network_config(&request)
        .await
        .expect("retry accepted");
    tokio::time::timeout(Duration::from_secs(10), confirming.notified())
        .await
        .expect("retry awaiting masterchain confirmation");
    let during: Network = client
        .request(Method::GET, "/v1/network", None)
        .await
        .expect("network while confirming");
    confirmed.notify_one();
    let succeeded = completed(&client, &accepted.id).await;
    let after: Network = client
        .request(Method::GET, "/v1/network", None)
        .await
        .expect("network after confirmed retry");
    let historical = client
        .operation(&failed.id)
        .await
        .expect("failed operation remains available");

    // Graceful shutdown waits for the operation's final durable write.
    service.stop(&client).await;
    let saved: Network = serde_json::from_slice(
        &std::fs::read(service.network.path.join("network.json")).expect("saved network"),
    )
    .expect("persisted network state");
    let outcome: Value = json!({
        "failed": failed.status,
        "errorBefore": before.error.as_ref() == Some(failure),
        "errorWhileConfirming": during.error.as_ref() == Some(failure),
        "succeeded": succeeded.status,
        "networkAfter": after.status,
        "errorAfter": after.error,
        "persistedError": saved.error,
        "historicalStatus": historical.status,
        "historicalErrorPreserved": historical.error.as_ref() == Some(failure),
    });

    v2.abort();
    v3.abort();
    admin.abort();
    let _ = tokio::join!(v2, v3, admin);
    drop(service);

    expect![[r#"
        {
          "errorAfter": null,
          "errorBefore": true,
          "errorWhileConfirming": true,
          "failed": "failed",
          "historicalErrorPreserved": true,
          "historicalStatus": "failed",
          "networkAfter": "running",
          "persistedError": null,
          "succeeded": "completed"
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&outcome).expect("config recovery snapshot"));
}
