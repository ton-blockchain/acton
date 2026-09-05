//! Each foreground process must own only the network it explicitly started.

use super::{Service, acton, api_listener, cli};
use acton_localnet::{CreateNetwork, Network, Status, catalog, client::Client};
use expect_test::expect;
use reqwest::Method;
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::process::Command;

#[tokio::test]
async fn foreground_shutdown_leaves_other_networks_and_services_untouched() {
    let mut first = Service::start(false).await;
    let first_client = first.client().await;
    let mut listeners = Vec::new();
    for offset in [2, 3] {
        listeners.push(api_listener(first.network.network.config.port_base + offset).await);
    }
    cli(&first.state(), &["start", "integration"]).await;

    let second = catalog::create(
        &first.state(),
        CreateNetwork {
            name: "second".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("second network");
    let third = catalog::create(
        &first.state(),
        CreateNetwork {
            name: "third".to_owned(),
            ..Default::default()
        },
    )
    .await
    .expect("third network");
    let untouched = std::fs::read(third.path.join("network.json")).expect("third definition");
    for offset in [2, 3] {
        listeners.push(api_listener(second.network.config.port_base + offset).await);
    }

    let log = std::fs::File::create(first.root.path().join("second.log")).expect("second log");
    let mut foreground = Command::from(acton(first.root.path(), &["start", "second"]))
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().expect("clone log")))
        .stderr(Stdio::from(log))
        .kill_on_drop(true)
        .spawn()
        .expect("second foreground");
    let second_client = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let Ok(client) = Client::connect(&second.path).await {
                let network: Network = client
                    .request(Method::GET, "/v1/network", None)
                    .await
                    .expect("second status");
                if network.status == Status::Running {
                    break client;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("second startup");
    expect![["true"]]
        .assert_eq(&(first_client.service_pid() != second_client.service_pid()).to_string());

    Command::new("kill")
        .args([
            "-INT",
            &foreground.id().expect("foreground PID").to_string(),
        ])
        .status()
        .await
        .expect("Ctrl-C");
    let status = tokio::time::timeout(Duration::from_secs(20), foreground.wait())
        .await
        .expect("second shutdown deadline")
        .expect("second exit");
    let first_status: Network = first_client
        .request(Method::GET, "/v1/network", None)
        .await
        .expect("first service still alive");
    expect![["true:Running:true:false:false:true"]].assert_eq(&format!(
        "{}:{:?}:{}:{}:{}:{}",
        status.success(),
        first_status.status,
        first.network.path.join("fixture-running").exists(),
        second.path.join("fixture-running").exists(),
        third.path.join("runtime.json").exists(),
        untouched == std::fs::read(third.path.join("network.json")).expect("third unchanged"),
    ));
    let events =
        std::fs::read_to_string(first.root.path().join("network-events")).expect("Docker events");
    let started = events
        .lines()
        .filter_map(|line| {
            let event: Value = serde_json::from_str(line).expect("event");
            (event["command"] == "up").then(|| event["network"].as_str().expect("name").to_owned())
        })
        .collect::<Vec<_>>();
    expect![[r#"["integration","second"]"#]]
        .assert_eq(&serde_json::to_string(&started).expect("starts"));

    first.stop(&first_client).await;
    for listener in listeners {
        listener.abort();
        let _ = listener.await;
    }

    drop(first);
}
