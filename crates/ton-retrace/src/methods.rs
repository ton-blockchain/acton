use crate::Network;
use crate::remote::TonCenterClient;
use crate::types::{BaseTxInfo, ComputeInfo, TraceMoneyResult};
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose;
use std::collections::HashMap;
use std::str::FromStr;
use ton_api::toncenter::v3;
use ton_executor::message::RunTransactionResultSuccess;
use ton_networks::CustomNetworkUrls;
use tycho_types::boc::Boc;
use tycho_types::cell::Lazy;
use tycho_types::dict::Dict;
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, MsgInfo, OptionalAccount, OutAction,
    OutActionsRevIter, ShardAccount, StdAddr, StorageInfo, TxInfo,
};
use tycho_types::num::Tokens;
use tycho_types::prelude::{Cell, HashBytes};

/// Returns base transaction information by its hash.
///
/// # Arguments
///
/// * `net`  — network to use
/// * `hash` — transaction hash to find
/// * `custom_networks` — V2/V3 endpoints for localnet and custom networks
///
/// # Examples
///
/// ```ignore
/// let info = find_base_tx_by_hash(Network::Mainnet, "transaction_hash_hex", &Default::default()).await?;
/// println!("Found tx with lt: {}", info.lt);
/// ```
pub async fn find_base_tx_by_hash(
    net: Network,
    hash: &str,
    custom_networks: &HashMap<String, CustomNetworkUrls>,
) -> anyhow::Result<BaseTxInfo> {
    let client = TonCenterClient::new(net.clone(), custom_networks)?;

    let resp = client
        .get_transactions(&[("hash", hash.to_owned()), ("limit", "1".to_owned())])
        .await?;

    let Some(raw_tx) = resp.transactions.first() else {
        anyhow::bail!("Cannot find transaction in network {net}");
    };

    let lt = raw_tx.lt.parse::<u64>()?;

    let mut hash_bytes = [0u8; 32];
    let decoded = general_purpose::STANDARD.decode(&raw_tx.hash)?;
    if decoded.len() != 32 {
        anyhow::bail!("Invalid hash length: {}", decoded.len());
    }
    hash_bytes.copy_from_slice(&decoded);

    let address = StdAddr::from_str(&raw_tx.account)?;

    Ok(BaseTxInfo {
        lt,
        hash: hash_bytes,
        address,
        block: raw_tx.block_ref.clone(),
    })
}

/// Loads the indexed shard-block header for the target transaction.
/// The header supplies the random seed and the masterchain reference used to
/// retrieve the configuration and the account state before replay.
pub(crate) async fn find_shard_block_for_tx(
    client: &TonCenterClient,
    tx: &BaseTxInfo,
) -> anyhow::Result<v3::Block> {
    client
        .get_blocks(&tx.block)
        .await?
        .blocks
        .into_iter()
        .next()
        .context("Cannot find shard block for transaction")
}

/// Loads the global configuration cell for the specified master‑block.
///
/// This configuration is required by the TVM executor to calculate gas costs,
/// random seeds, and limits exactly as they were on-chain at that time.
///
/// # Arguments
///
/// * `client` — Client for the network being replayed.
/// * `seqno` — Master-block sequence number.
///
/// # Returns
///
/// Returns the config cell as a base64-encoded `BoC`.
pub(crate) async fn get_block_config(
    client: &TonCenterClient,
    seqno: u32,
) -> anyhow::Result<String> {
    let config = client.get_config_all(seqno).await?;
    Ok(Boc::encode_base64(config))
}

/// Retrieves all transactions of an account within a logical-time interval.
///
/// Fetches transactions in the range `(after_lt, base_tx.lt]`, inclusive of the
/// target transaction. This is essential for reconstructing the account state
/// by replaying every transaction after the loaded account snapshot.
///
/// # Arguments
///
/// * `client` — Client for the network being replayed.
/// * `base_tx` — The "upper bound" transaction handle.
/// * `after_lt` — Last transaction LT in the loaded account snapshot.
///
/// # Returns
///
/// Returns transactions ordered from **newest to oldest**.
pub(crate) async fn find_all_transactions_between(
    client: &TonCenterClient,
    base_tx: &BaseTxInfo,
    after_lt: u64,
) -> anyhow::Result<Vec<tycho_types::models::Transaction>> {
    let address = base_tx.address.display_base64_url(false).to_string();
    let hash_base64 = general_purpose::STANDARD.encode(base_tx.hash);

    let raw_txs = client
        .get_account_transactions(&address, base_tx.lt, &hash_base64, after_lt, 1000)
        .await?;

    let mut txs = Vec::with_capacity(raw_txs.len());
    let mut expected_lt = base_tx.lt;
    let mut expected_hash = HashBytes(base_tx.hash);
    for raw_tx in raw_txs {
        let cell = Boc::decode_base64(raw_tx.data).with_context(|| {
            format!(
                "Failed to decode transaction at LT {}",
                raw_tx.transaction_id.lt
            )
        })?;
        let tx: tycho_types::models::Transaction = cell.parse().with_context(|| {
            format!(
                "Failed to parse TON Center transaction at LT {}",
                raw_tx.transaction_id.lt
            )
        })?;
        if tx.lt != expected_lt || *cell.repr_hash() != expected_hash {
            anyhow::bail!(
                "TON Center history does not match expected transaction at LT {expected_lt}"
            );
        }
        if tx.account != base_tx.address.address || tx.lt <= after_lt {
            anyhow::bail!(
                "TON Center history contains a transaction outside the requested account or LT range"
            );
        }
        if tx.prev_trans_lt >= tx.lt {
            anyhow::bail!(
                "TON Center transaction at LT {} has an invalid predecessor LT",
                tx.lt
            );
        }

        expected_lt = tx.prev_trans_lt;
        expected_hash = tx.prev_trans_hash;
        txs.push(tx);
    }

    // An archive can return fewer records than requested. Follow the chain to
    // detect truncation independently of the server's page size.
    if expected_lt > after_lt {
        anyhow::bail!("Incomplete TON Center history: missing transaction at LT {expected_lt}");
    }

    Ok(txs)
}

/// Returns an account snapshot as it existed *before* the current master‑block.
///
/// Fetches the account state at the end of the preceding master-block (N-1)
/// and converts it into a [`ShardAccount`] suitable for the TVM executor.
/// For block 1, `TON Center` cannot serve the zerostate, so replay uses the state
/// after block 1 as an approximation. The resulting state hash can differ from
/// the on-chain transaction and is still checked by the caller.
///
/// # Arguments
///
/// * `client` — Client for the network being replayed.
/// * `address` — Account address.
/// * `mc_seqno` — The master-block (N) containing the target transaction.
///
/// # Returns
///
/// Returns [`ShardAccount`] at N-1, or at block 1 for the first-block approximation.
pub(crate) async fn get_block_account(
    client: &TonCenterClient,
    address: &StdAddr,
    mc_seqno: u32,
) -> anyhow::Result<ShardAccount> {
    // TonCenter rejects seqno 0. Match retracer-core's first-block replay while
    // keeping the state-hash comparison visible to callers.
    let state_block_seqno = mc_seqno
        .checked_sub(1)
        .ok_or_else(|| anyhow::anyhow!("Cannot fetch account state before block seqno 0"))?
        .max(1);
    let address_str = address.to_string();

    let shard_account_cell = client
        .get_shard_account_cell(state_block_seqno, &address_str)
        .await?;

    let shard_account: ShardAccount = shard_account_cell
        .parse()
        .context("Failed to parse getShardAccountCell response as ShardAccount")?;

    if shard_account.load_account()?.is_some() {
        return Ok(shard_account);
    }

    Ok(ShardAccount {
        account: Lazy::new(&OptionalAccount(Some(Account {
            address: IntAddr::Std(address.clone()),
            storage_stat: StorageInfo::default(),
            last_trans_lt: shard_account.last_trans_lt,
            balance: CurrencyCollection::default(),
            state: AccountState::Uninit,
        })))?,
        last_trans_lt: shard_account.last_trans_lt,
        last_trans_hash: shard_account.last_trans_hash,
    })
}

/// Extracts out-actions from the `c5` register of a successful emulation.
///
/// Decodes the action list cell and returns both the parsed [`OutAction`]s
/// and the original `c5` cell.
///
/// # Arguments
///
/// * `res` — Successful emulation result.
pub(crate) fn find_final_actions(
    res: &RunTransactionResultSuccess,
) -> (Vec<OutAction>, Option<Cell>) {
    let Some(actions_b64) = &res.actions else {
        return (Vec::new(), None);
    };

    let Ok(actions_cell) = Boc::decode_base64(actions_b64.as_ref()) else {
        return (Vec::new(), None);
    };

    let Ok(slice) = actions_cell.as_slice() else {
        return (Vec::new(), None);
    };

    let mut actions: Vec<OutAction> = OutActionsRevIter::new(slice)
        .filter_map(Result::ok)
        .collect();

    actions.reverse();
    (actions, Some(actions_cell))
}

/// Sums the value of all *internal* outgoing messages in a transaction.
///
/// External messages are excluded as they carry no value.
pub(crate) fn calculate_sent_total(tx: &tycho_types::models::Transaction) -> Tokens {
    let mut total = 0u128;
    for msg in tx.iter_out_msgs() {
        let Ok(msg) = msg else { continue };
        if let MsgInfo::Int(info) = &msg.info {
            total += u128::from(info.value.tokens);
        }
    }
    Tokens::new(total)
}

/// Extracts the operation opcode from the incoming message of a transaction.
///
/// Handles both regular internal messages and bounced messages (skipping the bounce tag).
pub(crate) fn tx_opcode(tx: &tycho_types::models::Transaction) -> Option<u32> {
    let in_msg = tx.load_in_msg().ok()??;
    let mut slice = in_msg.body;

    if let MsgInfo::Int(info) = in_msg.info
        && info.bounced
    {
        // skip 0xFFFF..
        let _ = slice.load_u32().ok()?;
    }

    let opcode = slice.load_u32().ok()?;
    Some(opcode)
}

/// Assembles final execution data from successful emulation results.
///
/// Extracts balance changes, fee breakdown, and compute phase statistics.
/// Tick-tock has no incoming message, so its contract address comes from the
/// requested transaction's account, even if execution destroyed the account.
///
/// # Returns
///
/// Returns a tuple containing:
/// (Source, Destination, Amount, `MoneyResult`, Transaction, `ComputeInfo`)
#[allow(clippy::type_complexity)]
pub(crate) fn compute_final_data(
    res: &RunTransactionResultSuccess,
    balance_before: Tokens,
    contract_address: &StdAddr,
) -> anyhow::Result<(
    Option<IntAddr>,
    IntAddr,
    Option<Tokens>,
    TraceMoneyResult,
    tycho_types::models::Transaction,
    ComputeInfo,
)> {
    let shard_account_cell = Boc::decode_base64(res.shard_account.as_ref())?;
    let shard_account: ShardAccount = shard_account_cell.parse()?;
    let end_balance = shard_account
        .load_account()?
        .map_or(Tokens::ZERO, |a| a.balance.tokens);

    let emulated_tx_cell = Boc::decode_base64(res.transaction.as_ref())?;
    let emulated_tx: tycho_types::models::Transaction = emulated_tx_cell.parse()?;

    let (src, dest, amount, compute_phase) = match emulated_tx.load_info()? {
        TxInfo::Ordinary(info) => {
            let in_msg = emulated_tx
                .load_in_msg()?
                .context("No in_message was found in result tx")?;

            let (src, dest, amount) = match in_msg.info {
                MsgInfo::Int(info) => (Some(info.src), info.dst, Some(info.value.tokens)),
                MsgInfo::ExtIn(info) => (None, info.dst, None),
                MsgInfo::ExtOut(_) => anyhow::bail!("External out message as in_msg"),
            };

            (src, dest, amount, info.compute_phase)
        }
        TxInfo::TickTock(info) => (
            None,
            IntAddr::Std(contract_address.clone()),
            None,
            info.compute_phase,
        ),
    };

    let sent_total = calculate_sent_total(&emulated_tx);
    let total_fees = emulated_tx.total_fees.tokens;

    let compute_info = match compute_phase {
        tycho_types::models::ComputePhase::Skipped(_) => ComputeInfo::Skipped,
        tycho_types::models::ComputePhase::Executed(exec) => ComputeInfo::Success {
            success: exec.success,
            exit_code: exec.exit_code,
            vm_steps: exec.vm_steps,
            gas_used: u64::from(exec.gas_used),
            gas_fees: u128::from(exec.gas_fees) as u64,
        },
    };

    let money = TraceMoneyResult {
        balance_before: u128::from(balance_before) as u64,
        sent_total: u128::from(sent_total) as u64,
        total_fees: u128::from(total_fees) as u64,
        balance_after: u128::from(end_balance) as u64,
    };

    Ok((src, dest, amount, money, emulated_tx, compute_info))
}

/// Loads a library cell (T‑lib) by its 256‑bit hash.
///
/// Fetches the library from `TON Center`.
pub(crate) async fn get_library_by_hash(
    client: &TonCenterClient,
    hash: &str,
) -> anyhow::Result<Cell> {
    let data = client.get_libraries(hash).await?;
    Boc::decode_base64(data).context("Failed to decode library BOC data")
}

async fn add_maybe_exotic_library(
    client: &TonCenterClient,
    code: Option<Cell>,
) -> anyhow::Result<Option<(HashBytes, Cell)>> {
    const EXOTIC_LIBRARY_TAG: u8 = 2;
    let Some(code) = code else { return Ok(None) };

    let slice = code.as_slice_allow_exotic();
    if slice.size_bits() != 256 + 8 {
        // not an exotic library cell
        return Ok(None);
    }

    let mut cs = code.as_slice_allow_exotic();
    let tag = cs.load_u8()?;
    if tag != EXOTIC_LIBRARY_TAG {
        // not a library cell
        return Ok(None);
    }

    let lib_hash = cs.load_u256()?;
    let lib_hash_hex = format!("{lib_hash:X}");
    let actual_code = get_library_by_hash(client, &lib_hash_hex).await?;
    Ok(Some((lib_hash, actual_code)))
}

/// Identifies and collects all exotic library cells used by a contract.
///
/// Scans both the current contract code and any `StateInit` in the incoming
/// message for exotic library references (tag 2). If found, it downloads the
/// real code via [`get_library_by_hash`] and builds a dictionary for the
/// TVM executor.
///
/// # Arguments
///
/// * `client`          — Client for the network being replayed.
/// * `account`         — Current account state.
/// * `tx`              — Incoming transaction.
/// * `additional_libs` — User-provided libraries to include.
///
/// # Returns
///
/// Returns a tuple: (Dictionary cell with resolved libs, Actual code cell if original code was exotic).
pub(crate) async fn collect_used_libraries(
    client: &TonCenterClient,
    account: &ShardAccount,
    tx: &tycho_types::models::Transaction,
    additional_libs: &HashMap<HashBytes, Cell>,
) -> anyhow::Result<(Option<Cell>, Option<Cell>)> {
    let mut libs = HashMap::new();

    // if current contract code is exotic cell, we want to return actual code to the user
    let mut loaded_cell_code: Option<Cell> = None;

    // 1. scan the *current* contract code for exotic‑library links
    if let Some(acc) = account.load_account()?
        && let AccountState::Active(state) = acc.state
    {
        // The contract is already deployed and “active” so its `code`
        // cell may itself be a 264‑bit exotic library reference (tag 2).
        // If that’s the case, download the real library code and
        // register it in the `libs` dictionary.
        if let Some((hash, code)) = add_maybe_exotic_library(client, state.code).await? {
            libs.insert(hash, code.clone());
            loaded_cell_code = Some(code);
        }
    }

    // 2. scan the *incoming StateInit* (if present)
    if let Some(in_msg) = tx.load_in_msg()?
        && let Some(init) = in_msg.init
    {
        // This transaction might *deploy* a brand‑new contract or
        // *upgrade* the existing one. Its `StateInit.code` could also
        // be an exotic library cell. We must preload such libraries as
        // well, otherwise the sandbox would fail to resolve a library
        // during emulation.
        if let Some((hash, code)) = add_maybe_exotic_library(client, init.code).await? {
            libs.insert(hash, code.clone());
            loaded_cell_code.get_or_insert(code);
        }
    }

    for (hash, lib) in additional_libs {
        libs.insert(*hash, lib.clone());
    }

    // no libs found, return None, for emulator this means no libraries
    if libs.is_empty() {
        return Ok((None, loaded_cell_code));
    }

    // emulator expects libraries as a Cell with immediate dictionary
    let mut dict = Dict::<HashBytes, Cell>::new();
    for (hash, cell) in libs {
        dict.add(hash, cell)?;
    }

    Ok((dict.into_root(), loaded_cell_code))
}
