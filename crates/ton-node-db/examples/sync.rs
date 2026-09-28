//! Resume persistent masterchain and shard states from a validator snapshot.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use rston::models::BlockId;
use serde_json::json;
use ton_indexer_core::BlockSource;
use ton_indexer_p2p::P2pBlockSource;
use ton_node_db::StateStore;
use ton_p2p::{Client, ClientOptions, NetworkConfig, NetworkOptions, load_identity};
use tracing::info;

#[derive(Parser)]
struct Args {
    /// Immutable database from a stopped node or consistent backup
    database: PathBuf,

    /// Global config of the network that produced the snapshot
    global_config: PathBuf,

    /// Persistent state updates and P2P cache, outside the snapshot
    data_dir: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_ansi(false)
        .init();

    let args = Args::parse();
    let started = Instant::now();
    let mut config = NetworkConfig::load(&args.global_config)?;
    let mut states = StateStore::open(&args.database, &args.data_dir.join("states"), 1_000_000)?;
    let initial = states.head();
    config.set_initial_block(initial)?;
    let downloads = args.data_dir.join("blocks");
    let client = Client::open(
        &config,
        ClientOptions {
            network: NetworkOptions {
                address: "127.0.0.1:19005".parse()?,
                secret_key: load_identity(&downloads)?,
                timeout: Duration::from_secs(5),
            },
            data_dir: downloads,
            peers_file: None,
            parallelism: 16,
        },
    )?;
    client.validate_checkpoint(&initial)?;
    let mut source = P2pBlockSource::new(client)?;
    let mut applied = 0_u32;

    info!(
        operation = "state_sync",
        target = %initial,
        data_dir = %args.data_dir.display(),
        duration_ms = started.elapsed().as_millis(),
        outcome = "started",
        "resuming from the applied state checkpoint",
    );

    let sync = async {
        while applied < 10 {
            let after = states.head().into();
            let Some(batch) = source.next_batch(Some(&after)).await? else {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            };
            let master_id: BlockId = batch.masterchain().id().try_into()?;
            let (_, master_boc) = source
                .client_mut()
                .masterchain_block(master_id.seqno)
                .await?
                .context("downloaded masterchain block is absent from cache")?;
            let shard_ids = batch
                .shards()
                .iter()
                .map(|block| block.id().try_into())
                .collect::<Result<Vec<BlockId>, _>>()?;
            let shard_bocs = source
                .client_mut()
                .download_shards(&shard_ids)
                .await?
                .into_iter()
                .map(|boc| boc.context("downloaded shard block is absent from cache"))
                .collect::<Result<Vec<_>>>()?;
            let commit_started = Instant::now();
            states.apply_batch(
                (master_id, &master_boc),
                shard_ids
                    .iter()
                    .copied()
                    .zip(shard_bocs.iter().map(Vec::as_slice)),
            )?;
            applied += 1;

            info!(
                operation = "state_commit",
                target = %states.head(),
                shard_blocks = shard_ids.len(),
                applied_blocks = applied,
                duration_ms = commit_started.elapsed().as_millis(),
                outcome = "committed",
                "persisted masterchain and shard states",
            );
        }

        anyhow::Ok(())
    };
    tokio::time::timeout(Duration::from_secs(120), sync)
        .await
        .context("state synchronization timed out; restart to resume")??;

    let elector = states.masterchain_state()?.elector_address()?;
    let account = states.get_account(&elector)?;
    info!(
        operation = "state_sync",
        target = %states.head(),
        applied_blocks = applied,
        duration_ms = started.elapsed().as_millis(),
        outcome = "completed",
        "saved state synchronization checkpoint",
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "initial_block": initial,
            "block": states.head(),
            "applied_blocks": applied,
            "duration_ms": started.elapsed().as_millis(),
            "elector": account.account,
        }))?
    );

    Ok(())
}
