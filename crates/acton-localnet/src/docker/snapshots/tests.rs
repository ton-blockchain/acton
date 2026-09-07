//! Opt-in Docker regression covering the real owner, node archives and indexer.

use super::*;
use crate::docker::test_support::{block, completed, head, operation};
use crate::{CreateNetwork, Runtime, catalog, runtime::Action};
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::time::Duration;

async fn indexed_accounts(driver: &DockerNetwork) -> Result<String> {
    let mut command = driver.compose_command();
    command.args([
        "exec",
        "-T",
        "postgres",
        "psql",
        "-U",
        "postgres",
        "-d",
        "ton_index",
        "-Atc",
        "SELECT account FROM latest_account_states ORDER BY account",
    ]);
    let output = driver
        .command_output(
            command,
            "check indexed accounts",
            "snapshot_test",
            Duration::from_secs(30),
        )
        .await?;
    Ok(String::from_utf8(output.stdout)?)
}

#[tokio::test]
#[ignore = "requires Docker, the pinned Localton image, and free ports 29350-29354 and 20010-20019"]
async fn whole_network_snapshot_roundtrip_and_rollback() -> Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "snapshot-regression".into(),
            port_base: Some(29350),
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
    eprintln!("Snapshot regression deployment: {}", driver.project_name);

    // Cleanup runs before assertions too, so a regression does not leave a live network.
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

        let network = runtime.get().await;
        let driver =
            DockerNetwork::materialize(&location.path, directory.path(), &network, false).await?;
        // A joined full node need not retain blocks from before it joined.
        // Verify a new block that both nodes have actually processed.
        let seqno = tokio::time::timeout(Duration::from_secs(240), async {
            loop {
                if let (Ok(primary), Ok(replica)) = (
                    head(&driver, "localton").await,
                    head(&driver, "node-1").await,
                ) {
                    break primary.max(replica) + 3;
                }
                eprintln!("Waiting for joined node liteserver");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await?;
        tokio::time::timeout(Duration::from_secs(60), async {
            while head(&driver, "localton").await? < seqno || head(&driver, "node-1").await? < seqno
            {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        eprintln!("Checking common block {seqno}");
        let genesis = block(&driver, "localton", seqno).await?;
        let replica = block(&driver, "node-1", seqno).await?;
        ensure!(
            genesis == replica,
            "Replica did not join the baseline chain"
        );

        let accounts = indexed_accounts(&driver).await?;
        ensure!(
            accounts.lines().count() > 5,
            "Incomplete baseline account scan"
        );
        let snapshot = completed(
            &runtime,
            Action::CreateSnapshot {
                name: Some("two nodes".into()),
            },
        )
        .await?;
        let id = snapshot["id"]
            .as_str()
            .context("two-node snapshot id")?
            .to_owned();
        let manifest_path = driver.snapshot_path(&id);
        let bundle: Bundle = storage::read_json(&manifest_path).await?;

        completed(
            &runtime,
            Action::NodeRunning {
                id: "node-1".into(),
                running: false,
            },
        )
        .await?;
        let mut rounds = Vec::new();
        for _ in 0..2 {
            completed(&runtime, Action::RestoreSnapshot { id: id.clone() }).await?;
            rounds.push(json!({
                "accountsPreserved": indexed_accounts(&driver).await? == accounts,
                "sameChain": block(&driver, "localton", seqno).await? == genesis
                    && block(&driver, "node-1", seqno).await? == genesis,
            }));
        }

        completed(
            &runtime,
            Action::RemoveNode {
                id: "node-1".into(),
                force: false,
            },
        )
        .await?;
        let single = completed(
            &runtime,
            Action::CreateSnapshot {
                name: Some("single".into()),
            },
        )
        .await?;
        let single_id = single["id"]
            .as_str()
            .context("single snapshot id")?
            .to_owned();
        completed(&runtime, Action::RestoreSnapshot { id: id.clone() }).await?;

        // Removing the replica after the snapshot must not make its archive unusable.
        completed(&runtime, Action::RestoreSnapshot { id: single_id }).await?;
        let single_nodes = runtime.get().await.nodes.len();
        completed(&runtime, Action::RestoreSnapshot { id: id.clone() }).await?;
        let restored_nodes = runtime.get().await.nodes.len();

        // Genesis restores first; the missing second archive forces a partial failure.
        // The rollback must recover both databases and clear its durable journal.
        let mut broken = bundle.clone();
        broken
            .archives
            .get_mut("node-1")
            .context("replica archive")?
            .id = "snapshot-missing".into();
        storage::write_json(&manifest_path, &broken).await?;
        let failed = operation(&runtime, Action::RestoreSnapshot { id: id.clone() }).await?;
        storage::write_json(&manifest_path, &bundle).await?;

        let summary = json!({
            "archives": bundle.archives.keys().collect::<Vec<_>>(),
            "roundtrips": rounds,
            "singleNodes": single_nodes,
            "restoredNodes": restored_nodes,
            "restoredNodeEnabled": !runtime.get().await.nodes[0].stopped,
            "partialRestore": failed.status,
            "recoveryJournal": driver.has_snapshot_recovery(),
            "recoveredStatus": runtime.get().await.status,
            "recoveredAccounts": indexed_accounts(&driver).await? == accounts,
            "recoveredChain": block(&driver, "localton", seqno).await? == genesis
                && block(&driver, "node-1", seqno).await? == genesis,
        });

        completed(&runtime, Action::Stop).await?;
        completed(
            &runtime,
            Action::CreateSnapshot {
                name: Some("stopped".into()),
            },
        )
        .await?;
        ensure!(
            runtime.get().await.status == crate::Status::Stopped,
            "Stopped snapshot started the network"
        );
        Ok(summary)
    }
    .await;

    if result.is_err() {
        driver.stop().await?;
        eprintln!(
            "Preserved failed test state at {}",
            directory.keep().display()
        );
    } else {
        driver.delete().await?;
    }
    expect_test::expect![[r#"
        {
          "archives": [
            "localton",
            "node-1"
          ],
          "partialRestore": "failed",
          "recoveredAccounts": true,
          "recoveredChain": true,
          "recoveredStatus": "running",
          "recoveryJournal": false,
          "restoredNodeEnabled": true,
          "restoredNodes": 1,
          "roundtrips": [
            {
              "accountsPreserved": true,
              "sameChain": true
            },
            {
              "accountsPreserved": true,
              "sameChain": true
            }
          ],
          "singleNodes": 0
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result?)?);
    Ok(())
}

#[tokio::test]
#[ignore = "requires Docker, the pinned Localton image, and free ports 29450-29454"]
async fn interrupted_snapshot_restore_recovers_on_owner_restart() -> Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "snapshot-owner-recovery".into(),
            port_base: Some(29450),
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

    let result: Result<Value> = async {
        completed(&runtime, Action::Start).await?;
        let snapshot = completed(&runtime, Action::CreateSnapshot { name: None }).await?;
        let id = snapshot["id"].as_str().context("snapshot id")?;
        let seqno = head(&driver, "localton").await? + 3;
        tokio::time::timeout(Duration::from_secs(30), async {
            while head(&driver, "localton").await? < seqno {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        let expected_block = block(&driver, "localton", seqno).await?;
        let accounts = indexed_accounts(&driver).await?;

        runtime.shutdown().await?;
        driver.restore_snapshot(id, &[]).await?;
        ensure!(
            driver.has_snapshot_recovery(),
            "Uncommitted restore has no journal"
        );
        drop(runtime);

        let runtime = Runtime::open(&location.path).await?;
        completed(&runtime, Action::Start).await?;
        Ok(json!({
            "journalRemoved": !driver.has_snapshot_recovery(),
            "originalBlockRecovered": block(&driver, "localton", seqno).await? == expected_block,
            "accountsPreserved": indexed_accounts(&driver).await? == accounts,
            "status": runtime.get().await.status,
        }))
    }
    .await;

    driver.delete().await?;
    expect_test::expect![[r#"
        {
          "accountsPreserved": true,
          "journalRemoved": true,
          "originalBlockRecovered": true,
          "status": "running"
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&result?)?);
    Ok(())
}
