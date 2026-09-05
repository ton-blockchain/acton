//! Phase 0 de-risk spike: build hardfork blocks for a real, unmodified TON node.
//!
//!   `hardfork_spike` shards --mc-state <boc>
//!       prints the masterchain header and the shard top blocks recorded in it
//!
//!   `hardfork_spike` accounts --state <boc>
//!       lists the accounts of one shard state
//!
//!   `hardfork_spike` build --mc-state <boc> --mc-prev <seqno>:<root>:<file>
//!                        [--shard-state <boc> --shard-prev <seqno>:<root>:<file>]
//!                        --account <workchain>:<hex> [--add-balance <nanotons>]
//!                        --out <dir>
//!
//!   `hardfork_spike` keygen --out <dir>
//!       writes the block source key pair and prints its public half
//!
//!   `hardfork_spike` serve --out <dir> --port <port>
//!       serves every block of <dir> as a full-node master

use anyhow::{Context, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use std::path::PathBuf;
use ton_fullnode_master::{BlockSource, ServedBlock};
use ton_liteapi::adnl::crypto::{KeyPair, SecretKey};
use ton_localnet::block::hardfork::{
    AccountWrite, AdminBatch, HardforkPrevBlock, HardforkSources, ShardSource, build_hardfork,
};
use tycho_types::boc::Boc;
use tycho_types::cell::Lazy;
use tycho_types::models::account::{
    Account, AccountState, OptionalAccount, ShardAccount, StorageExtra, StorageInfo, StorageUsed,
};
use tycho_types::models::block::{BlockId, ShardIdent};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::message::{IntAddr, StdAddr};
use tycho_types::models::shard::ShardStateUnsplit;
use tycho_types::num::Tokens;
use tycho_types::prelude::HashBytes;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args
        .next()
        .context("expected: shards | accounts | build | keygen | serve")?;
    let mut flags = Flags::default();
    while let Some(flag) = args.next() {
        let mut value = || args.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--state" | "--mc-state" => flags.mc_state = Some(PathBuf::from(value()?)),
            "--mc-prev" => flags.mc_prev = Some(value()?),
            "--shard-state" => flags.shard_state = Some(PathBuf::from(value()?)),
            "--shard-prev" => flags.shard_prev = Some(value()?),
            "--account" => flags.accounts.push(value()?),
            "--add-balance" => flags.add_balance = value()?.parse()?,
            "--out" => flags.out = PathBuf::from(value()?),
            "--port" => flags.port = value()?.parse()?,
            other => bail!("unknown flag {other}"),
        }
    }

    match command.as_str() {
        "config" => config_params(&flags),
        "shards" => shards(&flags),
        "accounts" => accounts(&flags),
        "build" => build(&flags),
        "keygen" => keygen(&flags),
        "serve" => serve(&flags),
        other => bail!("unknown command {other}"),
    }
}

/// Writes the block source identity used by the `fullnodeslaves` config entry.
fn keygen(flags: &Flags) -> anyhow::Result<()> {
    std::fs::create_dir_all(&flags.out)?;
    let mut secret = [0u8; 32];
    getrandom(&mut secret)?;
    let keypair = KeyPair::from(&SecretKey::from_bytes(secret));
    std::fs::write(flags.out.join("source.key"), hex::encode(secret))?;
    println!(
        "{}",
        serde_json::json!({
            "public_key_base64": STANDARD.encode(keypair.public_key.as_bytes()),
        })
    );
    Ok(())
}

/// Serves every block built into the output directory over ADNL-over-TCP.
fn serve(flags: &Flags) -> anyhow::Result<()> {
    let secret: [u8; 32] =
        hex::decode(std::fs::read_to_string(flags.out.join("source.key"))?.trim())?
            .try_into()
            .map_err(|_| anyhow::anyhow!("block source key must be 32 bytes"))?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(flags.out.join("fork.json"))?)?;

    let source = BlockSource::new();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        for entry in manifest["static_files"].as_array().context("no blocks")? {
            let path = PathBuf::from(entry["path"].as_str().context("no path")?);
            let block_id = BlockId {
                shard: ShardIdent::new(
                    entry["workchain"].as_i64().context("no workchain")? as i32,
                    u64::from_str_radix(entry["shard"].as_str().context("no shard")?, 16)?,
                )
                .context("invalid shard")?,
                seqno: entry["seqno"].as_u64().context("no seqno")? as u32,
                root_hash: parse_hash(entry["root_hash"].as_str().context("no root hash")?)?,
                file_hash: parse_hash(entry["file_hash"].as_str().context("no file hash")?)?,
            };
            println!("serving {block_id}");
            source
                .insert(
                    &block_id,
                    ServedBlock {
                        data: std::fs::read(&path)?,
                        proof_link: std::fs::read(path.with_extension("proof"))?,
                    },
                )
                .await;
        }
        println!("listening on 127.0.0.1:{}", flags.port);
        source
            .serve(([127, 0, 0, 1], flags.port).into(), secret)
            .await
    })
}

struct Flags {
    mc_state: Option<PathBuf>,
    mc_prev: Option<String>,
    shard_state: Option<PathBuf>,
    shard_prev: Option<String>,
    accounts: Vec<String>,
    add_balance: u128,
    out: PathBuf,
    port: u16,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            mc_state: None,
            mc_prev: None,
            shard_state: None,
            shard_prev: None,
            accounts: Vec::new(),
            add_balance: 0,
            out: PathBuf::from("."),
            port: 4499,
        }
    }
}

impl Flags {
    fn state(&self) -> anyhow::Result<(tycho_types::cell::Cell, ShardStateUnsplit)> {
        let path = self.mc_state.as_ref().context("--state is required")?;
        let cell = Boc::decode(std::fs::read(path)?)?;
        let state = cell.parse::<ShardStateUnsplit>()?;
        Ok((cell, state))
    }
}

/// Reports which configuration parameters this tycho build can type-check.
fn config_params(flags: &Flags) -> anyhow::Result<()> {
    use tycho_types::models::config::{
        ConfigParam0, ConfigParam1, ConfigParam2, ConfigParam7, ConfigParam8, ConfigParam9,
        ConfigParam10, ConfigParam11, ConfigParam12, ConfigParam13, ConfigParam14, ConfigParam15,
        ConfigParam16, ConfigParam17, ConfigParam18, ConfigParam20, ConfigParam21, ConfigParam22,
        ConfigParam23, ConfigParam24, ConfigParam25, ConfigParam28, ConfigParam29, ConfigParam30,
        ConfigParam31, ConfigParam32, ConfigParam34,
    };

    let (_, state) = flags.state()?;
    let params = state
        .custom
        .as_ref()
        .context("not a masterchain state")?
        .load()?
        .config
        .params;

    macro_rules! probe {
        ($($id:literal => $ty:ty),* $(,)?) => {
            for (index, present, parsed) in [$((
                $id,
                params.get_raw($id).map(|v| v.is_some()).unwrap_or(false),
                params.get::<$ty>().map(|_| true).unwrap_or(false),
            )),*] {
                if present {
                    println!("{index}\t{}", if parsed { "ok" } else { "FAILS" });
                }
            }
        };
    }
    probe!(
        0 => ConfigParam0, 1 => ConfigParam1, 2 => ConfigParam2, 7 => ConfigParam7,
        8 => ConfigParam8, 9 => ConfigParam9, 10 => ConfigParam10, 11 => ConfigParam11,
        12 => ConfigParam12, 13 => ConfigParam13, 14 => ConfigParam14, 15 => ConfigParam15,
        16 => ConfigParam16, 17 => ConfigParam17, 18 => ConfigParam18, 20 => ConfigParam20,
        21 => ConfigParam21, 22 => ConfigParam22, 23 => ConfigParam23, 24 => ConfigParam24,
        25 => ConfigParam25, 28 => ConfigParam28, 29 => ConfigParam29, 30 => ConfigParam30,
        31 => ConfigParam31, 32 => ConfigParam32, 34 => ConfigParam34,
    );
    Ok(())
}

fn shards(flags: &Flags) -> anyhow::Result<()> {
    let (_, state) = flags.state()?;
    let extra = state
        .custom
        .as_ref()
        .context("not a masterchain state")?
        .load()?;
    let mut out = serde_json::Map::new();
    out.insert("seqno".into(), state.seqno.into());
    out.insert("vert_seqno".into(), state.vert_seqno.into());
    out.insert("gen_lt".into(), state.gen_lt.into());
    let mut shards = Vec::new();
    for entry in extra.shards.iter() {
        let (ident, descr) = entry?;
        shards.push(serde_json::json!({
            "workchain": ident.workchain(),
            "shard": format!("{:016x}", ident.prefix()),
            "seqno": descr.seqno,
            "root_hash": hex::encode(descr.root_hash.0),
            "file_hash": hex::encode(descr.file_hash.0),
        }));
    }
    out.insert("shards".into(), shards.into());
    println!("{}", serde_json::Value::Object(out));
    Ok(())
}

fn accounts(flags: &Flags) -> anyhow::Result<()> {
    let (_, state) = flags.state()?;
    println!(
        "shard={} seqno={} vert_seqno={} total_balance={}",
        state.shard_ident, state.seqno, state.vert_seqno, state.total_balance.tokens
    );
    for entry in state.accounts.load()?.iter() {
        let (address, depth, shard_account) = entry?;
        let status =
            shard_account
                .load_account()?
                .map_or("nonexist", |account| match account.state {
                    AccountState::Active(_) => "active",
                    AccountState::Uninit => "uninit",
                    AccountState::Frozen(_) => "frozen",
                });
        println!("{address} {status} balance={}", depth.balance.tokens);
    }
    Ok(())
}

fn build(flags: &Flags) -> anyhow::Result<()> {
    let (mc_state_cell, mc_state) = flags.state()?;
    let masterchain_prev = parse_prev(flags.mc_prev.as_ref().context("--mc-prev is required")?)?;

    let basechain = match (&flags.shard_state, &flags.shard_prev) {
        (Some(path), Some(prev)) => {
            let cell = Boc::decode(std::fs::read(path)?)?;
            let shard = cell.parse::<ShardStateUnsplit>()?.shard_ident;
            Some(ShardSource {
                shard,
                state: cell,
                prev: parse_prev(prev)?,
            })
        }
        (None, None) => None,
        _ => bail!("--shard-state and --shard-prev must be given together"),
    };

    if flags.accounts.is_empty() {
        bail!("--account is required");
    }
    let mut batch = AdminBatch::default();
    for account in &flags.accounts {
        let (workchain, address) = account
            .split_once(':')
            .context("--account must be <workchain>:<hex>")?;
        let workchain: i32 = workchain.parse()?;
        let address = parse_hash(address)?;

        let shard_state;
        let source_state = if workchain == ShardIdent::MASTERCHAIN.workchain() {
            &mc_state
        } else {
            shard_state = basechain
                .as_ref()
                .context("--shard-state is required for a basechain account")?
                .state
                .parse::<ShardStateUnsplit>()?;
            &shard_state
        };

        // An absent account is created as an uninitialized one, which is what
        // "fund this address out of nowhere" means for a chain with no faucet.
        let existing = source_state.accounts.load()?.get(address)?;
        let (mut inner, last_trans_hash, last_trans_lt) = match &existing {
            Some((_, shard_account)) => (
                shard_account
                    .load_account()?
                    .context("target account record is empty")?,
                shard_account.last_trans_hash,
                shard_account.last_trans_lt,
            ),
            None => (
                Account {
                    address: IntAddr::Std(StdAddr::new(workchain as i8, address)),
                    storage_stat: StorageInfo {
                        used: StorageUsed::ZERO,
                        storage_extra: StorageExtra::None,
                        last_paid: 0,
                        due_payment: None,
                    },
                    last_trans_lt: 0,
                    balance: CurrencyCollection::ZERO,
                    state: AccountState::Uninit,
                },
                HashBytes::ZERO,
                0,
            ),
        };
        inner.balance.tokens = inner
            .balance
            .tokens
            .checked_add(Tokens::new(flags.add_balance))
            .context("balance overflow")?;

        let write = AccountWrite::set(
            address,
            ShardAccount {
                account: Lazy::new(&OptionalAccount(Some(inner)))?,
                last_trans_hash,
                last_trans_lt,
            },
        );
        if workchain == ShardIdent::MASTERCHAIN.workchain() {
            batch.masterchain.push(write);
        } else {
            batch.basechain.push(write);
        }
    }

    let gen_utime = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;
    let plan = build_hardfork(
        &HardforkSources {
            masterchain_state: mc_state_cell,
            masterchain_prev,
            basechain,
        },
        gen_utime,
        &batch,
    )?;

    std::fs::create_dir_all(&flags.out)?;
    // The installer on the node side consumes this shape.
    let planned = |block: &ton_localnet::block::hardfork::HardforkBlock| {
        serde_json::json!({
            "workchain": block.shard.workchain(),
            "shard": block.shard.prefix(),
            "seqno": block.seqno,
            "root_hash": STANDARD.encode(block.root_hash.0),
            "file_hash": STANDARD.encode(block.file_hash.0),
            "block": STANDARD.encode(&block.block_boc),
            "proof": STANDARD.encode(&block.proof_link),
        })
    };
    std::fs::write(
        flags.out.join("plan.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "masterchain": planned(&plan.masterchain),
            "shard_blocks": plan.basechain.iter().map(planned).collect::<Vec<_>>(),
        }))?,
    )?;
    let mut files = Vec::new();
    for block in plan.static_blocks() {
        let path = flags.out.join(block.static_file_name());
        std::fs::write(&path, &block.block_boc)?;
        std::fs::write(path.with_extension("proof"), &block.proof_link)?;
        files.push(serde_json::json!({
            "workchain": block.shard.workchain(),
            "shard": format!("{:016x}", block.shard.prefix()),
            "seqno": block.seqno,
            "root_hash": hex::encode(block.root_hash.0),
            "file_hash": hex::encode(block.file_hash.0),
            "path": path,
            "bytes": block.block_boc.len(),
            "proof_bytes": block.proof_link.len(),
        }));
    }
    println!(
        "{}",
        serde_json::json!({
            "vert_seqno": plan.vert_seqno,
            "seqno": plan.masterchain.seqno,
            "root_hash": hex::encode(plan.masterchain.root_hash.0),
            "file_hash": hex::encode(plan.masterchain.file_hash.0),
            "state_root_hash": hex::encode(plan.masterchain.state_root_hash.0),
            "static_files": files,
        })
    );
    Ok(())
}

fn parse_prev(value: &str) -> anyhow::Result<HardforkPrevBlock> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        bail!("block reference must be <seqno>:<root_hash>:<file_hash>");
    }
    Ok(HardforkPrevBlock {
        seqno: parts[0].parse()?,
        root_hash: parse_hash(parts[1])?,
        file_hash: parse_hash(parts[2])?,
    })
}

/// Fills a buffer with operating-system randomness.
fn getrandom(buffer: &mut [u8; 32]) -> anyhow::Result<()> {
    use std::io::Read;
    std::fs::File::open("/dev/urandom")?.read_exact(buffer)?;
    Ok(())
}

fn parse_hash(value: &str) -> anyhow::Result<HashBytes> {
    let bytes = hex::decode(value).context("expected a hex hash")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("hash must be 32 bytes"))?;
    Ok(HashBytes(bytes))
}
