use crate::block::types::{BlockBuildContext, BuiltShardState, LOCALNET_GLOBAL_ID, LOCALNET_SHARD};
use crate::storage::{AccountMeta, AccountStatus, CellStore};
use crate::types::{Addr, Lt, Seqno};
use anyhow::Context;
use std::collections::{BTreeMap, HashMap};
use tycho_types::cell::{Cell, Lazy};
use tycho_types::dict::Dict;
use tycho_types::models::ShardAccount;
use tycho_types::models::currency::CurrencyCollection;
use tycho_types::models::shard::{DepthBalanceInfo, ShardAccounts, ShardStateUnsplit};
use tycho_types::prelude::HashBytes;

/// Builds the previous and next shard states used by the block Merkle update.
///
/// `mine_block` calls this after executing all transactions, so
/// `accounts_after` already reflects the post-block view. To build the old root
/// we replay transaction deltas backwards and reconstruct the account map as it
/// looked before the block. The returned cells are full `ShardStateUnsplit`
/// cells; the Merkle update currently stores full old/new roots rather than a
/// pruned diff, which is larger but much simpler and still valid for indexing.
pub(super) fn build_old_and_new_states(
    ctx: &BlockBuildContext<'_>,
) -> anyhow::Result<(BuiltShardState, BuiltShardState)> {
    let accounts_before = accounts_before_block(ctx.accounts_after, ctx.transactions);
    let old_state = build_state(
        ctx.cas,
        &accounts_before,
        ctx.seqno.saturating_sub(1),
        ctx.prev_block.map_or(0, |block| block.gen_utime),
        ctx.prev_block.map_or(0, |block| block.end_lt),
    )
    .context("Failed to build previous shard state")?;
    let new_state = build_state(
        ctx.cas,
        ctx.accounts_after,
        ctx.seqno,
        ctx.gen_utime,
        ctx.end_lt,
    )
    .context("Failed to build next shard state")?;

    Ok((old_state, new_state))
}

pub(crate) fn create_shard_state_cell(
    cas: &CellStore,
    accounts: &HashMap<Addr, AccountMeta>,
    seqno: Seqno,
    gen_utime: u32,
    gen_lt: Lt,
) -> anyhow::Result<Cell> {
    Ok(build_state(cas, accounts, seqno, gen_utime, gen_lt)?.cell)
}

/// Reconstructs the account metadata map that existed before the block.
///
/// Each executed transaction carries the old account metadata captured before
/// emulation. Walking transactions in reverse rolls the post-block map back to
/// the previous state, including account creations where `old_meta` is `None`.
fn accounts_before_block(
    accounts_after: &HashMap<Addr, AccountMeta>,
    transactions: &[crate::block::types::BlockTransaction],
) -> HashMap<Addr, AccountMeta> {
    let mut accounts = accounts_after.clone();
    for tx in transactions.iter().rev() {
        if let Some(old_meta) = &tx.old_meta {
            accounts.insert(tx.tx_meta.account, old_meta.clone());
        } else {
            accounts.remove(&tx.tx_meta.account);
        }
    }
    accounts
}

/// Serializes the full-shard `ShardStateUnsplit` stored in localnet blocks.
///
/// The state contains the account dictionary and total balance for accounts that
/// localnet has touched or imported into its CAS. Validator queues, overload
/// history, public library dictionaries, and masterchain extras are left empty
/// because they are not required for local indexing of transactions.
fn build_state(
    cas: &CellStore,
    accounts: &HashMap<Addr, AccountMeta>,
    seqno: Seqno,
    gen_utime: u32,
    gen_lt: Lt,
) -> anyhow::Result<BuiltShardState> {
    let (shard_accounts, total_balance) = build_shard_accounts(cas, accounts)?;
    let accounts = Lazy::new(&shard_accounts).context("Failed to wrap shard accounts")?;
    let accounts_hash = *accounts.inner().repr_hash();

    let state = ShardStateUnsplit {
        global_id: LOCALNET_GLOBAL_ID,
        shard_ident: LOCALNET_SHARD,
        seqno,
        vert_seqno: 0,
        gen_utime,
        gen_lt,
        min_ref_mc_seqno: 0,
        out_msg_queue_info: Cell::default(),
        before_split: false,
        accounts,
        overload_history: 0,
        underload_history: 0,
        total_balance: CurrencyCollection::new(total_balance),
        total_validator_fees: CurrencyCollection::ZERO,
        libraries: Dict::new(),
        master_ref: None,
        custom: None,
    };

    Ok(BuiltShardState {
        cell: tycho_types::cell::CellBuilder::build_from(&state)
            .context("Failed to serialize shard state")?,
        accounts_hash,
        total_balance,
    })
}

/// Builds the `ShardAccounts` augmented dictionary and total balance.
///
/// The dictionary key is the 256-bit account id. Values are parsed
/// `ShardAccount` cells from the content-addressed store, paired with the
/// `DepthBalanceInfo` augmentation expected by TON block state. Nonexistent
/// accounts and accounts outside the local base workchain are omitted.
fn build_shard_accounts(
    cas: &CellStore,
    accounts: &HashMap<Addr, AccountMeta>,
) -> anyhow::Result<(ShardAccounts, u128)> {
    let mut entries = BTreeMap::new();
    let mut total_balance = 0u128;

    for (addr, meta) in accounts {
        if addr.workchain != 0 || meta.status == AccountStatus::Nonexist {
            continue;
        }

        let Some(cell) = cas.get_cell(&meta.account_hash) else {
            continue;
        };
        let shard_account = cell
            .parse::<ShardAccount>()
            .with_context(|| format!("Failed to parse shard account {addr}"))?;
        if shard_account
            .account
            .load()
            .context("Failed to load shard account optional account")?
            .0
            .is_none()
        {
            continue;
        }

        let balance = CurrencyCollection::new(meta.balance);
        total_balance = total_balance
            .checked_add(meta.balance)
            .context("Shard state balance overflow")?;
        entries.insert(
            HashBytes(addr.addr),
            (
                DepthBalanceInfo {
                    split_depth: 0,
                    balance,
                },
                shard_account,
            ),
        );
    }

    Ok((
        ShardAccounts::try_from_btree(&entries)
            .context("Failed to build shard accounts dictionary")?,
        total_balance,
    ))
}
