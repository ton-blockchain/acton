//! Inventory reads stay available throughout a mutation, without invoking Docker.

use super::*;
use crate::{CreateNetwork, catalog};
use serde_json::json;

#[tokio::test]
async fn snapshot_inventory_reads_only_committed_manifests_while_busy() -> anyhow::Result<()> {
    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "snapshot-inventory".into(),
            port_base: Some(29380),
            ..Default::default()
        },
    )
    .await?;
    let runtime = Runtime::open(&location.path).await?;
    let inventory = location.path.join("snapshots");
    tokio::fs::create_dir(&inventory).await?;
    let snapshot = json!({
        "formatVersion": 3, "id": "snapshot-1", "name": "Baseline", "createdAt": 10,
        "archiveSizeBytes": 200, "stateSizeBytes": 400, "stateSchemaVersion": 4,
        "tonRelease": "test", "masterchainSeqno": 42,
    });
    storage::write_json(
        &inventory.join("snapshot-1.json"),
        &json!({
            "snapshot": snapshot, "nodes": [], "archives": {"localton": snapshot},
        }),
    )
    .await?;
    tokio::fs::write(inventory.join("snapshot-2.tmp"), "incomplete JSON").await?;

    let _mutation = runtime.inner.entry.mutation.lock().await;
    let snapshots = runtime.snapshots().await?;
    expect_test::expect![[r#"
        [
          {
            "formatVersion": 3,
            "id": "snapshot-1",
            "name": "Baseline",
            "createdAt": 10,
            "archiveSizeBytes": 200,
            "stateSizeBytes": 400,
            "stateSchemaVersion": 4,
            "tonRelease": "test",
            "masterchainSeqno": 42
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&snapshots)?);
    Ok(())
}

#[tokio::test]
async fn readiness_rejects_future_index_and_stale_or_future_node_heads() -> anyhow::Result<()> {
    use axum::{
        Json, Router,
        extract::{Path, State},
        routing::get,
    };

    let samples = Arc::new(RwLock::new(json!({
        "indexer": 387, "node": 100, "observedAt": u64::MAX,
    })));
    let app = Router::new()
        .route(
            "/{*path}",
            get(
                |State(samples): State<Arc<RwLock<serde_json::Value>>>,
                 Path(path): Path<String>| async move {
                    let samples = samples.read().await;
                    Json(if path.ends_with("getMasterchainInfo") {
                        json!({"result": {"last": {"seqno": 100}}})
                    } else if path.ends_with("masterchainInfo") {
                        json!({"last": {"seqno": samples["indexer"]}})
                    } else {
                        json!({"nodes": [{"name": "replica", "online": true, "running": true,
                "head_seqno": samples["node"], "head_observed_at": samples["observedAt"]}]})
                    })
                },
            ),
        )
        .with_state(Arc::clone(&samples));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base = format!("http://{}", listener.local_addr()?);
    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    let directory = tempfile::tempdir_in("/tmp")?;
    let location = catalog::create(
        directory.path(),
        CreateNetwork {
            name: "snapshot-readiness".into(),
            port_base: Some(29390),
            ..Default::default()
        },
    )
    .await?;
    let runtime = Runtime::open(&location.path).await?;
    {
        let mut record = runtime.inner.entry.record.write().await;
        record.endpoints.api_v2 = format!("{base}/api/v2");
        record.endpoints.api_v3 = format!("{base}/api/v3");
        record.endpoints.observability = base;
        record.nodes.push(crate::Node {
            id: "node-1".into(),
            name: "replica".into(),
            validator: false,
            port_base: 20010,
            stopped: false,
        });
    }
    let (mut progress, task) = readiness::observe(Arc::clone(&runtime.inner.entry));
    let result = tokio::time::timeout(std::time::Duration::from_secs(8), async {
        let mut phases = Vec::new();
        for (indexer, node, observed_at, waiting) in [
            (387, 100, u64::MAX, "Waiting for Indexer"),
            (100, 387, u64::MAX, "Waiting for Full nodes"),
            (100, 100, 0, "Waiting for Full nodes"),
            (100, 100, u64::MAX, "TON nodes, APIs and indexer ready"),
        ] {
            *samples.write().await = json!({"indexer": indexer, "node": node, "observedAt": observed_at});
            loop {
                progress.changed().await?;
                let current = progress.borrow_and_update().clone();
                if current.detail == waiting {
                    phases.push(json!({"completed": current.completed, "total": current.total, "detail": current.detail}));
                    break;
                }
            }
        }
        Ok::<_, anyhow::Error>(phases)
    }).await;
    task.abort();
    server.abort();
    expect_test::expect![[r#"
        [
          {
            "completed": 3,
            "detail": "Waiting for Indexer",
            "total": 4
          },
          {
            "completed": 3,
            "detail": "Waiting for Full nodes",
            "total": 4
          },
          {
            "completed": 3,
            "detail": "Waiting for Full nodes",
            "total": 4
          },
          {
            "completed": 4,
            "detail": "TON nodes, APIs and indexer ready",
            "total": 4
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&result??)?);
    Ok(())
}
