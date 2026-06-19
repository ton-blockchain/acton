use crate::localnet::LocalnetBlockHeader;
use crate::types::BocBytes;
use anyhow::Context;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::merkle::MerkleProof;
use tycho_types::models::ShardAccount;
use tycho_types::models::block::{ShardDescription, ShardHashes, ShardIdent};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::prelude::HashBytes;

/// Account-state payload expected by `liteServer.accountState`.
///
/// `shard_proof` links the real masterchain block to the real localnet shard
/// block through masterchain state. `proof` links the shard block to its
/// post-block shard state that contains the requested account. `state` is the
/// exact `OptionalAccount` cell from localnet's stored `ShardAccount`.
pub(super) struct AccountStateCells {
    pub shard_proof: Vec<u8>,
    pub proof: Vec<u8>,
    pub state: Vec<u8>,
}

/// Builds the `liteServer.allShardsInfo.data` `BoC` for the localnet shard.
///
/// `LiteAPI` clients ask the masterchain block for shard descriptions and then
/// fetch block bodies for every returned shard. Localnet has a single full
/// basechain shard, so the dictionary contains one `ShardDescription` that
/// points back to the block id produced by localnet's block builder.
pub(super) fn all_shards_info_data(header: &LocalnetBlockHeader) -> anyhow::Result<Vec<u8>> {
    let shards = shard_hashes(header)?;
    let cell = CellBuilder::build_from(&shards).context("Failed to serialize shard hashes")?;
    Ok(Boc::encode(cell))
}

/// Builds account proof/state cells for `liteServer.accountState`.
///
/// The account proof uses the real post-block shard state declared by the block's
/// Merkle update, so tonlib can compare the state hash declared by the block
/// header with the state root it virtualizes from the proof. The stored block
/// BOC itself may keep `state_update` pruned to avoid retaining full account
/// dictionaries in every empty block.
pub(super) fn account_state_cells(
    shard_account_boc: &BocBytes,
    block_boc: &BocBytes,
    shard_state_cell: &Cell,
    masterchain_block_boc: &BocBytes,
    masterchain_state_cell: &Cell,
) -> anyhow::Result<AccountStateCells> {
    let shard_account_cell =
        Boc::decode(shard_account_boc).context("Failed to decode ShardAccount BOC")?;
    let shard_account = shard_account_cell
        .parse::<ShardAccount>()
        .context("Failed to parse ShardAccount")?;
    let optional_account = shard_account
        .account
        .load()
        .context("Failed to load OptionalAccount")?;

    let state = if optional_account.0.is_some() {
        Boc::encode(shard_account.account.inner().clone())
    } else {
        Vec::new()
    };

    let block_cell = Boc::decode(block_boc).context("Failed to decode block BOC")?;
    let proof = two_root_proof(block_cell, shard_state_cell.clone())?;
    let shard_proof =
        masterchain_shard_proof(masterchain_block_boc, masterchain_state_cell.clone())?;

    Ok(AccountStateCells {
        shard_proof,
        proof,
        state,
    })
}

/// Builds `liteServer.configInfo` proofs for tonlib config validation.
///
/// `state_proof` is a Merkle proof of the stored masterchain block;
/// `config_proof` is a Merkle proof of the exact masterchain state declared by
/// that block. Tonlib virtualizes both and checks that the state root hash from
/// the block's `state_update.new` equals the virtualized state proof hash before
/// it extracts `McStateExtra.config`.
pub(super) fn config_proofs(
    masterchain_block_boc: &BocBytes,
    masterchain_state_cell: Cell,
) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    let block = Boc::decode(masterchain_block_boc).context("Failed to decode masterchain block")?;
    Ok((
        merkle_proof_boc(block)?,
        merkle_proof_boc(masterchain_state_cell)?,
    ))
}

/// Builds the `liteServer.shardInfo.shard_descr` `BoC` for localnet's shard.
///
/// The descriptor mirrors the shard entry exposed through
/// `liteServer.allShardsInfo`: it points to the real block root/file hashes that
/// localnet already generated, while merge/split and validator accounting fields
/// stay empty because the localnet model has a single full shard.
pub(super) fn shard_description_data(header: &LocalnetBlockHeader) -> anyhow::Result<Vec<u8>> {
    let shard_description = localnet_shard_description(header);
    let cell = CellBuilder::build_from(&shard_description)
        .context("Failed to serialize shard description")?;
    Ok(Boc::encode(cell))
}

/// Encodes an empty ordinary cell for TL fields that still expect cell bytes.
///
/// Several tonutils-go response structs decode proof `bytes` fields as `BoC`
/// cells during TL parsing even when the specific response path does not perform
/// proof validation. Those fields need a parseable cell instead of an omitted
/// byte slice.
pub(super) fn empty_cell_boc() -> Vec<u8> {
    Boc::encode(Cell::default())
}

/// Wraps a cell into a single-root `MerkleProof` `BoC`.
///
/// `tonlibjson` expects proof-bearing header fields such as
/// `lookupBlockResult.header` to deserialize as a single exotic Merkle proof
/// cell and then virtualizes that proof back into the block header cell. This
/// helper builds exactly that shape for either a real shard block or a real
/// localnet masterchain block.
pub(super) fn merkle_proof_boc(cell: Cell) -> anyhow::Result<Vec<u8>> {
    merkle_proof_cell(cell).map(Boc::encode)
}

/// Builds a single-root `MerkleProof` for a full post-state cell.
///
/// Stored localnet masterchain blocks may contain a pruned `MerkleUpdate`.
/// `LiteAPI` proof responses still need the virtualized post-state root
/// separately from the block root, so callers pass a rebuilt full state cell.
pub(super) fn state_proof_from_cell(state_cell: Cell) -> anyhow::Result<Vec<u8>> {
    merkle_proof_boc(state_cell)
}

fn shard_hashes(header: &LocalnetBlockHeader) -> anyhow::Result<ShardHashes> {
    let shard_ident = ShardIdent::new_full(header.id.workchain);
    let shard_description = localnet_shard_description(header);
    ShardHashes::from_shards([(&shard_ident, &shard_description)])
        .context("Failed to build shard hashes")
}

const fn localnet_shard_description(header: &LocalnetBlockHeader) -> ShardDescription {
    ShardDescription {
        seqno: header.id.seqno,
        reg_mc_seqno: header.id.seqno,
        start_lt: header.start_lt,
        end_lt: header.end_lt,
        root_hash: HashBytes(header.id.root_hash.0),
        file_hash: HashBytes(header.id.file_hash.0),
        before_split: false,
        before_merge: false,
        want_split: false,
        want_merge: false,
        nx_cc_updated: false,
        next_catchain_seqno: 0,
        next_validator_shard: header.id.shard as u64,
        min_ref_mc_seqno: 0,
        gen_utime: header.gen_utime,
        split_merge_at: None,
        fees_collected: CurrencyCollection::ZERO,
        funds_created: CurrencyCollection::ZERO,
    }
}

fn masterchain_shard_proof(
    masterchain_block_boc: &BocBytes,
    masterchain_state_cell: Cell,
) -> anyhow::Result<Vec<u8>> {
    let block = Boc::decode(masterchain_block_boc).context("Failed to decode masterchain block")?;
    two_root_proof(block, masterchain_state_cell)
}

fn two_root_proof(block_cell: Cell, state_cell: Cell) -> anyhow::Result<Vec<u8>> {
    let block_proof = merkle_proof_cell(block_cell)?;
    let state_proof = merkle_proof_cell(state_cell)?;
    Ok(Boc::encode_pair((block_proof, state_proof)))
}

fn merkle_proof_cell(cell: Cell) -> anyhow::Result<Cell> {
    let hash = *cell.hash(0);
    let depth = cell.depth(0);
    let proof = MerkleProof { hash, depth, cell };
    CellBuilder::build_from(&proof).context("Failed to build Merkle proof cell")
}
