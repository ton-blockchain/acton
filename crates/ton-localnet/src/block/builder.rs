use crate::block::account_blocks::build_account_blocks;
use crate::block::merkle::{OldStateCells, collect_path_to_hash};
use crate::block::messages::{build_in_msg_descr, build_out_msg_descr};
use crate::block::state::build_old_and_new_states;
use crate::block::types::{
    BlockBuildContext, BlockBuildResult, BuiltShardState, LOCALNET_GLOBAL_ID, LOCALNET_SHARD,
};
use crate::types::{BocBytes, Hash256};
use anyhow::Context;
use tycho_types::boc::Boc;
use tycho_types::cell::{CellBuilder, Lazy, LazyExotic};
use tycho_types::merkle::MerkleUpdate;
use tycho_types::models::block::{Block, BlockExtra, BlockInfo, BlockRef, PrevBlockRef, ValueFlow};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::prelude::HashBytes;

/// Builds a serialized TON `Block` cell for a localnet block.
///
/// The localnet executor has already produced real transaction cells and new
/// `ShardAccount` cells by the time this function is called. This function
/// arranges those pieces into the same high-level TL-B structure that external
/// tooling expects from a shard block: `BlockInfo`, `ValueFlow`, a Merkle state
/// update, and `BlockExtra` with `AccountBlocks`. The block omits
/// validator-specific artifacts: signatures, split/merge metadata, and
/// out-queue proofs. The goal is an indexable development block, not a candidate
/// that could pass validator consensus.
pub(crate) fn create_block_boc(ctx: BlockBuildContext<'_>) -> anyhow::Result<BlockBuildResult> {
    let (old_state, new_state) = build_old_and_new_states(&ctx)?;
    let state_update = shard_state_update(&old_state, &new_state)?;

    let info = build_block_info(&ctx)?;
    let fees_collected = ctx
        .transactions
        .iter()
        .try_fold(0u128, |acc, tx| acc.checked_add(tx.tx_meta.total_fees))
        .context("Block fees overflow")?;
    let value_flow = ValueFlow {
        from_prev_block: CurrencyCollection::new(
            new_state
                .total_balance
                .checked_add(fees_collected)
                .context("Block value flow overflow")?,
        ),
        to_next_block: CurrencyCollection::new(new_state.total_balance),
        fees_collected: CurrencyCollection::new(fees_collected),
        ..ValueFlow::default()
    };
    let extra = BlockExtra {
        in_msg_description: Lazy::new(&build_in_msg_descr(ctx.transactions)?)
            .context("Failed to wrap inbound message descriptor")?,
        out_msg_description: Lazy::new(&build_out_msg_descr()?)
            .context("Failed to wrap outbound message descriptor")?,
        account_blocks: Lazy::new(&build_account_blocks(ctx.transactions)?)
            .context("Failed to wrap account block dictionary")?,
        rand_seed: rand_seed(&ctx),
        created_by: HashBytes::ZERO,
        custom: None,
    };

    let block = Block {
        global_id: LOCALNET_GLOBAL_ID,
        info: Lazy::new(&info).context("Failed to wrap block info")?,
        value_flow: Lazy::new(&value_flow).context("Failed to wrap value flow")?,
        state_update: LazyExotic::new(&state_update).context("Failed to wrap state update")?,
        out_msg_queue_updates: None,
        extra: Lazy::new(&extra).context("Failed to wrap block extra")?,
    };

    let cell = CellBuilder::build_from(&block).context("Failed to serialize block")?;
    let block_hash = Hash256::from(cell.repr_hash());
    Ok(BlockBuildResult {
        block_boc: Boc::encode(cell).into(),
        block_hash,
    })
}

fn shard_state_update(
    old_state: &BuiltShardState,
    new_state: &BuiltShardState,
) -> anyhow::Result<MerkleUpdate> {
    let old_cells = OldStateCells::new(
        if old_state.cell.repr_hash() == new_state.cell.repr_hash() {
            Vec::new()
        } else if old_state.accounts_hash == new_state.accounts_hash {
            old_state_accounts_path(old_state)?
        } else {
            Vec::new()
        },
    );

    MerkleUpdate::create(old_state.cell.as_ref(), new_state.cell.as_ref(), old_cells)
        .build()
        .context("Failed to build shard state update")
}

fn old_state_accounts_path(old_state: &BuiltShardState) -> anyhow::Result<Vec<HashBytes>> {
    let mut path = Vec::new();
    anyhow::ensure!(
        collect_path_to_hash(old_state.cell.as_ref(), &old_state.accounts_hash, &mut path),
        "Shard state does not reference shard accounts"
    );
    Ok(path)
}

/// Creates the `BlockInfo` header for a localnet block.
///
/// `BlockInfo` is where external clients get the shard id, seqno, logical time
/// range, generation time, and previous block reference. Localnet models a
/// single full basechain shard, so split/merge flags stay empty. A masterchain
/// reference is stored because TON shardchain headers use that field to identify
/// the visible masterchain block; without it proof-checking clients treat the
/// header as structurally inconsistent. The previous reference is populated with
/// root/file hashes from the last local block when one exists.
fn build_block_info(ctx: &BlockBuildContext<'_>) -> anyhow::Result<BlockInfo> {
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

    let master_ref = ctx.master_ref.map_or_else(
        || BlockRef {
            end_lt: 0,
            seqno: 0,
            root_hash: HashBytes::ZERO,
            file_hash: HashBytes::ZERO,
        },
        |block| BlockRef {
            end_lt: block.end_lt,
            seqno: block.seqno,
            root_hash: HashBytes(block.block_hash.0),
            file_hash: HashBytes(block.file_hash.0),
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
        shard: LOCALNET_SHARD,
        gen_utime: ctx.gen_utime,
        start_lt: ctx.start_lt,
        end_lt: ctx.end_lt,
        gen_validator_list_hash_short: 0,
        gen_catchain_seqno: 0,
        min_ref_mc_seqno: 0,
        prev_key_block_seqno: 0,
        gen_software: Default::default(),
        master_ref: Some(Lazy::new(&master_ref).context("Failed to wrap masterchain reference")?),
        prev_ref: Default::default(),
        prev_vert_ref: None,
    };
    info.set_prev_ref(&prev_ref);
    Ok(info)
}

/// Produces a deterministic non-consensus random seed for `BlockExtra`.
///
/// Real validators derive this from collator/validator data. Localnet only needs
/// a stable 256-bit field that makes the block structurally valid, so the seed is
/// derived from the block seqno, generation time, and first transaction hash.
fn rand_seed(ctx: &BlockBuildContext<'_>) -> HashBytes {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&ctx.seqno.to_be_bytes());
    bytes[4..8].copy_from_slice(&ctx.gen_utime.to_be_bytes());
    if let Some(first) = ctx.transactions.first() {
        bytes[8..].copy_from_slice(&first.tx_meta.tx_hash.0[..24]);
    }
    HashBytes(bytes)
}

/// Computes the `file_hash` part of `BlockIdExt` from the serialized block `BoC`.
///
/// TON distinguishes the root cell representation hash (`root_hash`) from the
/// SHA-256 hash of the serialized `BoC` (`file_hash`). Older localnet blocks used
/// the root hash for both fields; real block ids need this value to be computed
/// from bytes so liteserver-style tooling can identify the block correctly.
pub(crate) fn file_hash(boc: &BocBytes) -> Hash256 {
    Hash256::from(Boc::file_hash(boc))
}
