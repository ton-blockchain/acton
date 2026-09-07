//! Adding a node must finish only when its own liteserver follows the network.

use super::*;
use crate::docker::test_support::{block, completed, head};
use crate::{CreateNetwork, Runtime, catalog, runtime::Action};
use anyhow::Result;
use serde_json::{Value, json};

#[tokio::test]
#[ignore = "requires Docker, ACTON_LOCALNET_IMAGE built from this checkout, and free ports 29550-29554"]
async fn joined_node_serves_same_chain() -> Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "node-join-regression".into(),
            port_base: Some(29550),
            block_time_ms: Some(1000),
            election_time_seconds: Some(3600),
            ..Default::default()
        },
    )
    .await?;
    let runtime = Runtime::open(&location.path).await?;
    let driver =
        DockerNetwork::materialize(&location.path, directory.path(), &location.network, false)
            .await?;
    eprintln!("Join regression state: {}", location.path.display());
    eprintln!("Join regression deployment: {}", driver.project_name);

    let result: Result<Value> = async {
        completed(&runtime, Action::Start).await?;
        completed(
            &runtime,
            Action::AddNode {
                name: "replica".into(),
                validator: false,
            },
        )
        .await?;

        let mut samples = Vec::new();
        for restarted in [false, true] {
            if restarted {
                completed(
                    &runtime,
                    Action::NodeRunning {
                        id: "node-1".into(),
                        running: false,
                    },
                )
                .await?;
                completed(
                    &runtime,
                    Action::NodeRunning {
                        id: "node-1".into(),
                        running: true,
                    },
                )
                .await?;
            }

            // No retries after Completed: readiness is the operation's contract,
            // not an extra synchronization step that callers should supply.
            let primary = head(&driver, "localton").await?;
            let replica = head(&driver, "node-1").await?;
            let seqno = primary.min(replica);
            samples.push(json!({
                "restarted": restarted,
                "sameChain": block(&driver, "localton", seqno).await?
                    == block(&driver, "node-1", seqno).await?,
                "nearHead": primary.abs_diff(replica) <= 2,
            }));
        }
        Ok(json!(samples))
    }
    .await;

    driver.delete().await?;
    if result.is_err() {
        eprintln!(
            "Preserved failed join state at {}",
            directory.keep().display()
        );
    }

    expect_test::expect![[r#"
        [
          {
            "nearHead": true,
            "restarted": false,
            "sameChain": true
          },
          {
            "nearHead": true,
            "restarted": true,
            "sameChain": true
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result?)?);
    Ok(())
}
