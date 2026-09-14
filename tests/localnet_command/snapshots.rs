//! Snapshot commands discover services automatically and reap only their own children.

use super::{Service, acton, api_listener, cli};
use expect_test::expect;
use serde_json::json;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

#[tokio::test]
async fn stopped_snapshots_manage_their_service_and_cleanup_after_errors() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    let v2 = api_listener(service.network.network.config.port_base + 2).await;
    let v3 = api_listener(service.network.network.config.port_base + 3).await;
    cli(&service.state(), &["start", "integration"]).await;
    service.stop(&client).await;

    let help = Command::from(acton(service.root.path(), &["--help"]))
        .output()
        .await
        .expect("CLI help");
    let help = String::from_utf8(help.stdout).expect("UTF-8 help");
    let descriptor = service.network.path.join("service.json");
    let mut cleanup = Vec::new();

    let empty = cli(&service.state(), &["snapshot", "integration", "list"]).await;
    cleanup.push(!descriptor.exists());
    let created = cli(
        &service.state(),
        &["snapshot", "integration", "create", "Baseline"],
    )
    .await;
    cleanup.push(!descriptor.exists());
    let id = created["result"]["id"].as_str().expect("snapshot ID");
    let listed = cli(&service.state(), &["snapshot", "integration", "list"]).await;
    cleanup.push(!descriptor.exists());

    // Full restore starts the APIs to rebuild indexing. Closing the temporary
    // service afterwards returns the network to its stopped lifecycle.
    let restored = cli(
        &service.state(),
        &["snapshot", "integration", "restore", id, "--yes"],
    )
    .await;
    cleanup.push(!descriptor.exists());

    // An operation failure must still reap the service and preserve the saved snapshot.
    let failed = Command::from(acton(
        service.root.path(),
        &[
            "snapshot",
            "integration",
            "restore",
            "missing",
            "--yes",
            "--json",
        ],
    ))
    .output()
    .await
    .expect("failed restore output");
    cleanup.push(!descriptor.exists());

    let deleted = cli(
        &service.state(),
        &["snapshot", "integration", "delete", id, "--yes"],
    )
    .await;
    cleanup.push(!descriptor.exists());
    let remaining = cli(&service.state(), &["snapshot", "integration", "list"]).await;
    cleanup.push(!descriptor.exists());
    let status = cli(&service.state(), &["status", "integration"]).await;
    let events =
        std::fs::read_to_string(service.root.path().join("events")).expect("Docker events");

    expect![[r#"
        {
          "cleanup": [
            true,
            true,
            true,
            true,
            true,
            true,
            true
          ],
          "created": "completed",
          "deleted": "completed",
          "empty": [],
          "failedRestore": true,
          "listed": "Baseline",
          "networkStarts": 2,
          "remaining": [],
          "restored": "completed",
          "serveVisible": false,
          "status": "stopped"
        }"#]]
    .assert_eq(
        &serde_json::to_string_pretty(&json!({
            "cleanup": cleanup,
            "created": created["status"],
            "deleted": deleted["status"],
            "empty": empty,
            "failedRestore": !failed.status.success(),
            "listed": listed[0]["name"],
            "networkStarts": events.lines().filter(|line| *line == "up").count(),
            "remaining": remaining,
            "restored": restored["status"],
            "serveVisible": help.lines().any(|line| line.split_whitespace().next() == Some("serve")),
            "status": status["status"],
        }))
        .expect("snapshot lifecycle summary"),
    );

    v2.abort();
    v3.abort();
    let _ = tokio::join!(v2, v3);

    drop(service);
}

#[tokio::test]
async fn interrupted_snapshot_finishes_its_archive_and_closes_the_temporary_service() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    service.stop(&client).await;
    let hold = service.root.path().join("hold-snapshot");
    std::fs::write(&hold, "").expect("hold archive creation");

    let mut command = Command::from(acton(
        service.root.path(),
        &["snapshot", "integration", "create", "Interrupted", "--json"],
    ))
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .kill_on_drop(true)
    .spawn()
    .expect("snapshot command");

    tokio::time::timeout(Duration::from_secs(20), async {
        while !service.root.path().join("snapshot-entered").exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("archive creation started");

    Command::new("kill")
        .args(["-INT", &command.id().expect("snapshot CLI PID").to_string()])
        .status()
        .await
        .expect("interrupt owned snapshot command");
    std::fs::remove_file(hold).expect("allow archive creation to finish");

    let status = tokio::time::timeout(Duration::from_secs(20), command.wait())
        .await
        .expect("snapshot cleanup deadline")
        .expect("snapshot CLI exit");
    let service_closed = !service.network.path.join("service.json").exists();
    let snapshots = cli(&service.state(), &["snapshot", "integration", "list"]).await;
    drop(service);

    expect![[r#"{"archive":"Interrupted","serviceClosed":true,"success":true}"#]].assert_eq(
        &json!({
            "archive": snapshots[0]["name"],
            "serviceClosed": service_closed,
            "success": status.success(),
        })
        .to_string(),
    );
}
