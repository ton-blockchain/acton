use crate::storage::{AccountMeta, BlockMeta, CellStore, MasterchainBlockMeta, TxMeta};
use crate::types::{Addr, BocBytes, Hash256, Lt, Seqno};
use std::collections::HashMap;
use tycho_types::cell::Cell;
use tycho_types::models::block::ShardIdent;
use tycho_types::prelude::HashBytes;

/// Development-network global id written into localnet block/state cells.
///
/// The value is intentionally stable and local to Acton. It is not meant to
/// identify mainnet/testnet consensus data; it only keeps the generated TL-B
/// structures internally consistent for local tooling.
pub(crate) const LOCALNET_GLOBAL_ID: i32 = -3;

/// The single shard currently collated by localnet.
///
/// Localnet does not model masterchain/shardchain split, so every generated
/// block is a full basechain shard block (`workchain = 0`, full shard prefix).
pub(crate) const LOCALNET_SHARD: ShardIdent = ShardIdent::BASECHAIN;

/// Number of recent masterchain refs kept in localnet state/proof context.
pub(crate) const MASTERCHAIN_PREV_BLOCKS_LIMIT: usize = 16;

/// Transaction data needed to include one executed localnet transaction in a block.
///
/// `TxMeta` is the localnet index metadata used by API handlers, while
/// `tx_cell` is the exact serialized TON `Transaction` returned by the executor.
/// The old/new account-state hashes are used to build the containing
/// `AccountBlock` state update without reparsing historical account snapshots.
#[derive(Clone)]
pub(crate) struct BlockTransaction {
    /// Localnet metadata for indexes, LT ranges, fees, message hashes, and API responses.
    pub tx_meta: TxMeta,
    /// Account metadata before this transaction; `None` means the account did not exist.
    pub old_meta: Option<AccountMeta>,
    /// Exact TON transaction cell produced by the executor.
    pub tx_cell: Cell,
    /// Hash of the account state cell before this transaction.
    pub old_account_state_hash: Hash256,
    /// Hash of the account state cell after this transaction.
    pub new_account_state_hash: Hash256,
}

/// Immutable inputs required to assemble a real localnet block.
///
/// `Node::mine_block` owns execution and mutation; the block builder only needs
/// a snapshot of the resulting state, the executed transactions, previous block
/// metadata, and CAS access for account cells. Keeping this as a typed context
/// makes the block assembly code independent from the rest of `Node`.
pub(crate) struct BlockBuildContext<'a> {
    /// Sequence number of the block being assembled.
    pub seqno: Seqno,
    /// Unix timestamp assigned to the block.
    pub gen_utime: u32,
    /// First logical time covered by this block.
    pub start_lt: Lt,
    /// Last logical time covered by this block.
    pub end_lt: Lt,
    /// Previous localnet block, if this is not the first block.
    pub prev_block: Option<&'a BlockMeta>,
    /// Previous masterchain block visible to this shard block.
    pub master_ref: Option<&'a MasterchainBlockMeta>,
    /// Post-block account metadata map after all transactions have executed.
    pub accounts_after: &'a HashMap<Addr, AccountMeta>,
    /// Transactions executed in this block in collation order.
    pub transactions: &'a [BlockTransaction],
    /// Content-addressed store used to resolve `ShardAccount` cells by hash.
    pub cas: &'a CellStore,
}

/// Serialized shard block plus values already known while it was assembled.
pub(crate) struct BlockBuildResult {
    /// Serialized basechain `Block` root cell.
    pub block_boc: BocBytes,
    /// Representation hash of the block root cell.
    pub block_hash: Hash256,
}

/// Immutable inputs required to assemble a real localnet masterchain block.
///
/// Localnet mines a single basechain shard and then creates a masterchain block
/// that anchors that shard through `McStateExtra.shards`. The masterchain block
/// intentionally has no account transactions; it exists so `LiteAPI` clients can
/// discover and prove the basechain shard through real block/state cells instead
/// of adapter-generated ids.
pub(crate) struct MasterchainBlockBuildContext<'a> {
    /// Sequence number of the masterchain block being assembled.
    pub seqno: Seqno,
    /// Unix timestamp assigned to the block.
    pub gen_utime: u32,
    /// First logical time covered by the corresponding basechain block.
    pub start_lt: Lt,
    /// Last logical time covered by the corresponding basechain block.
    pub end_lt: Lt,
    /// Previous masterchain block, if this is not the first block.
    pub prev_block: Option<&'a MasterchainBlockMeta>,
    /// Previous masterchain state cell loaded from the previous block.
    pub prev_state: Option<Cell>,
    /// Basechain block anchored by this masterchain block.
    pub shard_block: &'a BlockMeta,
    /// Blockchain config dictionary root stored in the masterchain state.
    pub config_cell: &'a Cell,
    /// Earlier masterchain blocks exposed through `old_mc_blocks`.
    pub prev_blocks: &'a [MasterchainBlockMeta],
}

/// Serialized masterchain block and the state hash it declares.
///
/// The block `BoC` goes into the content-addressed store under its representation
/// hash, while `state_root_hash` is returned by `getMasterchainInfo` so tonlib can
/// compare the trusted masterchain state root with proof virtualization.
pub(crate) struct MasterchainBlockBuildResult {
    /// Serialized masterchain `Block` root cell.
    pub block_boc: BocBytes,
    /// Representation hash of the masterchain block root cell.
    pub block_hash: Hash256,
    /// Hash of the post-block masterchain state.
    pub state_root_hash: Hash256,
    /// Post-block masterchain state cell.
    pub state_cell: Cell,
}

/// Serialized shard state plus aggregate data needed by block assembly.
///
/// The `cell` is used directly in the block Merkle update. `total_balance` is
/// kept beside it because `ValueFlow` needs a total token amount and computing it
/// again would require walking the same account dictionary twice.
#[derive(Clone)]
pub(crate) struct BuiltShardState {
    /// Serialized `ShardStateUnsplit` root cell.
    pub cell: Cell,
    /// Representation hash of the `ShardAccounts` dictionary root.
    pub accounts_hash: HashBytes,
    /// Sum of native token balances for accounts included in this state.
    pub total_balance: u128,
}

impl BlockTransaction {
    /// Returns the 256-bit account id used as the key in block dictionaries.
    ///
    /// Localnet currently collates a single full basechain shard, so the account
    /// id alone is enough for `AccountBlocks` and `ShardAccounts`; workchain is
    /// already fixed by the surrounding block/shard metadata.
    pub(crate) const fn account_hash(&self) -> HashBytes {
        HashBytes(self.tx_meta.account.addr)
    }
}
