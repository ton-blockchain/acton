//! Adding a node must finish only when its own liteserver follows the network.

use super::*;
use crate::docker::test_support::{block, completed, head};
use crate::{CreateNetwork, Runtime, catalog, runtime::Action};
use anyhow::Result;
use serde_json::{Value, json};

#[tokio::test]
#[ignore = "requires Docker, ACTON_LOCALNET_IMAGE built from this checkout, and free ports 29550-29555"]
async fn joined_node_serves_same_chain() -> Result<()> {
    check_joined_node(false).await
}

#[tokio::test]
#[ignore = "requires Docker, ACTON_LOCALNET_IMAGE built from this checkout, and free ports 29650-29655"]
async fn copied_node_serves_same_chain_with_independent_keys() -> Result<()> {
    check_joined_node(true).await
}

async fn check_joined_node(copy_state: bool) -> Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "node-join-regression".into(),
            port_base: Some(if copy_state { 29650 } else { 29550 }),
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
        let genesis_identity = identity(&driver, "localton").await?;
        completed(
            &runtime,
            Action::AddNode {
                name: "replica".into(),
                validator: copy_state,
                copy_state,
            },
        )
        .await?;

        let replica_identity = identity(&driver, "node-1").await?;
        let independent_keys = [
            "console_public_key",
            "liteserver_public_key",
            "full_node_adnl",
        ]
        .iter()
        .all(|key| {
            replica_identity[*key].is_string() && genesis_identity[*key] != replica_identity[*key]
        }) && replica_identity["validator_public_key"].is_null()
            && replica_identity["validator_adnl"].is_null();
        let genesis_unchanged = identity(&driver, "localton").await? == genesis_identity;

        let mut logs = driver.compose_command();
        logs.args(["logs", "--no-color", "--no-log-prefix", "node-1"]);
        let logs = driver
            .command_output(
                logs,
                "read joined node initialization log",
                "test_logs_failed",
                Duration::from_secs(20),
            )
            .await?;
        let copied_data_installed =
            String::from_utf8_lossy(&logs.stdout).contains("install_node_state");

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
                "copyModeRespected": copied_data_installed == copy_state,
                "genesisUnchanged": genesis_unchanged,
                "independentKeys": independent_keys,
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
            "copyModeRespected": true,
            "genesisUnchanged": true,
            "independentKeys": true,
            "nearHead": true,
            "restarted": false,
            "sameChain": true
          },
          {
            "copyModeRespected": true,
            "genesisUnchanged": true,
            "independentKeys": true,
            "nearHead": true,
            "restarted": true,
            "sameChain": true
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result?)?);
    Ok(())
}

async fn identity(driver: &DockerNetwork, service: &str) -> Result<Value> {
    let mut command = driver.compose_command();
    command.args([
        "exec",
        "--no-TTY",
        service,
        "cat",
        "/var/lib/localton/node/node-manifest.json",
    ]);
    let output = driver
        .command_output(
            command,
            "read public node identity",
            "test_identity_failed",
            Duration::from_secs(20),
        )
        .await?;
    Ok(serde_json::from_slice(&output.stdout)?)
}
