//! An external CLI must wait beyond HTTP acceptance and report cleanup failures.

use super::{Service, acton, api_listener, cli};
use acton_localnet::Network;
use expect_test::expect;
use serde_json::json;
use std::{process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[tokio::test]
async fn shutdown_after_a_prerequisite_failure_preserves_the_startup_error() {
    let mut service = Service::start(false).await;
    let client = service.client().await;
    std::fs::write(service.root.path().join("docker-unavailable"), "")
        .expect("Docker is not running");
    let startup = tokio::time::timeout(
        Duration::from_secs(20),
        Command::from(acton(
            service.root.path(),
            &["start", "integration", "--json"],
        ))
        .output(),
    )
    .await
    .expect("startup failure deadline")
    .expect("startup output");
    let failed = client.network().await.expect("failed network");

    // Keep the service alive until the client requests shutdown. There is no
    // deployment to clean up, but the startup diagnostic must remain available.
    let (shutdown, repeated) = tokio::time::timeout(Duration::from_secs(20), async {
        (client.shutdown().await, client.shutdown().await)
    })
    .await
    .expect("client shutdown deadline");
    let exit = tokio::time::timeout(Duration::from_secs(20), service.child.wait())
        .await
        .expect("service shutdown deadline")
        .expect("service exit");
    let observed: Network = serde_json::from_value(cli(&service.state(), &["status"]).await)
        .expect("offline network status");
    expect![[r#"
        {
          "deploymentCreated": false,
          "errorRetained": true,
          "repeatedShutdownSucceeded": true,
          "serviceLeftRunning": false,
          "serviceSucceeded": true,
          "shutdownSucceeded": true,
          "startupSucceeded": false,
          "status": "failed"
        }"#]]
    .assert_eq(
        &serde_json::to_string_pretty(&json!({
            "startupSucceeded": startup.status.success(),
            "shutdownSucceeded": shutdown.is_ok(),
            "repeatedShutdownSucceeded": repeated.is_ok(),
            "serviceSucceeded": exit.success(),
            "status": observed.status,
            "errorRetained": failed.error.is_some() && observed.error == failed.error,
            "deploymentCreated": service.network.path.join("runtime.json").exists(),
            "serviceLeftRunning": service.network.path.join("service.json").exists(),
        }))
        .expect("shutdown outcome"),
    );
    drop(service);
}

#[tokio::test]
async fn failed_foreground_owner_does_not_hide_the_service_cleanup_result() {
    let mut outcomes = Vec::new();
    for fail_stop in [false, true] {
        let mut service = Service::start(false).await;
        let _client = service.client().await;
        let v2 = api_listener(service.network.network.config.port_base + 2).await;
        let v3 = api_listener(service.network.network.config.port_base + 3).await;
        cli(&service.state(), &["start", "integration"]).await;
        if fail_stop {
            std::fs::write(service.root.path().join("fail-stop"), "").expect("fail cleanup");
        }

        // Keep the owner alive until shutdown so its failed exit is observed by
        // wait(), rather than winning the race with the initial try_wait().
        let mut owner = Command::new("sh")
            .args([
                "-c",
                "trap 'exit 1' TERM; printf 'ready\\n'; while :; do sleep 0.1; done",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("foreground owner fixture");
        let mut ready = String::new();
        BufReader::new(owner.stdout.take().expect("owner stdout"))
            .read_line(&mut ready)
            .await
            .expect("owner signal handler ready");
        assert_eq!(ready, "ready\n");

        let launcher = acton_localnet::process::Launcher {
            executable: env!("CARGO_BIN_EXE_acton").into(),
            project_root: service.root.path().to_owned(),
            catalog_root: service.state(),
        };
        let result = tokio::time::timeout(
            Duration::from_secs(20),
            launcher.shutdown_started(&service.network, &mut owner),
        )
        .await
        .expect("owner shutdown deadline");
        let status = tokio::time::timeout(Duration::from_secs(20), service.child.wait())
            .await
            .expect("service shutdown deadline")
            .expect("service exit");
        outcomes.push(format!(
            "{}:{}:{}",
            result.is_ok(),
            status.success(),
            result.err().is_some_and(|error| error
                .to_string()
                .contains("Docker could not stop the fixture network")),
        ));
        v2.abort();
        v3.abort();
        let _ = tokio::join!(v2, v3);
        drop(service);
    }
    expect![[r"
        true:true:false
        false:false:true
    "]]
    .assert_eq(&format!("{}\n", outcomes.join("\n")));
}

#[tokio::test]
async fn external_shutdown_waits_for_cleanup_and_reports_its_result() {
    let mut outcomes = Vec::new();
    for fail_stop in [false, true] {
        let mut service = Service::start(false).await;
        let _client = service.client().await;
        let v2 = api_listener(service.network.network.config.port_base + 2).await;
        let v3 = api_listener(service.network.network.config.port_base + 3).await;
        cli(&service.state(), &["start", "integration"]).await;
        std::fs::write(service.root.path().join("hold-stop"), "").expect("delay cleanup");
        if fail_stop {
            std::fs::write(service.root.path().join("fail-stop"), "").expect("fail cleanup");
        }

        let mut shutdown = Command::from(acton(
            service.root.path(),
            &["shutdown", "integration", "--json"],
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("shutdown command");
        tokio::time::timeout(Duration::from_secs(10), async {
            while !service.root.path().join("stop-entered").exists() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("shutdown reached Docker");

        // Give a command that incorrectly exits on HTTP 202 time to exit while
        // the fixture holds cleanup at an explicit synchronization boundary.
        tokio::time::sleep(Duration::from_millis(300)).await;
        expect![["true:true"]].assert_eq(&format!(
            "{}:{}",
            shutdown.try_wait().expect("shutdown status").is_none(),
            service.network.path.join("fixture-running").exists(),
        ));
        std::fs::remove_file(service.root.path().join("hold-stop")).expect("release cleanup");
        let output = tokio::time::timeout(Duration::from_secs(15), shutdown.wait_with_output())
            .await
            .expect("shutdown completion")
            .expect("CLI output");
        let service_status = service.child.wait().await.expect("service exit");
        let record: Network = serde_json::from_slice(
            &std::fs::read(service.network.path.join("network.json")).expect("network record"),
        )
        .expect("network");
        let error = String::from_utf8_lossy(&output.stderr);
        outcomes.push(format!(
            "{}:{}:{:?}:{}:{}:{}",
            output.status.success(),
            service_status.success(),
            record.status,
            service.network.path.join("service.json").exists(),
            error.contains("Docker could not stop the fixture network"),
            error.contains("service.log"),
        ));
        v2.abort();
        v3.abort();

        let _ = tokio::join!(v2, v3);
        drop(service);
    }
    expect![[r"
        true:true:Stopped:false:false:false
        false:false:Failed:false:true:true
    "]]
    .assert_eq(&format!("{}\n", outcomes.join("\n")));
}
