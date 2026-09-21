//! Advance a Localton database snapshot over P2P and read its elector account.
//! Available since trunk.

use std::net::SocketAddrV4;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde_json::json;
use ton_node_db::NodeDb;
use ton_p2p::{Client, ClientOptions, NetworkConfig, NetworkOptions, load_identity};
use tracing::info;

#[derive(Parser)]
struct Args {
    /// Immutable database from a stopped node or consistent backup
    database: PathBuf,

    /// Global config of the same Localton network
    #[arg(long)]
    global_config: PathBuf,

    /// Writable P2P cache outside the database snapshot
    #[arg(long, default_value = ".elector-p2p")]
    data_dir: PathBuf,

    /// Advertised UDP address reachable by the Localton node
    #[arg(long, default_value = "127.0.0.1:19005")]
    address: SocketAddrV4,

    /// Number of successor blocks to apply after the snapshot checkpoint
    #[arg(long, default_value_t = 10)]
    blocks: u32,

    /// Overall P2P download deadline in seconds
    #[arg(long, default_value_t = 120)]
    timeout: u64,

    /// Maximum database cell records read across all updates and queries
    #[arg(long, default_value_t = 1_000_000)]
    max_cells: usize,
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
    let db = NodeDb::open(&args.database)?;
    let checkpoints = db.checkpoints()?;
    let checkpoint = checkpoints
        .shard_client
        .or(checkpoints.init_block)
        .context("database has no masterchain checkpoint")?;
    let mut state = db.state(&checkpoint, args.max_cells)?;
    let before_address = state.elector_address()?;
    let before = state
        .get_account(&before_address)?
        .context("elector is absent from snapshot")?;
    let before_hash = *before.account.inner().repr_hash();

    let mut config = NetworkConfig::load(&args.global_config)?;
    db.state_record(&config.zero_state())
        .context("snapshot does not retain this network's zerostate")?;
    config.set_initial_block(checkpoint)?;

    // A misplaced writable cache must not change the snapshot being queried.
    let absolute = std::path::absolute(&args.data_dir)?;
    let ancestor = absolute
        .ancestors()
        .find(|path| path.exists())
        .context("cache path has no existing ancestor")?;
    let resolved = ancestor
        .canonicalize()?
        .join(absolute.strip_prefix(ancestor)?);
    ensure!(
        !resolved.starts_with(args.database.canonicalize()?),
        "P2P cache must be outside the database snapshot"
    );

    let mut client = Client::open(
        &config,
        ClientOptions {
            network: NetworkOptions {
                address: args.address,
                secret_key: load_identity(&args.data_dir)?,
                timeout: Duration::from_secs(5),
            },
            data_dir: args.data_dir,
            peers_file: None,
            parallelism: 8,
        },
    )?;
    client.validate_checkpoint(&checkpoint)?;
    let target = checkpoint
        .seqno
        .checked_add(args.blocks)
        .context("sequence number overflow")?;

    tokio::time::timeout(Duration::from_secs(args.timeout), async {
        while state.block_id().seqno < target {
            let next = state.block_id().seqno + 1;
            if let Some((id, bytes)) = client.masterchain_block(next).await? {
                state.apply_masterchain_block(&id, &bytes)?;
                info!(
                    operation = "elector_state_sync",
                    target = %id,
                    records_read = state.read_stats().records,
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "applied",
                    "applied masterchain state update"
                );
            } else if client.next_masterchain().await?.is_none() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        anyhow::Ok(())
    })
    .await
    .context("timed out waiting for Localton masterchain blocks")??;

    let address = state.elector_address()?;
    let account = state
        .get_account(&address)?
        .context("elector is absent from updated state")?;
    info!(
        operation = "elector_state_sync",
        target = %address,
        block = %state.block_id(),
        records_read = state.read_stats().records,
        duration_ms = started.elapsed().as_millis(),
        outcome = "completed",
        "read elector from updated masterchain state"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "address": address.to_string(),
            "snapshot_block": checkpoint,
            "block": state.block_id(),
            "applied_blocks": args.blocks,
            "snapshot_account_hash": before_hash,
            "account_hash": account.account.inner().repr_hash(),
            "reads": state.read_stats(),
            "duration_ms": started.elapsed().as_millis(),
            "account": account,
        }))?
    );

    Ok(())
}
