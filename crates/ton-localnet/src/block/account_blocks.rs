use crate::block::types::BlockTransaction;
use anyhow::Context;
use std::collections::BTreeMap;
use tycho_types::cell::Lazy;
use tycho_types::models::block::{AccountBlock, AccountBlocks};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::transaction::{HashUpdate, Transaction};
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
    let mut groups = BTreeMap::<HashBytes, AccountBlockGroup>::new();

    for tx in transactions {
        let tx_cell = tx.tx_cell.clone();
        tx_cell
            .parse::<Transaction>()
            .context("Failed to parse transaction cell for block account list")?;

        let fees = CurrencyCollection::new(tx.tx_meta.total_fees);
        let tx_ref = Lazy::<Transaction>::from_raw(tx_cell)
            .context("Failed to wrap transaction cell for block account list")?;

        let group = groups
            .entry(tx.account_hash())
            .or_insert_with(|| AccountBlockGroup {
                old_state_hash: HashBytes(tx.old_account_state_hash.0),
                new_state_hash: HashBytes(tx.new_account_state_hash.0),
                total_fees: CurrencyCollection::ZERO,
                transactions: BTreeMap::new(),
            });

        group.new_state_hash = HashBytes(tx.new_account_state_hash.0);
        group
            .total_fees
            .try_add_assign(&fees)
            .context("Account block fees overflow")?;
        group.transactions.insert(tx.tx_meta.lt, (fees, tx_ref));
    }

    let mut account_blocks = BTreeMap::new();
    for (account, group) in groups {
        let transactions = tycho_types::dict::AugDict::try_from_btree(&group.transactions)
            .context("Failed to build account transactions dictionary")?;
        let state_update = Lazy::new(&HashUpdate {
            old: group.old_state_hash,
            new: group.new_state_hash,
        })
        .context("Failed to build account block state update")?;

        account_blocks.insert(
            account,
            (
                group.total_fees,
                AccountBlock {
                    account,
                    transactions,
                    state_update,
                },
            ),
        );
    }

    AccountBlocks::try_from_btree(&account_blocks)
        .context("Failed to build block account dictionary")
}

struct AccountBlockGroup {
    /// Account-state hash before the first transaction for this account in the block.
    old_state_hash: HashBytes,
    /// Account-state hash after the last transaction for this account in the block.
    new_state_hash: HashBytes,
    /// Sum of transaction fees for the account block augmentation.
    total_fees: CurrencyCollection,
    /// Transactions keyed by logical time as required by `AccountBlock`.
    transactions: BTreeMap<u64, (CurrencyCollection, Lazy<Transaction>)>,
}
