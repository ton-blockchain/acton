//! Network readiness is measured by the service, regardless of which UI starts it.

use super::Entry;
use crate::OperationProgress;
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::watch, task::JoinHandle};

/// Owns the concurrent probe until startup succeeds, fails, or is cancelled.
/// A successful HTTP response alone does not prove the indexer has caught up.
pub(super) fn observe(entry: Arc<Entry>) -> (watch::Receiver<OperationProgress>, JoinHandle<()>) {
    let (sender, receiver) = watch::channel(OperationProgress {
        completed: 0,
        total: Some(4),
        unit: "checks passed".to_owned(),
        detail: "Waiting for TON APIs and indexer".to_owned(),
    });
    let task = tokio::spawn(async move {
        let started = Instant::now();
        let network = entry.record.read().await.clone();
        let endpoints = network.endpoints;
        let nodes = network
            .nodes
            .into_iter()
            .filter(|node| !node.stopped)
            .collect::<Vec<_>>();
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_millis(750))
            .build()
        else {
            return;
        };
        let Ok(toncenter) = toncenter_client::Client::builder()
            .v2_url(&endpoints.api_v2)
            .v3_url(&endpoints.api_v3)
            .user_agent(concat!("acton/", env!("CARGO_PKG_VERSION")))
            .operation_timeout(Duration::from_millis(750))
            .max_attempts(1)
            .build()
        else {
            return;
        };
        let health = format!(
            "{}/healthcheck",
            endpoints.api_v3.trim_end_matches("/api/v3")
        );

        loop {
            let (ton, indexer, api) = tokio::join!(
                seqno(&toncenter, true),
                seqno(&toncenter, false),
                client.get(&health).send(),
            );
            let nodes_ready = if nodes.is_empty() {
                true
            } else {
                joined_nodes_ready(&client, &endpoints.observability, &nodes, ton, started_at).await
            };
            let ready = [
                ton.is_some(),
                api.is_ok_and(|r| r.status().is_success()),
                matches!((ton, indexer), (Some(ton), Some(indexer)) if indexer.abs_diff(ton) <= 1),
                nodes_ready,
            ];
            let elapsed = started.elapsed().as_millis() as u64;
            {
                let mut record = entry.record.write().await;
                if let Some(timings) = &mut record.startup_timings {
                    for (ready, timing) in ready.into_iter().zip([
                        &mut timings.ton_ready_ms,
                        &mut timings.api_ready_ms,
                        &mut timings.indexer_ready_ms,
                    ]) {
                        if ready && timing.is_none() {
                            *timing = Some(elapsed);
                        }
                    }
                }
            }
            let waiting = ["TON node", "API", "Indexer", "Full nodes"]
                .into_iter()
                .zip(ready)
                .filter_map(|(name, ready)| (!ready).then_some(name))
                .collect::<Vec<_>>();
            sender.send_replace(OperationProgress {
                completed: ready.into_iter().filter(|ready| *ready).count() as u64,
                total: Some(4),
                unit: "checks passed".to_owned(),
                detail: if waiting.is_empty() {
                    "TON nodes, APIs and indexer ready".to_owned()
                } else {
                    format!("Waiting for {}", waiting.join(", "))
                },
            });
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
    (receiver, task)
}

async fn seqno(client: &toncenter_client::Client, v2: bool) -> Option<u64> {
    let value: serde_json::Value = if v2 {
        client
            .v2_request(
                toncenter_client::V2Transport::Get,
                "getMasterchainInfo",
                &(),
            )
            .await
            .ok()?
    } else {
        client.v3_get("masterchainInfo", &()).await.ok()?
    };
    value.pointer("/last/seqno")?.as_u64()
}

/// Container health is insufficient for joined nodes. Require a fresh successful
/// liteserver sample after startup, close to the primary chain in either direction.
async fn joined_nodes_ready(
    client: &reqwest::Client,
    endpoint: &str,
    nodes: &[crate::Node],
    head: Option<u64>,
    started_at: u64,
) -> bool {
    let Some(head) = head else {
        return false;
    };
    let Ok(response) = client
        .get(format!("{endpoint}/api/v1/network"))
        .send()
        .await
    else {
        return false;
    };
    let Ok(view) = response.json::<serde_json::Value>().await else {
        return false;
    };
    let Some(observed) = view["nodes"].as_array() else {
        return false;
    };
    nodes.iter().all(|node| {
        observed.iter().any(|sample| {
            sample["name"].as_str() == Some(node.name.as_str())
                && sample["online"] == true
                && sample["running"] == true
                && sample["head_observed_at"]
                    .as_u64()
                    .is_some_and(|time| time >= started_at)
                && sample["head_seqno"]
                    .as_u64()
                    .is_some_and(|seqno| seqno.abs_diff(head) <= 2)
        })
    })
}
