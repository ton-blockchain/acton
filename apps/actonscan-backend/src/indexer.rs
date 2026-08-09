use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use ton_indexer_core::{Batch, IndexPipeline, RunOutcome, Sink};
use ton_indexer_liteserver::{CanonicalBlockSource, TonutilsLiteClient};

use crate::{
    SqliteStorage,
    config::IndexerConfig,
    opcodes::{OpcodeBatchStats, OpcodeStats},
    stats::TpsStats,
};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);

pub(crate) fn spawn(
    config: IndexerConfig,
    tps_stats: TpsStats,
    opcode_stats: OpcodeStats,
    storage: SqliteStorage,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(run(config, tps_stats, opcode_stats, storage))
}

async fn run(
    config: IndexerConfig,
    tps_stats: TpsStats,
    opcode_stats: OpcodeStats,
    storage: SqliteStorage,
) {
    loop {
        if let Err(error) = run_connection(&config, &tps_stats, &opcode_stats, &storage).await {
            tracing::error!(%error, "Actonscan indexer disconnected");
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    }
}

async fn run_connection(
    config: &IndexerConfig,
    tps_stats: &TpsStats,
    opcode_stats: &OpcodeStats,
    storage: &SqliteStorage,
) -> Result<()> {
    let mut client = TonutilsLiteClient::connect_path_with_parallelism(
        &config.global_config_path,
        config.parallelism,
    )
    .await?;
    let tip = client.latest().await?;
    tps_stats.set_startup_tip(tip.seqno).await;
    let start_seqno = tip
        .seqno
        .saturating_sub(config.backfill_batches.saturating_sub(1));
    tracing::info!(
        tip_seqno = tip.seqno,
        start_seqno,
        parallelism = client.exact_block_parallelism(),
        "connected Actonscan indexer to LiteServer"
    );

    let source = CanonicalBlockSource::new(client, start_seqno);
    let sink = StatsSink {
        tps_stats: tps_stats.clone(),
        opcode_stats: opcode_stats.clone(),
        storage: storage.clone(),
    };
    let mut pipeline = IndexPipeline::new(source, sink, storage.clone());
    loop {
        match pipeline.run_once().await? {
            RunOutcome::Idle => tokio::time::sleep(config.poll_interval).await,
            RunOutcome::Committed(checkpoint) => {
                tracing::debug!(seqno = checkpoint.seqno, "indexed Actonscan batch");
            }
        }
    }
}

struct StatsSink {
    tps_stats: TpsStats,
    opcode_stats: OpcodeStats,
    storage: SqliteStorage,
}

#[async_trait]
impl Sink for StatsSink {
    async fn commit(&mut self, batch: &Batch) -> ton_indexer_core::Result<()> {
        let tps_sample = TpsStats::sample_from_batch(batch);
        let opcode_batch =
            OpcodeBatchStats::from_batch(batch).map_err(ton_indexer_core::Error::sink)?;
        self.storage
            .record_batch_stats(tps_sample, &opcode_batch)
            .map_err(ton_indexer_core::Error::sink)?;
        self.tps_stats.record_sample(tps_sample).await;
        self.opcode_stats.record_batch(&opcode_batch).await;
        Ok(())
    }
}
