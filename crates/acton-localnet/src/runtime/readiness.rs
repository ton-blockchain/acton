//! Network readiness is measured by the service, regardless of which UI starts it.

use super::Entry;
use crate::OperationProgress;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{sync::watch, task::JoinHandle};

/// Owns the concurrent probe until startup succeeds, fails, or is cancelled.
/// A successful HTTP response alone does not prove the indexer has caught up.
pub(super) fn observe(entry: Arc<Entry>) -> (watch::Receiver<OperationProgress>, JoinHandle<()>) {
    let (sender, receiver) = watch::channel(OperationProgress {
        completed: 0,
        total: Some(3),
        unit: "checks passed".to_owned(),
        detail: "Waiting for TON APIs and indexer".to_owned(),
    });
    let task = tokio::spawn(async move {
        let started = Instant::now();
        let endpoints = entry.record.read().await.endpoints.clone();
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_millis(750))
            .build()
        else {
            return;
        };
        let ton = format!("{}/getMasterchainInfo", endpoints.api_v2);
        let indexer = format!("{}/masterchainInfo", endpoints.api_v3);
        let health = format!(
            "{}/healthcheck",
            endpoints.api_v3.trim_end_matches("/api/v3")
        );

        loop {
            let (ton, indexer, api) = tokio::join!(
                seqno(&client, &ton, "/result/last/seqno"),
                seqno(&client, &indexer, "/last/seqno"),
                client.get(&health).send(),
            );
            let ready = [
                ton.is_some(),
                api.is_ok_and(|r| r.status().is_success()),
                matches!((ton, indexer), (Some(ton), Some(indexer)) if indexer >= ton.saturating_sub(1)),
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
            let waiting = ["TON node", "API", "Indexer"]
                .into_iter()
                .zip(ready)
                .filter_map(|(name, ready)| (!ready).then_some(name))
                .collect::<Vec<_>>();
            sender.send_replace(OperationProgress {
                completed: ready.into_iter().filter(|ready| *ready).count() as u64,
                total: Some(3),
                unit: "checks passed".to_owned(),
                detail: if waiting.is_empty() {
                    "TON APIs and indexer ready".to_owned()
                } else {
                    format!("Waiting for {}", waiting.join(", "))
                },
            });
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
    (receiver, task)
}

async fn seqno(client: &reqwest::Client, url: &str, pointer: &str) -> Option<u64> {
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?
        .pointer(pointer)?
        .as_u64()
}
