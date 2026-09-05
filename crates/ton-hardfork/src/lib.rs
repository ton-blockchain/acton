//! Hardfork block construction for networks of unmodified TON nodes.
//!
//! A hardfork block is the only way to change state in a network of stock
//! `validator-engine` nodes. The node never re-executes such a block: it takes
//! the block's Merkle state update and applies it to the previous state verbatim
//! (`validator/impl/accept-block.cpp`, `ShardStateQ::apply_block`). Everything an
//! administrator wants to change therefore has to be baked into the state update
//! by this module.
//!
//! What the node *does* still enforce is structure. The masterchain block must
//! declare `vert_seqno_incr`, must be a key block, and must carry a
//! `McBlockExtra` with the full config; the Merkle update's old hash must match
//! the previous state root exactly. On top of that, the resulting state has to be
//! good enough for the *next* ordinary block, which unmodified collators and
//! validators check in full. The new state is therefore derived from the real
//! previous state and mirrors `Collator::create_mc_state_extra` rather than being
//! assembled from scratch.
//!
//! Basechain state cannot be forked on its own: `accept-block.cpp:392` refuses a
//! non-masterchain fork block outright. A basechain change is instead a *pair* of
//! blocks — a basechain block plus a masterchain key block whose `ShardHashes`
//! point at it. The shard block is never "accepted"; the shard client applies it
//! on the authority of the masterchain state, reading its data from
//! an authenticated local full-node block source.
//!
//! Every action of one administrative request is applied to a single pair of
//! blocks. The coordinator stops and restarts the nodes to suspend validation,
//! install the fork and restore ordinary networking.

pub mod account_blocks;
pub mod proof;
pub mod request;

use anyhow::{Context, bail, ensure};
use rustc_hash::FxHashSet;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, Lazy, LazyExotic};
use tycho_types::merkle::{FilterAction, MerkleFilter, MerkleUpdate};
use tycho_types::models::account::ShardAccount;
use tycho_types::models::block::{
    Block, BlockExtra, BlockId, BlockInfo, BlockRef, McBlockExtra, PrevBlockRef, ShardHashes,
    ShardIdent, ValueFlow,
};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::shard::{
    DepthBalanceInfo, KeyBlockRef, KeyMaxLt, McStateExtra, ShardAccounts, ShardStateUnsplit,
};
use tycho_types::prelude::HashBytes;

use crate::account_blocks::{ExecutedTransaction, build_account_blocks_from};

/// Logical time granularity every TON block start time is aligned to.
///
/// Mirrors `block::Config::get_lt_align()`. Collators of the following blocks
/// derive their own `start_lt` from the state produced here, so a hardfork block
/// has to respect the same alignment.
const LT_ALIGN: u64 = 1_000_000;

/// Identity of the block a hardfork block is grafted onto.
///
/// The node truncates its database to `seqno` and applies the hardfork as the
/// next block, so these fields have to describe a block the node really has:
/// they end up verbatim in `prev_ref`, `prev_vert_ref` and in the `prev_blocks`
/// dictionary of the new masterchain state.
///
/// The seqno must be the node's current top block. A lower one makes the node
/// truncate its database, and `StateDb::truncate` repairs only the `last` field
/// of its persistent-state serializer position, never `block` — the stale
/// position then aborts the node on every subsequent start.
#[derive(Debug, Clone)]
pub struct HardforkPrevBlock {
    /// Sequence number of the block the hardfork is built on top of.
    pub seqno: u32,
    /// Representation hash of the previous block root cell.
    pub root_hash: HashBytes,
    /// SHA-256 of the serialized previous block `BoC`.
    pub file_hash: HashBytes,
}

/// One account change to bake into a hardfork block.
#[derive(Debug, Clone)]
pub struct AccountWrite {
    /// Account id inside its shard.
    pub address: HashBytes,
    /// Replacement account record, or `None` to delete the account.
    pub account: Option<Box<ShardAccount>>,
    /// Transaction to record for this account, when the change is the result of
    /// executing a message rather than a plain state overwrite.
    ///
    /// A fork block is never re-executed, so the transaction is taken at face
    /// value; recording it is what makes the change visible to indexers and to
    /// `getTransactions` instead of appearing as an unexplained state jump.
    pub transaction: Option<RecordedTransaction>,
}

impl AccountWrite {
    /// Replaces one account without recording a transaction.
    #[must_use]
    pub fn set(address: HashBytes, account: ShardAccount) -> Self {
        Self {
            address,
            account: Some(Box::new(account)),
            transaction: None,
        }
    }

    /// Deletes one account.
    #[must_use]
    pub const fn remove(address: HashBytes) -> Self {
        Self {
            address,
            account: None,
            transaction: None,
        }
    }

    /// Records a transaction alongside the account change.
    #[must_use]
    pub fn with_transaction(mut self, transaction: RecordedTransaction) -> Self {
        self.transaction = Some(transaction);
        self
    }
}

/// A transaction the caller executed and wants the fork block to contain.
///
/// The logical time has to lie inside the block's logical time window, which
/// [`logical_time_window`] returns for the same sources.
#[derive(Debug, Clone)]
pub struct RecordedTransaction {
    /// Exact serialized `Transaction` cell.
    pub cell: Cell,
    /// Logical time of the transaction.
    pub lt: u64,
    /// Fees charged by the transaction.
    pub total_fees: CurrencyCollection,
    /// Account-state hash before the transaction.
    pub old_state_hash: HashBytes,
    /// Account-state hash after the transaction.
    pub new_state_hash: HashBytes,
}

/// Account changes of one administrative request, grouped by chain.
///
/// Both groups are applied to the same pair of blocks so that any number of
/// changes can share one coordinated operation.
#[derive(Debug, Clone, Default)]
pub struct AdminBatch {
    /// Changes to masterchain accounts.
    pub masterchain: Vec<AccountWrite>,
    /// Changes to basechain accounts.
    pub basechain: Vec<AccountWrite>,
}

impl AdminBatch {
    /// Returns whether the batch would not change anything.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.masterchain.is_empty() && self.basechain.is_empty()
    }
}

/// Live chain data a hardfork is built from.
///
/// The states must be complete cell trees of exactly those blocks, because the
/// Merkle updates are computed against them; a partial or virtualized state
/// produces an update the node refuses to apply.
pub struct HardforkSources {
    /// Complete masterchain state of `masterchain_prev`.
    pub masterchain_state: Cell,
    /// Top masterchain block of the (stopped) node.
    pub masterchain_prev: HardforkPrevBlock,
    /// Basechain shard, required whenever the batch touches basechain accounts.
    pub basechain: Option<ShardSource>,
}

/// Live basechain data a hardfork is built from.
pub struct ShardSource {
    /// Shard being forked. Localnet-style networks run a single full basechain shard.
    pub shard: ShardIdent,
    /// Complete shard state of `prev`.
    pub state: Cell,
    /// Top block of that shard, as recorded in the masterchain state.
    pub prev: HardforkPrevBlock,
}

/// One built hardfork block and the state it produces.
///
/// `block_boc` has to be written to `<db>/static/<FILE_HASH>` on every node.
/// Only the masterchain block is additionally registered in the `hardforks`
/// section of the global config; the shard block is reached through the
/// masterchain state and read from the static directory on demand.
#[derive(Debug, Clone)]
pub struct HardforkBlock {
    /// Shard this block belongs to.
    pub shard: ShardIdent,
    /// Serialized `Block` root cell.
    pub block_boc: Vec<u8>,
    /// Representation hash of the block root cell.
    pub root_hash: HashBytes,
    /// SHA-256 of `block_boc`, and the name of its file in `db/static`.
    pub file_hash: HashBytes,
    /// Sequence number of the block.
    pub seqno: u32,
    /// Serialized post-fork state root cell.
    pub state_boc: Vec<u8>,
    /// Representation hash of the post-fork state root cell.
    pub state_root_hash: HashBytes,
    /// Serialized `BlockProof` link, needed when the block is served over the
    /// network instead of being read from `db/static`.
    pub proof_link: Vec<u8>,
}

impl HardforkBlock {
    /// Returns the file name this block must have inside `db/static`.
    ///
    /// `StaticFilesDb::load_file` looks the block up by the uppercase hex of its
    /// file hash, with no extension.
    #[must_use]
    pub fn static_file_name(&self) -> String {
        hex::encode_upper(self.file_hash.0)
    }

    /// Returns the full identifier other nodes use to ask for this block.
    #[must_use]
    pub const fn block_id(&self) -> BlockId {
        BlockId {
            shard: self.shard,
            seqno: self.seqno,
            root_hash: self.root_hash,
            file_hash: self.file_hash,
        }
    }
}

/// The complete set of blocks one administrative request produces.
#[derive(Debug, Clone)]
pub struct HardforkPlan {
    /// Masterchain key block; the one registered in `validator.hardforks`.
    pub masterchain: HardforkBlock,
    /// Basechain block referenced by the masterchain block, when the batch
    /// changed basechain accounts.
    pub basechain: Option<HardforkBlock>,
    /// Vertical sequence number both blocks declare.
    pub vert_seqno: u32,
}

impl HardforkPlan {
    /// Returns every block that has to be present in `db/static`.
    pub fn static_blocks(&self) -> impl Iterator<Item = &HardforkBlock> {
        std::iter::once(&self.masterchain).chain(self.basechain.iter())
    }
}

/// Returns the first logical time the blocks of this hardfork will use.
///
/// Transactions recorded through [`RecordedTransaction`] must use logical times
/// at or after this value, because a block may only contain transactions inside
/// its own `start_lt..end_lt` range.
pub fn logical_time_window(sources: &HardforkSources) -> anyhow::Result<u64> {
    let mc_state = sources
        .masterchain_state
        .parse::<ShardStateUnsplit>()
        .context("Failed to parse previous masterchain state")?;
    let mut base_lt = mc_state.gen_lt;
    if let Some(shard) = &sources.basechain {
        let shard_state = shard
            .state
            .parse::<ShardStateUnsplit>()
            .context("Failed to parse previous basechain state")?;
        base_lt = base_lt.max(shard_state.gen_lt);
    }
    align_lt(base_lt)
}

/// Builds the hardfork blocks that apply `batch` to the live chain in `sources`.
pub fn build_hardfork(
    sources: &HardforkSources,
    gen_utime: u32,
    batch: &AdminBatch,
) -> anyhow::Result<HardforkPlan> {
    if batch.is_empty() {
        bail!("hardfork batch is empty");
    }

    let old_mc_state = sources
        .masterchain_state
        .parse::<ShardStateUnsplit>()
        .context("Failed to parse previous masterchain state")?;
    if old_mc_state.shard_ident != ShardIdent::MASTERCHAIN {
        bail!("hardfork source state is not a masterchain state");
    }
    if old_mc_state.seqno != sources.masterchain_prev.seqno {
        bail!(
            "masterchain state seqno {} does not match the previous block seqno {}",
            old_mc_state.seqno,
            sources.masterchain_prev.seqno
        );
    }
    let old_mc_extra = old_mc_state
        .custom
        .as_ref()
        .context("Masterchain state has no McStateExtra")?
        .load()
        .context("Failed to load McStateExtra")?;

    if let Some(source) = &sources.basechain {
        ensure!(
            source.shard == ShardIdent::BASECHAIN,
            "Only an unsplit basechain shard is supported"
        );
        let mut found = false;
        for entry in old_mc_extra.shards.iter() {
            let (shard, descr) = entry?;
            ensure!(
                shard == ShardIdent::BASECHAIN,
                "Split and multiple workchains are not supported"
            );
            if shard == source.shard {
                ensure!(
                    descr.seqno == source.prev.seqno
                        && descr.root_hash == source.prev.root_hash
                        && descr.file_hash == source.prev.file_hash,
                    "Basechain source does not match the masterchain shard descriptor"
                );
                found = true;
            }
        }
        ensure!(
            found,
            "Basechain source is not referenced by the masterchain"
        );
    }

    let vert_seqno = old_mc_state
        .vert_seqno
        .checked_add(1)
        .context("Vertical seqno overflow")?;
    let gen_utime = gen_utime.max(
        old_mc_state
            .gen_utime
            .checked_add(1)
            .context("Timestamp overflow")?,
    );

    // Both blocks share one logical time window. The window has to start after
    // everything the previous masterchain block and its shards used, so that the
    // next collator's `init_lt` keeps moving forward.
    let mut base_lt = old_mc_state.gen_lt;
    if let Some(shard) = &sources.basechain {
        let shard_state = shard
            .state
            .parse::<ShardStateUnsplit>()
            .context("Failed to parse previous basechain state")?;
        base_lt = base_lt.max(shard_state.gen_lt);
    }
    let start_lt = align_lt(base_lt)?;
    let end_lt = batch
        .masterchain
        .iter()
        .chain(&batch.basechain)
        .filter_map(|write| write.transaction.as_ref())
        .try_fold(start_lt + 1, |end, tx| {
            if tx.lt < start_lt {
                bail!(
                    "recorded transaction has logical time {} before the block window start {start_lt}",
                    tx.lt
                );
            }
            Ok(end.max(tx.lt + 1))
        })?;

    let mc_prev_ref = BlockRef {
        end_lt: old_mc_state.gen_lt,
        seqno: sources.masterchain_prev.seqno,
        root_hash: sources.masterchain_prev.root_hash,
        file_hash: sources.masterchain_prev.file_hash,
    };

    // The shard block has to exist before the masterchain block, because the
    // masterchain state records its id in `ShardHashes`.
    let basechain = match &sources.basechain {
        Some(shard) if !batch.basechain.is_empty() => Some(build_shard_block(
            shard,
            &mc_prev_ref,
            &old_mc_state,
            vert_seqno,
            gen_utime,
            start_lt,
            end_lt,
            &batch.basechain,
        )?),
        _ if !batch.basechain.is_empty() => {
            bail!("batch changes basechain accounts but no basechain shard was provided")
        }
        _ => None,
    };

    let masterchain = build_masterchain_block(
        &sources.masterchain_state,
        &old_mc_state,
        &old_mc_extra,
        &mc_prev_ref,
        vert_seqno,
        gen_utime,
        start_lt,
        end_lt,
        &batch.masterchain,
        basechain.as_ref(),
    )?;

    Ok(HardforkPlan {
        masterchain,
        basechain: basechain.map(|shard| shard.block),
        vert_seqno,
    })
}

/// A built basechain block together with what the masterchain block needs from it.
struct BuiltShardBlock {
    block: HardforkBlock,
    shard: ShardIdent,
    start_lt: u64,
    end_lt: u64,
    gen_utime: u32,
    min_ref_mc_seqno: u32,
}

#[allow(clippy::too_many_arguments)]
fn build_shard_block(
    source: &ShardSource,
    mc_prev_ref: &BlockRef,
    old_mc_state: &ShardStateUnsplit,
    vert_seqno: u32,
    gen_utime: u32,
    start_lt: u64,
    end_lt: u64,
    writes: &[AccountWrite],
) -> anyhow::Result<BuiltShardBlock> {
    let old_state = source
        .state
        .parse::<ShardStateUnsplit>()
        .context("Failed to parse previous basechain state")?;
    if old_state.shard_ident != source.shard {
        bail!(
            "basechain state belongs to shard {} instead of {}",
            old_state.shard_ident,
            source.shard
        );
    }
    if old_state.seqno != source.prev.seqno {
        bail!(
            "basechain state seqno {} does not match the previous block seqno {}",
            old_state.seqno,
            source.prev.seqno
        );
    }

    let mut accounts = old_state
        .accounts
        .load()
        .context("Failed to load basechain shard accounts")?;
    apply_writes(&mut accounts, writes, old_state.shard_ident)?;

    let new_state = ShardStateUnsplit {
        seqno: old_state
            .seqno
            .checked_add(1)
            .context("Sequence overflow")?,
        vert_seqno,
        gen_utime,
        gen_lt: end_lt,
        // The masterchain block that publishes this one is not visible from
        // inside it, so the shard keeps referencing the previous one.
        min_ref_mc_seqno: old_mc_state.seqno,
        accounts: Lazy::new(&accounts).context("Failed to wrap basechain shard accounts")?,
        total_balance: accounts.root_extra().balance.clone(),
        master_ref: Some(mc_prev_ref.clone()),
        ..old_state.clone()
    };
    let new_state_cell = CellBuilder::build_from(&new_state)
        .context("Failed to serialize post-fork basechain state")?;

    let prev_ref = BlockRef {
        end_lt: old_state.gen_lt,
        seqno: source.prev.seqno,
        root_hash: source.prev.root_hash,
        file_hash: source.prev.file_hash,
    };

    let mut info = fork_block_info(
        source.shard,
        new_state.seqno,
        vert_seqno,
        gen_utime,
        start_lt,
        end_lt,
        old_mc_state.seqno,
    );
    info.key_block = false;
    info.master_ref =
        Some(Lazy::new(mc_prev_ref).context("Failed to wrap basechain masterchain reference")?);
    info.set_prev_ref(&PrevBlockRef::Single(prev_ref));
    // `Collator::create_block_info` stores the masterchain reference here for
    // shard hardforks, not the previous shard block.
    info.prev_vert_ref =
        Some(Lazy::new(mc_prev_ref).context("Failed to wrap previous vertical reference")?);

    let block = finish_block(
        old_state.global_id,
        &source.state,
        &new_state_cell,
        info,
        value_flow(&old_state, &new_state),
        block_extra(
            rand_seed(new_state.seqno, gen_utime, &source.prev.root_hash),
            writes,
            None,
        )?,
        source.shard,
    )?;

    Ok(BuiltShardBlock {
        shard: source.shard,
        start_lt,
        end_lt,
        gen_utime,
        min_ref_mc_seqno: new_state.min_ref_mc_seqno,
        block,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_masterchain_block(
    old_state_cell: &Cell,
    old_state: &ShardStateUnsplit,
    old_extra: &McStateExtra,
    prev_ref: &BlockRef,
    vert_seqno: u32,
    gen_utime: u32,
    start_lt: u64,
    end_lt: u64,
    writes: &[AccountWrite],
    basechain: Option<&BuiltShardBlock>,
) -> anyhow::Result<HardforkBlock> {
    let seqno = old_state
        .seqno
        .checked_add(1)
        .context("Masterchain seqno overflow")?;

    let mut accounts = old_state
        .accounts
        .load()
        .context("Failed to load masterchain shard accounts")?;
    apply_writes(&mut accounts, writes, ShardIdent::MASTERCHAIN)?;
    let libraries = updated_libraries(old_state, writes)?;

    let mut extra = old_extra.clone();
    // A key block always starts a new masterchain catchain session.
    extra.validator_info.catchain_seqno = extra
        .validator_info
        .catchain_seqno
        .checked_add(1)
        .context("Catchain seqno overflow")?;
    extra.validator_info.nx_cc_updated = true;
    extra
        .prev_blocks
        .set(
            prev_ref.seqno,
            KeyMaxLt {
                has_key_block: old_extra.after_key_block,
                max_end_lt: prev_ref.end_lt,
            },
            KeyBlockRef {
                is_key_block: old_extra.after_key_block,
                block_ref: prev_ref.clone(),
            },
        )
        .context("Failed to record previous masterchain block")?;
    let (last_key_block, prev_key_block_seqno) = prev_key_block(old_extra, prev_ref);
    extra.after_key_block = true;
    extra.last_key_block = last_key_block;
    if let Some(shard) = basechain {
        extra.shards = published_shards(&extra.shards, shard, seqno)?;
    }

    let new_state = ShardStateUnsplit {
        seqno,
        vert_seqno,
        gen_utime,
        gen_lt: end_lt,
        libraries,
        accounts: Lazy::new(&accounts).context("Failed to wrap masterchain shard accounts")?,
        total_balance: accounts.root_extra().balance.clone(),
        custom: Some(Lazy::new(&extra).context("Failed to wrap McStateExtra")?),
        ..old_state.clone()
    };
    let new_state_cell = CellBuilder::build_from(&new_state)
        .context("Failed to serialize post-fork masterchain state")?;

    let mut info = fork_block_info(
        ShardIdent::MASTERCHAIN,
        seqno,
        vert_seqno,
        gen_utime,
        start_lt,
        end_lt,
        old_state.min_ref_mc_seqno,
    );
    // `accept-block.cpp:152` rejects a fork block that is not a key block.
    info.key_block = true;
    info.prev_key_block_seqno = prev_key_block_seqno;
    info.set_prev_ref(&PrevBlockRef::Single(prev_ref.clone()));
    info.prev_vert_ref =
        Some(Lazy::new(prev_ref).context("Failed to wrap previous vertical reference")?);

    let extra_cell = block_extra(
        rand_seed(seqno, gen_utime, &prev_ref.root_hash),
        writes,
        Some(McBlockExtra {
            shards: extra.shards.clone(),
            fees: Default::default(),
            prev_block_signatures: Default::default(),
            recover_create_msg: None,
            mint_msg: None,
            // A key block must carry the config: `accept-block.cpp:211` unpacks
            // and validates it before accepting the fork.
            config: Some(extra.config.clone()),
        }),
    )?;

    finish_block(
        old_state.global_id,
        old_state_cell,
        &new_state_cell,
        info,
        value_flow(old_state, &new_state),
        extra_cell,
        ShardIdent::MASTERCHAIN,
    )
}

/// Republishes the shard configuration with the forked basechain block on top.
///
/// The shard client applies whatever block the masterchain state names as the
/// shard top, which is what makes a basechain hardfork reachable at all. Only the
/// fields that describe the new top block change; split/merge plans, validator
/// shard and catchain bookkeeping are carried over untouched.
fn published_shards(
    shards: &ShardHashes,
    basechain: &BuiltShardBlock,
    mc_seqno: u32,
) -> anyhow::Result<ShardHashes> {
    let mut updated = Vec::new();
    let mut found = false;
    for entry in shards.iter() {
        let (ident, mut descr) = entry.context("Failed to read shard description")?;
        if ident == basechain.shard {
            descr.seqno = basechain.block.seqno;
            descr.reg_mc_seqno = mc_seqno;
            descr.start_lt = basechain.start_lt;
            descr.end_lt = basechain.end_lt;
            descr.root_hash = basechain.block.root_hash;
            descr.file_hash = basechain.block.file_hash;
            descr.gen_utime = basechain.gen_utime;
            descr.min_ref_mc_seqno = basechain.min_ref_mc_seqno;
            descr.before_split = false;
            descr.before_merge = false;
            descr.want_split = false;
            descr.want_merge = false;
            descr.fees_collected = CurrencyCollection::ZERO;
            descr.funds_created = CurrencyCollection::ZERO;
            found = true;
        }
        updated.push((ident, descr));
    }
    if !found {
        bail!(
            "masterchain state has no shard {} to publish a hardfork block for",
            basechain.shard
        );
    }

    ShardHashes::from_shards(updated.iter().map(|(ident, descr)| (ident, descr)))
        .context("Failed to rebuild shard configuration")
}

/// Builds `BlockExtra` with the transactions the caller wants recorded.
fn block_extra(
    rand_seed: HashBytes,
    writes: &[AccountWrite],
    custom: Option<McBlockExtra>,
) -> anyhow::Result<BlockExtra> {
    let account_blocks = build_account_blocks_from(writes.iter().filter_map(|write| {
        write.transaction.as_ref().map(|tx| ExecutedTransaction {
            account: write.address,
            lt: tx.lt,
            fees: tx.total_fees.clone(),
            transaction: tx.cell.clone(),
            old_state_hash: tx.old_state_hash,
            new_state_hash: tx.new_state_hash,
        })
    }))
    .context("Failed to build hardfork account blocks")?;

    Ok(BlockExtra {
        account_blocks: Lazy::new(&account_blocks).context("Failed to wrap account blocks")?,
        rand_seed,
        custom: match custom {
            Some(custom) => {
                Some(Lazy::new(&custom).context("Failed to wrap masterchain block extra")?)
            }
            None => None,
        },
        ..BlockExtra::default()
    })
}

/// Applies one batch of account changes to a shard account dictionary.
fn apply_writes(
    accounts: &mut ShardAccounts,
    writes: &[AccountWrite],
    shard: ShardIdent,
) -> anyhow::Result<()> {
    let mut seen = FxHashSet::default();
    for write in writes {
        ensure!(
            seen.insert(write.address),
            "Duplicate account write: {}",
            write.address
        );
        ensure!(
            write.transaction.is_none(),
            "Recorded transactions are not supported: hardfork message descriptors and outgoing queues must be built together"
        );
        match &write.account {
            Some(account) => {
                let state = account
                    .load_account()?
                    .context("Use AccountWrite::remove for a nonexistent account")?;
                let tycho_types::models::IntAddr::Std(address) = &state.address else {
                    bail!("Variable addresses are not supported");
                };
                ensure!(
                    address.workchain as i32 == shard.workchain()
                        && address.address == write.address
                        && address.anycast.is_none(),
                    "Replacement account address does not match its dictionary key"
                );
                let split_depth =
                    if let tycho_types::models::AccountState::Active(init) = &state.state {
                        ensure!(
                            shard.is_masterchain() || init.special.is_none(),
                            "Tick/tock is only supported in masterchain"
                        );
                        for entry in init.libraries.iter() {
                            let (hash, library) = entry?;
                            ensure!(
                                *library.root.repr_hash() == hash,
                                "Library key does not match its code hash"
                            );
                        }
                        init.split_depth.map_or(0, |d| d.into_bit_len() as u8)
                    } else {
                        0
                    };
                let balance = state.balance;
                accounts
                    .set(
                        write.address,
                        DepthBalanceInfo {
                            split_depth,
                            balance,
                        },
                        account.as_ref(),
                    )
                    .with_context(|| format!("Failed to overwrite account {}", write.address))?;
            }
            None => {
                accounts
                    .remove(write.address)
                    .with_context(|| format!("Failed to remove account {}", write.address))?;
            }
        }
    }
    Ok(())
}

/// Keep the masterchain registry consistent when publishers are edited or removed.
fn updated_libraries(
    state: &ShardStateUnsplit,
    writes: &[AccountWrite],
) -> anyhow::Result<tycho_types::dict::Dict<HashBytes, tycho_types::models::LibDescr>> {
    use tycho_types::models::{AccountState, LibDescr};
    let accounts = state.accounts.load()?;
    let mut libraries = state.libraries.clone();
    for write in writes {
        if let Some((_, old)) = accounts.get(write.address)?
            && let Some(account) = old.load_account()?
            && let AccountState::Active(init) = account.state
        {
            for entry in init.libraries.iter() {
                let (hash, library) = entry?;
                if library.public {
                    let mut registered = libraries
                        .get(hash)?
                        .context("Published library is missing from the masterchain registry")?;
                    registered.publishers.remove(write.address)?;
                    if registered.publishers.is_empty() {
                        libraries.remove(hash)?;
                    } else {
                        libraries.set(hash, registered)?;
                    }
                }
            }
        }
        if let Some(record) = &write.account
            && let Some(account) = record.load_account()?
            && let AccountState::Active(init) = account.state
        {
            for entry in init.libraries.iter() {
                let (hash, library) = entry?;
                ensure!(
                    *library.root.repr_hash() == hash,
                    "Library key does not match its code hash"
                );
                if library.public {
                    let mut registered = libraries.get(hash)?.unwrap_or(LibDescr {
                        lib: library.root,
                        publishers: Default::default(),
                    });
                    registered.publishers.set(write.address, ())?;
                    libraries.set(hash, registered)?;
                }
            }
        }
    }
    Ok(libraries)
}

/// Serializes one hardfork block from its already prepared parts.
#[allow(clippy::too_many_arguments)]
fn finish_block(
    global_id: i32,
    old_state_cell: &Cell,
    new_state_cell: &Cell,
    info: BlockInfo,
    value_flow: ValueFlow,
    extra: BlockExtra,
    shard: ShardIdent,
) -> anyhow::Result<HardforkBlock> {
    let state_update = MerkleUpdate::create(
        old_state_cell.as_ref(),
        new_state_cell.as_ref(),
        CellsOf::new(old_state_cell),
    )
    .build()
    .context("Failed to build hardfork state update")?;

    let seqno = info.seqno;
    let block = Block {
        global_id,
        info: Lazy::new(&info).context("Failed to wrap block info")?,
        value_flow: Lazy::new(&value_flow).context("Failed to wrap value flow")?,
        state_update: LazyExotic::new(&state_update).context("Failed to wrap state update")?,
        out_msg_queue_updates: None,
        extra: Lazy::new(&extra).context("Failed to wrap block extra")?,
    };

    let block_cell =
        CellBuilder::build_from(&block).context("Failed to serialize hardfork block")?;
    let block_boc = Boc::encode(block_cell.clone());
    let block_id = BlockId {
        shard,
        seqno,
        root_hash: *block_cell.repr_hash(),
        file_hash: Boc::file_hash(&block_boc),
    };
    let proof_link = crate::proof::build_block_proof_link(&block_id, &block_cell)?;

    Ok(HardforkBlock {
        shard,
        root_hash: block_id.root_hash,
        file_hash: block_id.file_hash,
        seqno,
        state_root_hash: *new_state_cell.repr_hash(),
        state_boc: Boc::encode(new_state_cell.clone()),
        block_boc,
        proof_link,
    })
}

/// Builds the parts of `BlockInfo` shared by every hardfork block.
///
/// A fork block is not produced by a validator session, so the collator writes
/// zeros into the validator list hash and catchain seqno
/// (`Collator::create_block_info`), and `flags` stays empty because no
/// `gen_software` is reported.
fn fork_block_info(
    shard: ShardIdent,
    seqno: u32,
    vert_seqno: u32,
    gen_utime: u32,
    start_lt: u64,
    end_lt: u64,
    min_ref_mc_seqno: u32,
) -> BlockInfo {
    BlockInfo {
        version: 0,
        after_merge: false,
        before_split: false,
        after_split: false,
        want_split: false,
        want_merge: false,
        key_block: false,
        flags: 0,
        seqno,
        vert_seqno,
        shard,
        gen_utime,
        start_lt,
        end_lt,
        gen_validator_list_hash_short: 0,
        gen_catchain_seqno: 0,
        min_ref_mc_seqno,
        prev_key_block_seqno: 0,
        gen_software: Default::default(),
        master_ref: None,
        prev_ref: Cell::default(),
        // Presence of this reference is what encodes `vert_seqno_incr`.
        prev_vert_ref: None,
    }
}

/// Builds the value flow of a hardfork block.
///
/// Nothing validates the flow of a fork block itself, but the totals must match
/// the account dictionaries of the two states: the next ordinary block declares
/// `from_prev_blk` from the state we leave behind, and `validate-query` compares
/// it against the sum over that dictionary.
fn value_flow(old_state: &ShardStateUnsplit, new_state: &ShardStateUnsplit) -> ValueFlow {
    ValueFlow {
        from_prev_block: old_state.total_balance.clone(),
        to_next_block: new_state.total_balance.clone(),
        ..ValueFlow::default()
    }
}

/// Resolves the key block that precedes the block being created.
///
/// Mirrors `ConfigInfo::get_last_key_block`: a state produced by a key block is
/// itself the last key block, otherwise the recorded reference is used.
fn prev_key_block(old_extra: &McStateExtra, prev_ref: &BlockRef) -> (Option<BlockRef>, u32) {
    if old_extra.after_key_block {
        (Some(prev_ref.clone()), prev_ref.seqno)
    } else if let Some(last) = &old_extra.last_key_block {
        (Some(last.clone()), last.seqno)
    } else {
        (None, 0)
    }
}

/// Rounds a logical time up to the next TON logical time boundary.
fn align_lt(lt: u64) -> anyhow::Result<u64> {
    let remainder = lt % LT_ALIGN;
    if remainder == 0 && lt != 0 {
        return Ok(lt);
    }
    lt.checked_add(LT_ALIGN - remainder)
        .context("Logical time overflow while aligning hardfork block")
}

/// Produces a deterministic `BlockExtra.rand_seed` for a hardfork block.
///
/// Nothing verifies this field for fork blocks, but deriving it from the block
/// identity keeps hardfork generation reproducible for a given input.
fn rand_seed(seqno: u32, gen_utime: u32, prev_root_hash: &HashBytes) -> HashBytes {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&seqno.to_be_bytes());
    bytes[4..8].copy_from_slice(&gen_utime.to_be_bytes());
    bytes[8..].copy_from_slice(&prev_root_hash.0[..24]);
    HashBytes(bytes)
}

/// Merkle filter that treats every cell of the previous state as reusable.
///
/// `MerkleUpdate::create` prunes the subtrees this filter includes, so listing
/// the whole previous tree keeps only genuinely new cells in the update. Without
/// it the update would embed a full copy of the state, which for a basechain
/// shard is unbounded.
struct CellsOf(FxHashSet<HashBytes>);

impl CellsOf {
    fn new(root: &Cell) -> Self {
        let mut hashes = FxHashSet::default();
        let mut stack = vec![root.clone()];
        while let Some(cell) = stack.pop() {
            if !hashes.insert(*cell.repr_hash()) {
                continue;
            }
            stack.extend(cell.references().cloned());
        }
        Self(hashes)
    }
}

impl MerkleFilter for CellsOf {
    fn check(&self, cell: &HashBytes) -> FilterAction {
        if self.0.contains(cell) {
            FilterAction::Include
        } else {
            FilterAction::Skip
        }
    }
}

#[cfg(test)]
mod tests {
    use tycho_types::dict::{AugDict, Dict};
    use tycho_types::merkle::MerkleProof;
    use tycho_types::models::account::{
        Account, AccountState, OptionalAccount, StorageExtra, StorageInfo, StorageUsed,
    };
    use tycho_types::models::block::BlockProof;
    use tycho_types::models::config::BlockchainConfig;
    use tycho_types::models::message::{IntAddr, StdAddr};
    use tycho_types::models::shard::ValidatorInfo;
    use tycho_types::num::Tokens;

    use super::*;

    const PREV_SEQNO: u32 = 41;
    const PREV_GEN_LT: u64 = 42_000_004;

    fn masterchain_state() -> Cell {
        let extra = McStateExtra {
            shards: ShardHashes::default(),
            config: BlockchainConfig::new_empty(HashBytes([0x33; 32])),
            validator_info: ValidatorInfo {
                validator_list_hash_short: 7,
                catchain_seqno: 3,
                nx_cc_updated: false,
            },
            prev_blocks: AugDict::new(),
            after_key_block: false,
            last_key_block: None,
            block_create_stats: None,
            global_balance: CurrencyCollection::new(1_000),
        };
        let state = ShardStateUnsplit {
            global_id: -3,
            shard_ident: ShardIdent::MASTERCHAIN,
            seqno: PREV_SEQNO,
            vert_seqno: 0,
            gen_utime: 100,
            gen_lt: PREV_GEN_LT,
            min_ref_mc_seqno: PREV_SEQNO,
            out_msg_queue_info: Cell::default(),
            before_split: false,
            accounts: Lazy::new(&ShardAccounts::new()).unwrap(),
            overload_history: 0,
            underload_history: 0,
            total_balance: CurrencyCollection::ZERO,
            total_validator_fees: CurrencyCollection::ZERO,
            libraries: Dict::new(),
            master_ref: None,
            custom: Some(Lazy::new(&extra).unwrap()),
        };
        CellBuilder::build_from(&state).unwrap()
    }

    fn account(address: HashBytes, balance: u128) -> ShardAccount {
        ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                address: IntAddr::Std(StdAddr::new(-1, address)),
                storage_stat: StorageInfo {
                    used: StorageUsed::ZERO,
                    storage_extra: StorageExtra::None,
                    last_paid: 0,
                    due_payment: None,
                },
                last_trans_lt: 0,
                balance: CurrencyCollection::new(balance),
                state: AccountState::Uninit,
            })))
            .unwrap(),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        }
    }

    fn sources() -> HardforkSources {
        HardforkSources {
            masterchain_state: masterchain_state(),
            masterchain_prev: HardforkPrevBlock {
                seqno: PREV_SEQNO,
                root_hash: HashBytes([0xaa; 32]),
                file_hash: HashBytes([0xbb; 32]),
            },
            basechain: None,
        }
    }

    #[test]
    fn masterchain_hardfork_is_a_key_block_with_vertical_increment() {
        let sources = sources();
        let plan = build_hardfork(
            &sources,
            200,
            &AdminBatch {
                masterchain: vec![AccountWrite::set(
                    HashBytes([0x11; 32]),
                    account(HashBytes([0x11; 32]), 777),
                )],
                basechain: Vec::new(),
            },
        )
        .unwrap();

        let block = Boc::decode(&plan.masterchain.block_boc)
            .unwrap()
            .parse::<Block>()
            .unwrap();
        let info = block.info.load().unwrap();

        assert_eq!(info.seqno, PREV_SEQNO + 1);
        assert_eq!(info.vert_seqno, 1);
        assert_eq!(plan.vert_seqno, 1);
        // A fork block is refused unless it is a key block that declares
        // `vert_seqno_incr`, which is encoded by the vertical reference.
        assert!(info.key_block);
        assert!(info.prev_vert_ref.is_some());
        assert!(info.master_ref.is_none());
        assert!(block.extra.load().unwrap().custom.is_some());
        assert!(plan.basechain.is_none());
    }

    #[test]
    fn state_update_of_a_hardfork_applies_to_the_previous_state() {
        let sources = sources();
        let address = HashBytes([0x11; 32]);
        let plan = build_hardfork(
            &sources,
            200,
            &AdminBatch {
                masterchain: vec![AccountWrite::set(address, account(address, 777))],
                basechain: Vec::new(),
            },
        )
        .unwrap();

        let block = Boc::decode(&plan.masterchain.block_boc)
            .unwrap()
            .parse::<Block>()
            .unwrap();
        let applied = block
            .state_update
            .load()
            .unwrap()
            .apply(&sources.masterchain_state)
            .unwrap();
        assert_eq!(*applied.repr_hash(), plan.masterchain.state_root_hash);

        let state = applied.parse::<ShardStateUnsplit>().unwrap();
        assert_eq!(state.seqno, PREV_SEQNO + 1);
        assert_eq!(state.vert_seqno, 1);
        assert_eq!(state.total_balance.tokens, Tokens::new(777));
        let (_, written) = state
            .accounts
            .load()
            .unwrap()
            .get(address)
            .unwrap()
            .unwrap();
        assert_eq!(
            written.load_account().unwrap().unwrap().balance.tokens,
            Tokens::new(777)
        );
    }

    #[test]
    fn proof_link_virtualizes_to_the_block_root() {
        let plan = build_hardfork(
            &sources(),
            200,
            &AdminBatch {
                masterchain: vec![AccountWrite::remove(HashBytes([0x11; 32]))],
                basechain: Vec::new(),
            },
        )
        .unwrap();

        let proof = Boc::decode(&plan.masterchain.proof_link)
            .unwrap()
            .parse::<BlockProof>()
            .unwrap();
        assert_eq!(proof.proof_for, plan.masterchain.block_id());
        assert!(proof.signatures.is_none());

        let merkle = proof.root.parse_exotic::<MerkleProof>().unwrap();
        let virtual_root = merkle.cell.virtualize();
        assert_eq!(*virtual_root.repr_hash(), plan.masterchain.root_hash);

        // The checker reads the header out of the proof, so those cells must be
        // present; `BlockExtra` is never touched and stays a pruned branch.
        let info = virtual_root
            .reference(0)
            .unwrap()
            .parse::<BlockInfo>()
            .unwrap();
        assert_eq!(info.seqno, PREV_SEQNO + 1);
        assert!(info.prev_ref.parse::<BlockRef>().is_ok());
        virtual_root
            .reference(1)
            .unwrap()
            .parse::<ValueFlow>()
            .unwrap();
        assert!(virtual_root.reference(3).unwrap().is_exotic());
        assert!(plan.masterchain.proof_link.len() < plan.masterchain.block_boc.len());
    }
}
