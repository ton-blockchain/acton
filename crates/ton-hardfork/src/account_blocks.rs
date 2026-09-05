use anyhow::Context;
use std::collections::BTreeMap;
use tycho_types::cell::{Cell, Lazy};
use tycho_types::models::block::{AccountBlock, AccountBlocks};
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::transaction::{HashUpdate, Transaction};
use tycho_types::prelude::HashBytes;

/// One transaction as it has to appear in `BlockExtra.account_blocks`.
///
/// The localnet executor and the hardfork builder produce transactions in very
/// different ways, but a block records them identically, so both funnel through
/// this description.
pub struct ExecutedTransaction {
    /// Account the transaction belongs to.
    pub account: HashBytes,
    /// Logical time of the transaction; the key inside its `AccountBlock`.
    pub lt: u64,
    /// Fees charged, used for the `AccountBlock` augmentation.
    pub fees: CurrencyCollection,
    /// Exact serialized `Transaction` cell.
    pub transaction: Cell,
    /// Account-state hash before the transaction.
    pub old_state_hash: HashBytes,
    /// Account-state hash after the transaction.
    pub new_state_hash: HashBytes,
}

/// Groups transactions by account and builds the block transaction dictionary.
pub fn build_account_blocks_from(
    transactions: impl Iterator<Item = ExecutedTransaction>,
) -> anyhow::Result<AccountBlocks> {
    let mut groups = BTreeMap::<HashBytes, AccountBlockGroup>::new();

    for tx in transactions {
        tx.transaction
            .parse::<Transaction>()
            .context("Failed to parse transaction cell for block account list")?;

        let tx_ref = Lazy::<Transaction>::from_raw(tx.transaction)
            .context("Failed to wrap transaction cell for block account list")?;

        let group = groups
            .entry(tx.account)
            .or_insert_with(|| AccountBlockGroup {
                old_state_hash: tx.old_state_hash,
                new_state_hash: tx.new_state_hash,
                total_fees: CurrencyCollection::ZERO,
                transactions: BTreeMap::new(),
            });

        group.new_state_hash = tx.new_state_hash;
        group
            .total_fees
            .try_add_assign(&tx.fees)
            .context("Account block fees overflow")?;
        group.transactions.insert(tx.lt, (tx.fees, tx_ref));
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
