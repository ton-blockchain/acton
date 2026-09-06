//! Activity controls exercise the real authenticated router and persisted records.

use acton_localnet::{CreateNetwork, Runtime, activity::ActivityConfig, catalog, http};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Method, Request},
};
use expect_test::expect;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Notify;
use tower::ServiceExt;

async fn request(app: &Router, method: Method, path: &str, body: Value) -> (u16, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("Content-Type", "application/json")
                .header("Authorization", "Bearer activity-test")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1_000_000)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).expect("JSON response"),
    )
}

#[tokio::test]
async fn settings_are_validated_and_survive_restart_without_starting_a_network() {
    let root = tempfile::tempdir().expect("catalog");
    let location = catalog::create(
        root.path(),
        CreateNetwork {
            name: "activity-test".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    let runtime = Runtime::open(&location.path).await.expect("runtime");
    let app = http::router(
        runtime.clone(),
        "activity-test".to_owned(),
        Arc::new(Notify::new()),
    );
    let mut config = serde_json::to_value(ActivityConfig::default()).expect("config");
    config["intervalSeconds"] = json!(7);
    config["maxBatchSize"] = json!(24);
    config["randomizeBatchSize"] = json!(true);
    config["scenariosPerLaunch"] = json!(1000);
    config["concurrency"] = json!(1024);
    let saved = request(&app, Method::PUT, "/v1/network/activity", config.clone()).await;
    let mut invalid = config.clone();
    invalid["concurrency"] = json!(0);
    let rejected = request(&app, Method::PUT, "/v1/network/activity", invalid).await;
    let start = request(&app, Method::POST, "/v1/network/activity/start", config).await;
    let stop = request(&app, Method::POST, "/v1/network/activity/stop", Value::Null).await;
    runtime.shutdown().await.expect("shutdown");
    let after_shutdown = runtime
        .configure_activity(ActivityConfig::default(), false)
        .await
        .expect_err("shutdown rejects writes")
        .to_string();
    drop(app);
    drop(runtime);

    let reopened = Runtime::open(&location.path).await.expect("reopen");
    let state = reopened.activity().await.expect("persisted settings");
    let summary = json!({
        "saveStatus": saved.0,
        "invalid": rejected,
        "startWhileStopped": start,
        "stopStatus": stop.0,
        "afterShutdown": after_shutdown,
        "interval": state.config.interval_seconds,
        "maxBatchSize": state.config.max_batch_size,
        "randomizeBatchSize": state.config.randomize_batch_size,
        "scenariosPerLaunch": state.config.scenarios_per_launch,
        "concurrency": state.config.concurrency,
        "walletVersions": state.config.wallet_versions,
        "status": state.status,
        "hasRun": state.run_id.is_some(),
    });
    expect![[r#"
        {
          "afterShutdown": "The localnet service is stopping",
          "concurrency": 1024,
          "hasRun": false,
          "interval": 7,
          "invalid": [
            400,
            {
              "code": "invalid_request",
              "message": "Choose between 1 and 1024 concurrent scenarios"
            }
          ],
          "maxBatchSize": 24,
          "randomizeBatchSize": true,
          "saveStatus": 200,
          "scenariosPerLaunch": 1000,
          "startWhileStopped": [
            409,
            {
              "code": "network_not_running",
              "message": "Start the network before generating activity"
            }
          ],
          "status": "stopped",
          "stopStatus": 200,
          "walletVersions": [
            "v3r2",
            "v4r2",
            "v5r1"
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&summary).expect("snapshot"));
}

#[tokio::test]
async fn opening_an_interrupted_run_preserves_counters_and_never_replays_messages() {
    let root = tempfile::tempdir().expect("catalog");
    let location = catalog::create(
        root.path(),
        CreateNetwork {
            name: "activity-test".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("network");
    let path = location.path.join("activity.json");
    let mut state =
        serde_json::to_value(acton_localnet::activity::ActivityState::default()).expect("state");
    state["status"] = json!("running");
    state["runId"] = json!("interrupted-run");
    state["active"] = json!(2);
    state["completed"] = json!(5);
    state["confirmedMessages"] = json!(42);
    std::fs::write(&path, state.to_string()).expect("interrupted state");

    let runtime = Runtime::open(&location.path).await.expect("runtime");
    let state = runtime.activity().await.expect("activity");
    let saved: Value = serde_json::from_slice(&std::fs::read(path).expect("saved")).expect("JSON");
    expect![[r#"
        {
          "active": 0,
          "completed": 5,
          "confirmedMessages": 42,
          "finished": true,
          "persistedStatus": "interrupted",
          "status": "interrupted"
        }"#]]
    .assert_eq(
        &serde_json::to_string_pretty(&json!({
            "status": state.status,
            "persistedStatus": saved["status"],
            "active": state.active,
            "completed": state.completed,
            "confirmedMessages": state.confirmed_messages,
            "finished": state.finished_at.is_some(),
        }))
        .expect("snapshot"),
    );
}

#[test]
fn unsafe_or_empty_workloads_are_rejected() {
    let baseline = serde_json::to_value(ActivityConfig::default()).expect("config");
    let cases = [
        ("intervalSeconds", json!(0)),
        ("intervalSeconds", json!(3601)),
        ("scenariosPerLaunch", json!(0)),
        ("scenariosPerLaunch", json!(1001)),
        ("concurrency", json!(1025)),
        ("durationSeconds", json!(86401)),
        ("maxBatchSize", json!(129)),
        ("transferAmount", json!(0)),
        ("walletVersions", json!([])),
        ("walletVersions", json!(["v4r2", "v4r2"])),
        (
            "scenarios",
            json!({"transfers":0,"batches":0,"jettons":0,"nfts":0}),
        ),
        (
            "scenarios",
            json!({"transfers":101,"batches":0,"jettons":0,"nfts":0}),
        ),
    ];
    let messages: Vec<_> = cases
        .into_iter()
        .map(|(field, value)| {
            let mut config = baseline.clone();
            config[field] = value;
            serde_json::from_value::<ActivityConfig>(config)
                .expect("config")
                .validate()
                .expect_err("invalid workload")
                .to_string()
        })
        .collect();
    expect![[r#"
        [
          "Choose a scenario interval between 1 and 3600 seconds",
          "Choose a scenario interval between 1 and 3600 seconds",
          "Choose between 1 and 1000 scenarios per launch",
          "Choose between 1 and 1000 scenarios per launch",
          "Choose between 1 and 1024 concurrent scenarios",
          "Choose a duration up to 24 hours, or 0 to run until stopped",
          "Choose between 2 and 128 transfers per batch",
          "Choose a transfer amount between 0.001 and 1000 GRAM",
          "Select at least one wallet version",
          "Select each wallet version only once",
          "Enable at least one activity scenario",
          "Scenario weights must be between 0 and 100"
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&messages).expect("snapshot"));
}
