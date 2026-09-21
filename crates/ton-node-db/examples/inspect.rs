//! Inspect an extracted database and optionally export its blocks and states.
//! Available since trunk.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde_json::{Value, json};
use ton_node_db::{NodeDb, StateRecord};
use tracing::info;
use tycho_types::boc::Boc;
use tycho_types::cell::CellBuilder;
use tycho_types::models::{AccountState, Block, BlockId, ShardStateUnsplit};

#[derive(Parser)]
#[command(about = "Inspect a stopped TON validator's database snapshot")]
struct Args {
    /// Extracted database directory containing celldb, state, and archive
    database: PathBuf,

    /// Write BoCs and report.json outside the database directory
    #[arg(long)]
    export: Option<PathBuf>,

    /// Read an exact block ID instead of inspecting the current states
    #[arg(long, requires = "export", conflicts_with = "state")]
    block: Option<BlockId>,

    /// Inspect an exact retained state instead of the shard-client checkpoint
    #[arg(long)]
    state: Option<BlockId>,

    /// Maximum database cell records reconstructed per state
    #[arg(long, default_value_t = 1_000_000)]
    max_cells: usize,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let args = Args::parse();
    if let Some(output) = &args.export {
        // Resolve existing ancestors before creating anything, so an output
        // path inside the input snapshot cannot modify it even on rejection.
        let absolute = std::path::absolute(output)?;
        let ancestor = absolute
            .ancestors()
            .find(|path| path.exists())
            .context("export path has no existing ancestor")?;
        let resolved = ancestor
            .canonicalize()?
            .join(absolute.strip_prefix(ancestor)?);
        ensure!(
            !resolved.starts_with(args.database.canonicalize()?),
            "export directory must be outside the database snapshot"
        );
        fs::create_dir_all(output)?;
    }

    let started = Instant::now();
    let db = NodeDb::open(&args.database)?;
    info!(
        operation = "database_inspect",
        target = %args.database.display(),
        outcome = "started",
        "reading validator database snapshot"
    );

    if let Some(id) = args.block {
        let bytes = db.read_block(&id, 64 * 1024 * 1024)?;
        let output = args.export.as_ref().context("missing export directory")?;
        let path = output.join("block.boc");
        fs::write(&path, &bytes)?;
        info!(
            operation = "block_read",
            target = %id,
            bytes = bytes.len(),
            duration_ms = started.elapsed().as_millis(),
            outcome = "exported",
            "block read from database snapshot"
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "block_id": id, "bytes": bytes.len(), "path": path,
            }))?
        );
        return Ok(());
    }

    let inventory = db.inspect()?;
    let records = db.states()?;
    let checkpoint = args
        .state
        .or(inventory.checkpoints.shard_client)
        .or(inventory.checkpoints.init_block)
        .context("snapshot has no block checkpoint; specify --state")?;
    let mut pending = vec![checkpoint];
    let mut states = Vec::new();

    while let Some(id) = pending.pop() {
        let record = records
            .iter()
            .find(|record| record.block_id == id)
            .with_context(|| format!("state for {id} is not retained in celldb"))?;
        let (summary, shards) = inspect_state(&args, &db, record)?;
        states.push(summary);
        pending.extend(shards.into_iter().rev());
    }

    let report = json!({ "database": inventory, "states": states });
    let report = serde_json::to_string_pretty(&report)?;
    if let Some(output) = &args.export {
        fs::write(output.join("report.json"), &report)?;
    }
    println!("{report}");

    info!(
        operation = "database_inspect",
        target = %args.database.display(),
        duration_ms = started.elapsed().as_millis(),
        outcome = "complete",
        "database snapshot inspected"
    );

    Ok(())
}

fn inspect_state(args: &Args, db: &NodeDb, record: &StateRecord) -> Result<(Value, Vec<BlockId>)> {
    let started = Instant::now();
    let id = record.block_id;
    let root = db
        .load_state(record, args.max_cells)
        .with_context(|| format!("cannot reconstruct state for {id}"))?;
    let state = root.parse::<ShardStateUnsplit>()?;

    // Match the reconstructed state to the block's Merkle update. The database
    // descriptor alone is only a local index, not a cryptographic commitment.
    let block = if id.seqno > 0 {
        let bytes = db.read_block(&id, 64 * 1024 * 1024)?;
        let update = Boc::decode(&bytes)?.parse::<Block>()?.load_state_update()?;
        ensure!(
            &update.new_hash == root.repr_hash(),
            "block state-update hash mismatch for {id}"
        );
        Some(bytes)
    } else {
        ensure!(
            root.repr_hash() == &id.root_hash,
            "zerostate root hash mismatch"
        );
        None
    };

    let prefix = format!(
        "{}-{:016x}-{}",
        id.shard.workchain(),
        id.shard.prefix(),
        id.seqno
    );
    if let Some(output) = &args.export {
        fs::write(
            output.join(format!("state-{prefix}.boc")),
            Boc::encode(&root),
        )?;
        if let Some(bytes) = block {
            fs::write(output.join(format!("block-{prefix}.boc")), bytes)?;
        }
    }

    let mut accounts = Vec::new();
    for entry in state.load_accounts()?.iter() {
        let (address, _, shard_account) = entry?;
        let Some(account) = shard_account.load_account()? else {
            continue;
        };
        let (code_hash, data_hash) = match &account.state {
            AccountState::Active(init) => {
                if let Some(output) = &args.export {
                    let cell = CellBuilder::build_from(init)?;
                    fs::write(
                        output.join(format!("state-init-{}-{address}.boc", id.shard.workchain())),
                        Boc::encode(cell),
                    )?;
                }
                (
                    init.code.as_ref().map(|cell| *cell.repr_hash()),
                    init.data.as_ref().map(|cell| *cell.repr_hash()),
                )
            }
            _ => (None, None),
        };

        let account_cell = shard_account.account.inner();
        if let Some(output) = &args.export {
            fs::write(
                output.join(format!("account-{}-{address}.boc", id.shard.workchain())),
                Boc::encode(&account_cell),
            )?;
        }

        accounts.push(json!({
            "address": account.address.to_string(),
            "status": account.state.status(),
            "balance_nano": account.balance.tokens.to_string(),
            "last_transaction_lt": shard_account.last_trans_lt,
            "last_transaction_hash": shard_account.last_trans_hash,
            "account_hash": account_cell.repr_hash(),
            "code_hash": code_hash,
            "data_hash": data_hash,
        }));
    }

    let custom = state.load_custom()?;
    let shards = match &custom {
        Some(extra) => extra
            .shards
            .latest_blocks()
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };

    info!(
        operation = "state_read",
        target = %id,
        accounts = accounts.len(),
        duration_ms = started.elapsed().as_millis(),
        outcome = "loaded",
        "shard state reconstructed from celldb"
    );

    Ok((
        json!({
            "block_id": id,
            "root_hash": root.repr_hash(),
            "global_id": state.global_id,
            "gen_utime": state.gen_utime,
            "gen_lt": state.gen_lt,
            "total_balance_nano": state.total_balance.tokens.to_string(),
            "config_address": custom.as_ref().map(|extra| extra.config.address),
            "shards": shards,
            "accounts": accounts,
        }),
        shards,
    ))
}
