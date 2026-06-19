use crate::block::merkle::{OldStateCells, collect_path_to_hash};
use crate::block::types::{
    LOCALNET_GLOBAL_ID, MASTERCHAIN_PREV_BLOCKS_LIMIT, MasterchainBlockBuildContext,
    MasterchainBlockBuildResult,
};
use crate::types::Hash256;
use anyhow::Context;
use std::collections::BTreeMap;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, Lazy, LazyExotic};
use tycho_types::dict::{AugDict, Dict};
use tycho_types::merkle::MerkleUpdate;
use tycho_types::models::block::{
    Block, BlockExtra, BlockInfo, BlockRef, McBlockExtra, PrevBlockRef, ShardDescription,
    ShardHashes, ShardIdent, ValueFlow,
};
use tycho_types::models::config::{BlockchainConfig, BlockchainConfigParams};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::shard::{
    KeyBlockRef, KeyMaxLt, McStateExtra, ShardAccounts, ShardStateUnsplit, ValidatorInfo,
};
use tycho_types::prelude::HashBytes;

/// Builds a serialized localnet masterchain block for one mined basechain block.
///
/// The resulting block has an empty transaction set and carries the useful
/// masterchain payload in its state and block extra: blockchain config, previous
/// masterchain block references, and a shard descriptor pointing at the real
/// basechain block mined for the same sequence number. This gives `LiteAPI`
/// clients a real block and state root to fetch and validate against.
pub(crate) fn create_masterchain_block_boc(
    ctx: MasterchainBlockBuildContext<'_>,
) -> anyhow::Result<MasterchainBlockBuildResult> {
    let new_state = create_masterchain_state_cell(&ctx)?;
    let old_state = ctx.prev_state.clone().unwrap_or_else(|| new_state.clone());
    let state_update = masterchain_state_update(&old_state, &new_state, ctx.config_cell)?;
    let info = masterchain_block_info(&ctx)?;
    let extra = masterchain_block_extra(&ctx)?;
    let block = Block {
        global_id: LOCALNET_GLOBAL_ID,
        info: Lazy::new(&info).context("Failed to wrap masterchain block info")?,
        value_flow: Lazy::new(&ValueFlow::default())
            .context("Failed to wrap masterchain value flow")?,
        state_update: LazyExotic::new(&state_update)
            .context("Failed to wrap masterchain state update")?,
        out_msg_queue_updates: None,
        extra: Lazy::new(&extra).context("Failed to wrap masterchain block extra")?,
    };
    let cell = CellBuilder::build_from(&block).context("Failed to serialize masterchain block")?;

    let block_hash = Hash256::from(cell.repr_hash());
    Ok(MasterchainBlockBuildResult {
        block_boc: Boc::encode(cell).into(),
        block_hash,
        state_root_hash: Hash256::from(new_state.repr_hash()),
        state_cell: new_state,
    })
}

/// Builds block-level masterchain extra that links the block to localnet shards.
///
/// Tonlib validates `lookupBlockWithProof` shard links by virtualizing a
/// masterchain block proof and reading `McBlockExtra.shards` from the block
/// itself. The same shard dictionary is also stored in the masterchain state, but
/// the block body must carry it as `McBlockExtra` for that validation path.
fn masterchain_block_extra(ctx: &MasterchainBlockBuildContext<'_>) -> anyhow::Result<BlockExtra> {
    let custom = McBlockExtra {
        shards: shard_hashes(ctx)?,
        ..Default::default()
    };
    Ok(BlockExtra {
        custom: Some(Lazy::new(&custom).context("Failed to wrap masterchain block extra custom")?),
        ..BlockExtra::default()
    })
}

/// Creates the `BlockInfo` header for a localnet masterchain block.
///
/// Masterchain blocks do not have `master_ref`; they are the anchor chain. The
/// previous reference points at the stored previous masterchain block when one
/// exists, which makes block ids and header proofs follow the local mined chain.
fn masterchain_block_info(ctx: &MasterchainBlockBuildContext<'_>) -> anyhow::Result<BlockInfo> {
    let prev_ref = ctx.prev_block.map_or_else(
        || {
            PrevBlockRef::Single(BlockRef {
                end_lt: 0,
                seqno: 0,
                root_hash: HashBytes::ZERO,
                file_hash: HashBytes::ZERO,
            })
        },
        |prev| {
            PrevBlockRef::Single(BlockRef {
                end_lt: prev.end_lt,
                seqno: prev.seqno,
                root_hash: HashBytes(prev.block_hash.0),
                file_hash: HashBytes(prev.file_hash.0),
            })
        },
    );

    let mut info = BlockInfo {
        version: 0,
        after_merge: false,
        before_split: false,
        after_split: false,
        want_split: false,
        want_merge: false,
        key_block: false,
        flags: 0,
        seqno: ctx.seqno,
        vert_seqno: 0,
        shard: ShardIdent::MASTERCHAIN,
        gen_utime: ctx.gen_utime,
        start_lt: ctx.start_lt,
        end_lt: ctx.end_lt,
        gen_validator_list_hash_short: 0,
        gen_catchain_seqno: 0,
        min_ref_mc_seqno: ctx.prev_block.map_or(0, |block| block.seqno),
        prev_key_block_seqno: 0,
        gen_software: Default::default(),
        master_ref: None,
        prev_ref: Default::default(),
        prev_vert_ref: None,
    };
    info.set_prev_ref(&prev_ref);
    Ok(info)
}

/// Builds the masterchain state rooted by a localnet masterchain block.
///
/// The state is intentionally small: it has no masterchain accounts or
/// libraries, but its custom `McStateExtra` contains the blockchain config, old
/// masterchain block refs, and a `ShardHashes` dictionary describing the single
/// basechain shard block created by the same mining step.
pub(crate) fn create_masterchain_state_cell(
    ctx: &MasterchainBlockBuildContext<'_>,
) -> anyhow::Result<Cell> {
    let custom = McStateExtra {
        shards: shard_hashes(ctx)?,
        config: blockchain_config(ctx.config_cell),
        validator_info: ValidatorInfo {
            validator_list_hash_short: 0,
            catchain_seqno: 0,
            nx_cc_updated: false,
        },
        prev_blocks: old_mc_blocks_info(ctx.prev_blocks)?,
        after_key_block: false,
        last_key_block: Some(BlockRef {
            end_lt: 0,
            seqno: 0,
            root_hash: HashBytes::ZERO,
            file_hash: HashBytes::ZERO,
        }),
        block_create_stats: None,
        global_balance: CurrencyCollection::ZERO,
    };
    let custom = Lazy::new(&custom).context("Failed to wrap masterchain state extra")?;
    let state = ShardStateUnsplit {
        global_id: LOCALNET_GLOBAL_ID,
        shard_ident: ShardIdent::MASTERCHAIN,
        seqno: ctx.seqno,
        vert_seqno: 0,
        gen_utime: ctx.gen_utime,
        gen_lt: ctx.end_lt,
        min_ref_mc_seqno: ctx.prev_block.map_or(0, |block| block.seqno),
        out_msg_queue_info: Cell::default(),
        before_split: false,
        accounts: Lazy::new(&ShardAccounts::default())
            .context("Failed to wrap masterchain accounts")?,
        overload_history: 0,
        underload_history: 0,
        total_balance: CurrencyCollection::ZERO,
        total_validator_fees: CurrencyCollection::ZERO,
        libraries: Dict::new(),
        master_ref: None,
        custom: Some(custom),
    };
    CellBuilder::build_from(&state).context("Failed to serialize masterchain state")
}

fn masterchain_state_update(
    old_state: &Cell,
    new_state: &Cell,
    config_cell: &Cell,
) -> anyhow::Result<MerkleUpdate> {
    let old_cells = OldStateCells::new(if old_state.repr_hash() == new_state.repr_hash() {
        Vec::new()
    } else {
        old_state_config_path(old_state, config_cell.repr_hash())?
    });

    MerkleUpdate::create(old_state.as_ref(), new_state.as_ref(), old_cells)
        .build()
        .context("Failed to build masterchain state update")
}

fn old_state_config_path(
    old_state: &Cell,
    config_hash: &HashBytes,
) -> anyhow::Result<Vec<HashBytes>> {
    let mut path = Vec::new();
    anyhow::ensure!(
        collect_path_to_hash(old_state.as_ref(), config_hash, &mut path),
        "Masterchain state does not reference blockchain config"
    );
    Ok(path)
}

/// Builds the shard dictionary entry that links masterchain state to basechain.
///
/// Localnet has exactly one full basechain shard, so this dictionary contains one
/// descriptor keyed by `ShardIdent::BASECHAIN`. The descriptor root/file hashes
/// are the real hashes of the stored basechain block, which is what external
/// indexers later use to fetch the block body.
fn shard_hashes(ctx: &MasterchainBlockBuildContext<'_>) -> anyhow::Result<ShardHashes> {
    let shard_ident = ShardIdent::BASECHAIN;
    let id = ctx.shard_block.block_id();
    let shard_description = ShardDescription {
        seqno: id.seqno,
        reg_mc_seqno: ctx.seqno,
        start_lt: ctx.shard_block.start_lt,
        end_lt: ctx.shard_block.end_lt,
        root_hash: HashBytes(id.root_hash.0),
        file_hash: HashBytes(id.file_hash.0),
        before_split: false,
        before_merge: false,
        want_split: false,
        want_merge: false,
        nx_cc_updated: false,
        next_catchain_seqno: 0,
        next_validator_shard: id.shard as u64,
        min_ref_mc_seqno: ctx.prev_block.map_or(0, |block| block.seqno),
        gen_utime: ctx.shard_block.gen_utime,
        split_merge_at: None,
        fees_collected: CurrencyCollection::ZERO,
        funds_created: CurrencyCollection::ZERO,
    };
    ShardHashes::from_shards([(&shard_ident, &shard_description)])
        .context("Failed to build masterchain shard hashes")
}

/// Wraps the local blockchain config dictionary for masterchain state.
///
/// The config root is the same `BoC` that the executor uses for TVM execution.
/// Storing it in the real masterchain state lets `liteServer.getConfig*`
/// responses prove config parameters against the block returned by
/// `getMasterchainInfo`.
fn blockchain_config(config_root: &Cell) -> BlockchainConfig {
    BlockchainConfig {
        address: HashBytes::ZERO,
        params: BlockchainConfigParams::from_raw(config_root.clone()),
    }
}

/// Builds the `old_mc_blocks` dictionary for masterchain state history.
///
/// The zero-state reference is always present because tonlib expects a root
/// entry. Recent previous localnet masterchain blocks are then added as non-key
/// block references; localnet does not model key blocks.
fn old_mc_blocks_info(
    prev_blocks: &[crate::storage::MasterchainBlockMeta],
) -> anyhow::Result<AugDict<u32, KeyMaxLt, KeyBlockRef>> {
    let mut entries = BTreeMap::new();
    entries.insert(
        0,
        (
            KeyMaxLt {
                has_key_block: false,
                max_end_lt: 0,
            },
            KeyBlockRef {
                is_key_block: false,
                block_ref: BlockRef {
                    end_lt: 0,
                    seqno: 0,
                    root_hash: HashBytes::ZERO,
                    file_hash: HashBytes::ZERO,
                },
            },
        ),
    );

    for block in prev_blocks.iter().rev().take(MASTERCHAIN_PREV_BLOCKS_LIMIT) {
        entries.insert(
            block.seqno,
            (
                KeyMaxLt {
                    has_key_block: false,
                    max_end_lt: block.end_lt,
                },
                KeyBlockRef {
                    is_key_block: false,
                    block_ref: BlockRef {
                        end_lt: block.end_lt,
                        seqno: block.seqno,
                        root_hash: HashBytes(block.block_hash.0),
                        file_hash: HashBytes(block.file_hash.0),
                    },
                },
            ),
        );
    }

    AugDict::try_from_btree(&entries).context("Failed to build old masterchain blocks dictionary")
}
