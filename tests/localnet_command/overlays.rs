//! Recovery exercises the real owner and CLI with a persisted fake engine configuration.

use super::{Service, acton, api_listener, cli};
use acton_localnet::{Network, OperationStatus, Status};
use serde_json::{Value, json};
use std::process::Stdio;
use tokio::process::Command;

#[tokio::test]
async fn interrupted_overlay_update_requires_start_and_reconciles_saved_configuration() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let v2 = api_listener(service.network.network.config.ports().api_v2).await;
    let v3 = api_listener(service.network.network.config.ports().api_v3).await;
    cli(&service.state(), &["start", "integration"]).await;
    cli(&service.state(), &["node", "integration", "add", "peer"]).await;
    let observability = tokio::net::TcpListener::bind((
        "127.0.0.1",
        service.network.network.config.ports().observability,
    ))
    .await
    .expect("mock collector listener");
    let collector = tokio::spawn(async move {
        let app = axum::Router::new().fallback(|| async {
            axum::Json(json!({"nodes": [{
                "name": "peer", "online": true, "running": true,
                "head_seqno": 10, "head_observed_at": u64::MAX
            }]}))
        });
        axum::serve(observability, app)
            .await
            .expect("mock collector");
    });

    assert_eq!(
        cli(&service.state(), &["overlays", "integration"]).await,
        json!({"overlays": []})
    );
    let malformed_path = service.root.path().join("bad-overlays.json");
    std::fs::write(&malformed_path, r#"{"overlay": []}"#).expect("malformed config");
    let before = client
        .network()
        .await
        .expect("network before malformed config");
    let output = Command::from(acton(
        service.root.path(),
        &[
            "overlays",
            "integration",
            "--config",
            malformed_path.to_str().expect("fixture path"),
        ],
    ))
    .output()
    .await
    .expect("invalid overlay CLI output");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid overlay configuration"));
    let mut record = client.network().await.expect("unchanged operation");
    assert_eq!(
        record.operation.as_ref().map(|operation| &operation.id),
        before.operation.as_ref().map(|operation| &operation.id)
    );

    // The owner dies after an engine changes, before committing desired state.
    // Containers stay alive, and the persisted desired configuration is empty.
    service
        .child
        .kill()
        .await
        .expect("kill owner without cleanup");
    service.child.wait().await.expect("reap killed owner");
    assert!(service.network.path.join("fixture-running").exists());
    let mut interrupted = record.operation.clone().expect("operation to interrupt");
    interrupted.id = "overlay-interrupted".to_owned();
    interrupted.kind = "configureOverlays".to_owned();
    interrupted.phase = "configuringOverlays".to_owned();
    interrupted.status = OperationStatus::Running;
    interrupted.result = None;
    interrupted.completed_steps.clear();
    record.operation = Some(interrupted);
    std::fs::write(
        service.network.path.join("network.json"),
        serde_json::to_vec(&record).expect("interrupted record"),
    )
    .expect("persist interrupted operation");
    std::fs::write(
        service.network.path.join("fixture-overlays-localton.json"),
        serde_json::to_vec(&json!({"overlays": [{
            "@type": "engine.validator.customOverlay",
            "name": "uncommitted",
            "nodes": [{
                "@type": "engine.validator.customOverlayNode",
                "adnl_id": "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
                "msg_sender": true,
                "msg_sender_priority": 0,
                "block_sender": true
            }, {
                "@type": "engine.validator.customOverlayNode",
                "adnl_id": "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=",
                "msg_sender": true,
                "msg_sender_priority": 0,
                "block_sender": true
            }]
        }]}))
        .expect("partial engine state"),
    )
    .expect("engine state file");
    std::fs::write(
        service.network.path.join("fixture-overlays-node-1.json"),
        r#"{"overlays": []}"#,
    )
    .expect("unchanged peer engine state");
    let bin = service.root.path().join("bin");
    std::fs::copy(bin.join("docker"), bin.join("docker-base.py"))
        .expect("preserve base Docker fixture");
    std::fs::write(
        bin.join("docker"),
        include_str!("../fixtures/localnet/docker_overlays.py"),
    )
    .expect("install overlay Docker fixture");

    let log = std::fs::OpenOptions::new()
        .append(true)
        .open(service.root.path().join("service.log"))
        .expect("service log");
    service.child = Command::from(acton(service.root.path(), &["serve", "integration"]))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().expect("clone log")))
        .stderr(Stdio::from(log))
        .kill_on_drop(true)
        .spawn()
        .expect("restart owner");
    let client = service.client().await;
    let interrupted = client.network().await.expect("network requiring recovery");
    assert_eq!(interrupted.status, Status::Failed);
    assert!(
        interrupted
            .error
            .as_deref()
            .unwrap()
            .contains("start the network")
    );
    assert_eq!(
        interrupted
            .operation
            .as_ref()
            .unwrap()
            .error_code
            .as_deref(),
        Some("operation_interrupted")
    );
    assert!(service.network.path.join("overlays-recovery").exists());
    assert_eq!(
        cli(&service.state(), &["overlays", "integration"]).await,
        json!({"overlays": []})
    );
    assert!(!service.root.path().join("overlay-events").exists());

    // `start` must submit a real reconciliation, even though Docker is running.
    let started = cli(&service.state(), &["start", "integration"]).await;
    assert_eq!(started["status"], "running");
    assert_eq!(started["operation"]["kind"], "start");
    assert_eq!(started["operation"]["status"], "completed");
    assert!(started["error"].is_null());
    assert!(!service.network.path.join("overlays-recovery").exists());
    let actual: Value = serde_json::from_slice(
        &std::fs::read(service.network.path.join("fixture-overlays-localton.json"))
            .expect("reconciled engine config"),
    )
    .expect("engine JSON");
    assert_eq!(actual, json!({"overlays": []}));
    assert_eq!(
        std::fs::read_to_string(service.root.path().join("overlay-events"))
            .expect("overlay mutation log"),
        "del-custom-overlay uncommitted\n"
    );

    let config_path = service.root.path().join("overlays.json");
    std::fs::write(&config_path, r#"{"overlays": []}"#).expect("valid overlay config");
    let configured = cli(
        &service.state(),
        &[
            "overlays",
            "integration",
            "--config",
            config_path.to_str().expect("fixture path"),
        ],
    )
    .await;
    assert_eq!(configured["kind"], "configureOverlays");
    assert_eq!(configured["status"], "completed");
    assert_eq!(configured["result"], json!({"overlays": []}));

    service.stop(&client).await;
    let saved: Network = serde_json::from_slice(
        &std::fs::read(service.network.path.join("network.json")).expect("saved network"),
    )
    .expect("network JSON");
    assert!(saved.overlay_config.is_empty());
    v2.abort();
    v3.abort();
    collector.abort();
    let _ = tokio::join!(v2, v3, collector);
}
