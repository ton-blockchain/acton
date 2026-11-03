//! The opt-in copy path stops and resumes the network, including after copy failure.

use super::{Service, acton, api_listener, cli};
use expect_test::expect;
use serde_json::json;
use tokio::process::Command;

#[tokio::test]
async fn node_copy_is_opt_in_and_restarts_the_network_after_failure() {
    let mut cases = Vec::new();
    for (copy_state, fail_copy, fail_stop, fail_restart) in [
        (false, false, false, false),
        (true, false, false, false),
        (true, true, false, false),
        (true, false, true, false),
        (true, false, false, true),
    ] {
        let mut service = Service::start(false).await;
        let client = service.client().await;
        let v2 = api_listener(service.network.network.config.port_base + 2).await;
        let v3 = api_listener(service.network.network.config.port_base + 3).await;
        cli(&service.state(), &["start", "integration"]).await;
        if fail_copy {
            std::fs::write(service.root.path().join("fail-copy"), "").expect("fail copy");
        }
        if fail_stop {
            std::fs::write(service.root.path().join("fail-stop"), "").expect("fail stop");
        }
        if fail_restart {
            std::fs::write(service.root.path().join("fail-restart"), "").expect("fail restart");
        }

        let mut args = vec!["node", "integration", "add", "peer", "--json"];
        if copy_state {
            args.push("--copy-state");
        }
        let output = Command::from(acton(service.root.path(), &args))
            .output()
            .await
            .expect("add node");
        let network = client.network().await.expect("network after operation");
        let events = std::fs::read_to_string(service.root.path().join("events"))
            .expect("Docker lifecycle events");
        cases.push(json!({
            "copyState": copy_state,
            "copyFailure": fail_copy,
            "stopFailure": fail_stop,
            "restartFailure": fail_restart,
            "copiedStateRetained": service.root.path().join("copied-state").exists(),
            "success": output.status.success(),
            "network": network.status,
            "nodes": network.nodes.len(),
            "events": events.lines().collect::<Vec<_>>(),
        }));

        if fail_stop {
            std::fs::remove_file(service.root.path().join("fail-stop")).expect("allow shutdown");
        }
        service.stop(&client).await;
        v2.abort();
        v3.abort();
        let _ = tokio::join!(v2, v3);
        drop(service);
    }

    expect![[r#"
        [
          {
            "copiedStateRetained": false,
            "copyFailure": false,
            "copyState": false,
            "events": [
              "up"
            ],
            "network": "running",
            "nodes": 1,
            "restartFailure": false,
            "stopFailure": false,
            "success": true
          },
          {
            "copiedStateRetained": true,
            "copyFailure": false,
            "copyState": true,
            "events": [
              "up",
              "stop",
              "copy-state",
              "up"
            ],
            "network": "running",
            "nodes": 1,
            "restartFailure": false,
            "stopFailure": false,
            "success": true
          },
          {
            "copiedStateRetained": false,
            "copyFailure": true,
            "copyState": true,
            "events": [
              "up",
              "stop",
              "copy-state",
              "up"
            ],
            "network": "running",
            "nodes": 0,
            "restartFailure": false,
            "stopFailure": false,
            "success": false
          },
          {
            "copiedStateRetained": false,
            "copyFailure": false,
            "copyState": true,
            "events": [
              "up",
              "stop",
              "up"
            ],
            "network": "running",
            "nodes": 0,
            "restartFailure": false,
            "stopFailure": true,
            "success": false
          },
          {
            "copiedStateRetained": false,
            "copyFailure": false,
            "copyState": true,
            "events": [
              "up",
              "stop",
              "copy-state",
              "up",
              "stop"
            ],
            "network": "failed",
            "nodes": 0,
            "restartFailure": true,
            "stopFailure": false,
            "success": false
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&cases).expect("copy lifecycle summary"));
}
