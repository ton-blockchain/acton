use std::time::Duration;

use anyhow::{Result, ensure};
use serde::Serialize;
use tokio::time::Instant;
use ton_indexer_core::{BlockId, BlockSource, CheckpointStore, FileCheckpointStore};
use ton_indexer_p2p::P2pBlockSource;
use ton_p2p::{Client, ClientOptions, NetworkConfig};
use tracing::info;

/// Progress for this invocation. Batch counts include blocks reused from the cache.
#[derive(Serialize)]
pub(crate) struct SyncReport {
    head: BlockId,
    downloaded_blocks: u64,
    shard_blocks: u64,
    verification: &'static str,
}

/// Runs until the target is reached or the CLI cancels the future.
/// The library commits downloaded files; this loop records complete CLI batches.
pub(crate) async fn run(
    config: &NetworkConfig,
    options: ClientOptions,
    to_seqno: Option<u32>,
    masterchain_only: bool,
) -> Result<SyncReport> {
    let checkpoint = FileCheckpointStore::new(options.data_dir.join("batch-checkpoint.json"));
    let mut source = P2pBlockSource::new(Client::open(config, options)?)?;
    let mut after = if masterchain_only {
        None
    } else {
        checkpoint.load().await?
    };

    if let Some(checkpoint) = &after {
        source
            .client()
            .validate_checkpoint(&(*checkpoint).try_into()?)?;
        ensure!(
            source
                .client()
                .head()
                .is_some_and(|head| checkpoint.seqno <= head.seqno),
            "batch checkpoint is ahead of the downloaded masterchain"
        );
    }

    ensure!(
        to_seqno.is_none_or(|target| target >= source.client().anchor().seqno),
        "sync target precedes P2P anchor"
    );

    let mut report = SyncReport {
        head: if masterchain_only {
            source
                .client()
                .head()
                .unwrap_or_else(|| source.client().anchor())
                .into()
        } else {
            after.unwrap_or_else(|| source.client().anchor().into())
        },
        downloaded_blocks: 0,
        shard_blocks: 0,
        verification: "hashes_and_links",
    };
    let started = Instant::now();
    let mut progress = started;

    info!(
        operation = "block_sync",
        target = %report.head,
        masterchain_only,
        outcome = "started",
        "starting block synchronization",
    );

    loop {
        if to_seqno.is_some_and(|target| report.head.seqno >= target)
            && (!masterchain_only || source.client().head().is_some())
        {
            info!(
                operation = "block_sync",
                target = %report.head,
                blocks = report.downloaded_blocks,
                shard_blocks = report.shard_blocks,
                duration_ms = started.elapsed().as_millis(),
                outcome = "target_reached",
                "block synchronization target reached",
            );
            return Ok(report);
        }

        let downloaded = if masterchain_only {
            source
                .client_mut()
                .next_masterchain()
                .await?
                .map(|id| (id.into(), 0))
        } else if let Some(batch) = source.next_batch(after.as_ref()).await? {
            let id = batch.masterchain().id();
            checkpoint.save(&id).await?;
            after = Some(id);
            Some((id, batch.shards().len() as u64))
        } else {
            None
        };

        if let Some((id, shards)) = downloaded {
            report.head = id;
            report.downloaded_blocks += 1;
            report.shard_blocks += shards;
        } else {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        if progress.elapsed() >= Duration::from_secs(10) {
            info!(
                operation = "block_sync",
                target = %report.head,
                blocks = report.downloaded_blocks,
                shard_blocks = report.shard_blocks,
                duration_ms = started.elapsed().as_millis(),
                outcome = "syncing",
                "block synchronization progress",
            );
            progress = Instant::now();
        }
    }
}
