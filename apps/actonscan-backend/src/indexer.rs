use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use ton_indexer_core::{
    Batch, BlockSource, CanonicalBlockSource, CheckpointStore, IndexPipeline, RunOutcome, Sink,
};
use ton_indexer_liteserver::TonutilsLiteClient;
use ton_indexer_p2p::{P2pBlockSource, start};
use ton_p2p::{Client, ClientOptions, NetworkConfig, NetworkOptions, load_identity};

use crate::{
    SqliteStorage,
    config::{IndexerConfig, SourceKind},
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
            tracing::error!(
                operation = "indexer",
                target = ?config.source,
                error = %format!("{error:#}"),
                outcome = "reconnect",
                "Actonscan indexer disconnected",
            );
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
    if config.source == SourceKind::P2p {
        let mut network = NetworkConfig::load(&config.global_config_path)?;
        let timeout = Duration::from_secs(config.p2p.timeout_seconds);

        // A fresh P2P cache must resume the existing index, even when the user
        // enabled a recent start. Jumping to a newer ID would lose statistics.
        if let Some(checkpoint) = storage.load().await? {
            network.set_initial_block(checkpoint.try_into()?)?;
        } else if config.p2p.from_latest {
            start::use_latest_block(
                &mut network,
                &config.global_config_path,
                &config.p2p.data_dir,
                timeout,
            )
            .await?;
        }

        let client = Client::open(
            &network,
            ClientOptions {
                network: NetworkOptions {
                    address: config.p2p.address,
                    secret_key: load_identity(&config.p2p.data_dir)?,
                    timeout,
                },
                data_dir: config.p2p.data_dir.clone(),
                peers_file: config.p2p.peers_file.clone(),
                parallelism: config.p2p.parallelism,
            },
        )?;
        let source = P2pBlockSource::new(client)?;
        tps_stats.follow_recent_blocks().await;
        return run_pipeline(source, config, tps_stats, opcode_stats, storage).await;
    }

    let mut client = TonutilsLiteClient::connect_path_with_stats(
        &config.global_config_path,
        config.parallelism,
        &config.peer_stats_path,
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
        "connected Actonscan indexer to LiteServer",
    );

    let source = CanonicalBlockSource::new(client, start_seqno);
    run_pipeline(source, config, tps_stats, opcode_stats, storage).await
}

async fn run_pipeline(
    source: impl BlockSource,
    config: &IndexerConfig,
    tps_stats: &TpsStats,
    opcode_stats: &OpcodeStats,
    storage: &SqliteStorage,
) -> Result<()> {
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
                tracing::debug!(
                    operation = "indexer",
                    seqno = checkpoint.seqno,
                    outcome = "committed",
                    "indexed Actonscan batch",
                );
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
