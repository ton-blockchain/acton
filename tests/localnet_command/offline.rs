//! Offline reads must not start a service, rewrite state, or mutate Docker resources.

use super::{Service, acton, api_listener, cli};
use acton_localnet::{Network, Operation, OperationStatus, Status};
use expect_test::expect;
use serde_json::json;
use std::time::Duration;
use tokio::process::Command;

#[tokio::test]
async fn stopped_service_keeps_logs_operations_and_observed_status_available() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let v2 = api_listener(service.network.network.config.port_base + 2).await;
    let v3 = api_listener(service.network.network.config.port_base + 3).await;
    let started = cli(&service.state(), &["start", "integration"]).await;
    let operation_id = started["operation"]["id"]
        .as_str()
        .expect("start operation");
    service.stop(&client).await;

    let directory = &service.network.path;
    let events = std::fs::read(service.root.path().join("network-events")).expect("Docker events");
    let compose = std::fs::read(directory.join("compose.yaml")).expect("Compose file");
    std::fs::write(
        directory.join("startup.log"),
        "first line\nsecond line\nlast line\n",
    )
    .expect("saved log");
    let logs = cli(&service.state(), &["logs", "--tail", "2"]).await;
    let operation = cli(&service.state(), &["operation", operation_id]).await;
    let waited = cli(&service.state(), &["operation", operation_id, "--wait"]).await;

    // Docker remains authoritative even when a service crashed after persisting
    // an older state. Both directions must be observed without writing the record.
    let record_path = directory.join("network.json");
    let mut record: Network =
        serde_json::from_slice(&std::fs::read(&record_path).expect("record")).expect("network");
    record.status = Status::Running;
    std::fs::write(
        &record_path,
        serde_json::to_vec(&record).expect("stale record"),
    )
    .expect("record file");
    let stopped = cli(&service.state(), &["status"]).await;
    record.status = Status::Stopped;
    std::fs::write(
        &record_path,
        serde_json::to_vec(&record).expect("stale record"),
    )
    .expect("record file");
    std::fs::write(directory.join("fixture-running"), "").expect("containers running");
    let running = cli(&service.state(), &["status"]).await;
    std::fs::write(service.root.path().join("docker-unavailable"), "").expect("daemon unavailable");
    let unknown = cli(&service.state(), &["status"]).await;
    let saved = std::fs::read(&record_path).expect("unchanged definition");
    expect![[r#"
        {
          "daemonError": true,
          "logs": "second line\nlast line",
          "operation": "completed",
          "running": "running",
          "stopped": "stopped",
          "unknown": "unknown",
          "waited": "completed"
        }"#]].assert_eq(&serde_json::to_string_pretty(&json!({
            "logs": logs["logs"], "operation": operation["status"], "waited": waited["status"],
            "stopped": stopped["status"], "running": running["status"], "unknown": unknown["status"],
            "daemonError": unknown["error"].as_str().expect("inspection error").contains("Docker daemon is unavailable"),
        })).expect("offline results"));

    let operation_path = directory
        .join("operations")
        .join(format!("{operation_id}.json"));
    let mut interrupted: Operation =
        serde_json::from_slice(&std::fs::read(&operation_path).expect("operation"))
            .expect("saved operation");
    interrupted.status = OperationStatus::Running;
    std::fs::write(
        &operation_path,
        serde_json::to_vec(&interrupted).expect("interrupted operation"),
    )
    .expect("operation file");
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::from(acton(
            service.root.path(),
            &["operation", operation_id, "--wait", "--json"],
        ))
        .output(),
    )
    .await
    .expect("offline wait must finish")
    .expect("CLI output");
    expect![["false:true"]].assert_eq(&format!(
        "{}:{}",
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).contains("its service is unavailable")
    ));

    interrupted.status = OperationStatus::Failed;
    interrupted.error = Some("Saved operation failed".to_owned());
    std::fs::write(
        &operation_path,
        serde_json::to_vec(&interrupted).expect("failed operation"),
    )
    .expect("operation file");
    let failed = Command::from(acton(
        service.root.path(),
        &["operation", operation_id, "--wait", "--json"],
    ))
    .output()
    .await
    .expect("failed operation output");
    expect![["false:true"]].assert_eq(&format!(
        "{}:{}",
        failed.status.success(),
        String::from_utf8_lossy(&failed.stderr).contains("Saved operation failed")
    ));

    expect![["true:true:true:true"]].assert_eq(&format!(
        "{}:{}:{}:{}",
        !directory.join("service.json").exists(),
        events
            == std::fs::read(service.root.path().join("network-events"))
                .expect("no Docker mutations"),
        compose == std::fs::read(directory.join("compose.yaml")).expect("unchanged Compose"),
        saved == serde_json::to_vec(&record).expect("unchanged network"),
    ));
    v2.abort();
    v3.abort();

    let _ = tokio::join!(v2, v3);
    drop(service);
}
