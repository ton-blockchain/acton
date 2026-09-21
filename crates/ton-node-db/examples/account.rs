//! Read accounts without scanning packages or reconstructing entire states.
//! Available since trunk.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use serde_json::json;
use ton_node_db::NodeDb;
use tycho_types::models::{BlockId, StdAddr};

#[derive(Parser)]
struct Args {
    /// Extracted, immutable validator database
    database: PathBuf,

    /// Raw workchain:hex addresses
    #[arg(required = true)]
    addresses: Vec<StdAddr>,

    /// Exact masterchain checkpoint; defaults to the shard-client checkpoint
    #[arg(long)]
    block: Option<BlockId>,

    /// Maximum database cell records read for each address
    #[arg(long, default_value_t = 100_000)]
    max_cells: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let db = NodeDb::open(&args.database)?;
    let checkpoint = match args.block {
        Some(id) => id,
        None => {
            let checkpoints = db.checkpoints()?;
            checkpoints
                .shard_client
                .or(checkpoints.init_block)
                .context("database has no masterchain checkpoint; pass --block")?
        }
    };

    for address in args.addresses {
        let started = Instant::now();
        let snapshot = db.get_account(&checkpoint, &address, args.max_cells)?;
        let elapsed = started.elapsed();
        let account_hash = snapshot
            .account
            .as_ref()
            .map(|account| *account.account.inner().repr_hash());

        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "address": address.to_string(),
                "masterchain_block": snapshot.masterchain_block,
                "shard_block": snapshot.shard_block,
                "reads": snapshot.reads,
                "duration_ms": elapsed.as_millis(),
                "duration_us": elapsed.as_micros(),
                "account_hash": account_hash,
                "account": snapshot.account,
            }))?
        );
    }

    Ok(())
}
