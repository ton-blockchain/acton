use crate::block::types::BlockTransaction;
use ton_hardfork::account_blocks::{ExecutedTransaction, build_account_blocks_from};
use tycho_types::models::block::AccountBlocks;
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::prelude::HashBytes;

/// Builds `BlockExtra.account_blocks` from executed localnet transactions.
///
/// This dictionary is the main structure external indexers use to discover
/// transactions in a block. Transactions are grouped by account id, keyed by
/// logical time inside each `AccountBlock`, and stored as lazy references to the
/// exact transaction cells produced by the executor. The account block state
/// update spans from the first pre-block account-state hash to the final
/// post-block account-state hash for that account.
pub(super) fn build_account_blocks(
    transactions: &[BlockTransaction],
) -> anyhow::Result<AccountBlocks> {
    build_account_blocks_from(transactions.iter().map(|tx| ExecutedTransaction {
        account: tx.account_hash(),
        lt: tx.tx_meta.lt,
        fees: CurrencyCollection::new(tx.tx_meta.total_fees),
        transaction: tx.tx_cell.clone(),
        old_state_hash: HashBytes(tx.old_account_state_hash.0),
        new_state_hash: HashBytes(tx.new_account_state_hash.0),
    }))
}
