//! An external CLI must wait beyond HTTP acceptance and report cleanup failures.

use super::{Service, acton, api_listener, cli};
use acton_localnet::Network;
use expect_test::expect;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

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
