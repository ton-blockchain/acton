use crate::Network;
use crate::remote::{DtonClient, TonCenterClient, TonHubClient};
use crate::types::{
    AccountFromAPI, BaseTxInfo, Block, BlockInfo, ComputeInfo, RawTransaction, StateFromAPI,
    StorageStat, StorageUsed, TraceMoneyResult,
};
use base64::Engine;
use base64::engine::general_purpose;
use emulator::executor::ResultSuccess;
use std::collections::HashMap;
use std::str::FromStr;
use tycho_types::boc::Boc;
use tycho_types::cell::Lazy;
use tycho_types::dict::Dict;
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, MsgInfo, OptionalAccount, OutAction,
    OutActionsRevIter, ShardAccount, StdAddr, StorageExtra, StorageInfo, TxInfo,
};
use tycho_types::num::{Tokens, VarUint56};
use tycho_types::prelude::{Cell, HashBytes};

/// Returns base transaction information by its hash.
///
/// # Arguments
///
/// * `net`  — network to use
/// * `hash` — transaction hash to find
///
/// # Examples
///
/// ```ignore
/// let info = find_base_tx_by_hash(Network::Mainnet, "transaction_hash_hex").await?;
/// println!("Found tx with lt: {}", info.lt);
/// ```
pub async fn find_base_tx_by_hash(net: Network, hash: &str) -> anyhow::Result<BaseTxInfo> {
    let api_key = std::env::var("TONCENTER_API_KEY").ok();
    let client = TonCenterClient::new(net, api_key);

    let resp = client.get_transactions(hash, 1).await?;

    let Some(raw_tx) = resp.transactions.first() else {
        anyhow::bail!("Cannot find transaction in network {}", net);
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
    })
}

/// Returns full information for transaction by base information obtained from [`find_base_tx_by_hash`].
///
/// # Arguments
///
/// * `net`  — network to use
/// * `info` — base transaction information
pub(crate) async fn find_raw_tx_by_hash(
    net: Network,
    info: BaseTxInfo,
) -> anyhow::Result<Vec<RawTransaction>> {
    let client = TonHubClient::new(net);
    let address = info.address.display_base64_url(true).to_string();
    let hash_base64 = general_purpose::URL_SAFE.encode(info.hash);

    let resp = client
        .get_account_transactions(&address, info.lt, &hash_base64)
        .await?;

    let cells = boc_ext::decode_multi_root_base64(resp.boc)?;

    let mut txs = vec![];
    for (id, block) in resp.blocks.iter().enumerate() {
        let Some(cell): Option<&tycho_types::cell::Cell> = cells.get(id) else {
            continue;
        };
        let tx: tycho_types::models::Transaction = cell.parse()?;
        txs.push(RawTransaction {
            block: block.clone(),
            tx,
        });
    }

    Ok(txs)
}

/// Return the shard-block header that contains a given [`RawTransaction`].
///
/// # Arguments
///
/// * `net` — network to use
/// * `tx`  — raw transaction object
///
/// # Returns
///
/// Returns the matching shard-block or `None` if TonCenter cannot find it.
pub(crate) async fn find_shard_block_for_tx(
    net: Network,
    tx: &RawTransaction,
) -> anyhow::Result<Option<Block>> {
    let shard = &tx.block;

    // normalize potentially negative shard to positive one
    let shard_int = shard.shard.parse::<i64>()?;
    let shard_uint = shard_int as u64;
    let shard_hex = format!("0x{:x}", shard_uint);

    let api_key = std::env::var("TONCENTER_API_KEY").ok();
    let client = TonCenterClient::new(net, api_key);

    let res = client
        .get_blocks(shard.workchain, &shard_hex, shard.seqno)
        .await?;

    Ok(res.blocks.into_iter().next())
}

/// Return a master‑block (full representation, including `shards[]`)
/// by its `seqno` via TON API v4.
///
/// # Arguments
///
/// * `net`   — network to use
/// * `seqno` — master‑block sequence number
pub(crate) async fn find_full_block_for_seqno(
    net: Network,
    seqno: u32,
) -> anyhow::Result<BlockInfo> {
    let client = TonHubClient::new(net);
    client.get_block(seqno).await
}

/// Load the global configuration cell valid for the master‑block that
/// encloses the target transaction. Required by the TVM executor to
/// calculate gas, random‑seed and limits exactly as onchain.
///
/// # Arguments
///
/// * `net`   — network to use
/// * `block` — full master‑block object (with `shards[]` array)
///
/// # Returns
///
/// Returns the config cell as a hex-encoded string.
pub(crate) async fn get_block_config(net: Network, block: &BlockInfo) -> anyhow::Result<String> {
    let client = TonHubClient::new(net);

    let block_seqno = block
        .shards
        .first()
        .map(|s| s.seqno)
        .ok_or_else(|| anyhow::anyhow!("No shards found in master block"))?;

    client.get_config(block_seqno).await
}

/// Retrieve all transactions of a given account whose logical‑time
/// lies in the interval `(min_lt, base_tx.lt]`, inclusive of `base_tx`.
///
/// Used to reconstruct in‑block history before emulation.
///
/// # Arguments
///
/// * `net`     — network to use
/// * `base_tx` — the "upper bound" transaction
/// * `min_lt`  — lower logical‑time boundary
///
/// # Returns
///
/// Returns transactions ordered **newest → oldest**.
pub(crate) async fn find_all_transactions_between(
    net: Network,
    base_tx: &BaseTxInfo,
    min_lt: u64,
) -> anyhow::Result<Vec<tycho_types::models::Transaction>> {
    let api_key = std::env::var("TONCENTER_API_KEY").ok();
    let client = TonCenterClient::new(net, api_key);

    let address = base_tx.address.display_base64_url(false).to_string();
    let hash_base64 = general_purpose::STANDARD.encode(base_tx.hash);

    let to_lt = min_lt.saturating_sub(1);

    let raw_txs = client
        .get_transactions_toncenter(&address, base_tx.lt, &hash_base64, to_lt, 1000)
        .await?;

    let mut txs = Vec::new();
    for raw_tx in raw_txs {
        let Some(data) = raw_tx.get("data").and_then(|v| v.as_str()) else {
            continue;
        };
        let cell = Boc::decode_base64(data)?;
        let tx: tycho_types::models::Transaction = cell.parse()?;
        txs.push(tx);
    }

    Ok(txs)
}

/// Scan every shard‑summary inside a master‑block and return the
/// smallest `lt` for the specified account. This value marks the
/// earliest transaction of the account inside that master‑block.
///
/// # Arguments
///
/// * `tx`      — Target (latest) transaction object.
/// * `address` — Account address.
/// * `block`   — Master‑block that contains `tx`.
///
/// # Returns
///
/// Returns the minimum logical‑time as `u64`.
pub(crate) fn compute_min_lt(
    tx: &tycho_types::models::Transaction,
    address: &StdAddr,
    block: &BlockInfo,
) -> u64 {
    let mut min_lt = tx.lt;
    let addr_str = address.display_base64_url(false).to_string();
    for shard in &block.shards {
        for tx_in_block in &shard.transactions {
            if tx_in_block.account == addr_str
                && let Ok(lt) = tx_in_block.lt.parse::<u64>()
                && lt < min_lt
            {
                min_lt = lt;
            }
        }
    }
    min_lt
}

/// Return an account snapshot *before* the current master‑block.
/// The snapshot is converted to [`ShardAccount`] so it can be
/// directly fed into `run_transaction`.
///
/// # Arguments
///
/// * `net`     — network to use
/// * `address` — account address
/// * `block`   — master‑block N (the one that contains the tx)
///
/// # Returns
///
/// Returns [`ShardAccount`] representing state on master‑block N‑1.
pub(crate) async fn get_block_account(
    net: Network,
    address: &StdAddr,
    block: &BlockInfo,
) -> anyhow::Result<ShardAccount> {
    let client = TonHubClient::new(net);

    let block_seqno = block
        .shards
        .first()
        .map(|s| s.seqno)
        .ok_or_else(|| anyhow::anyhow!("No shards found in master block"))?;

    let address_str = address.display_base64_url(false).to_string();
    let api_account = client.get_account(block_seqno - 1, &address_str).await?;

    create_shard_account_from_api(api_account, address)
}

pub(crate) fn create_shard_account_from_api(
    api_account: AccountFromAPI,
    address: &StdAddr,
) -> anyhow::Result<ShardAccount> {
    let last_trans_lt = api_account
        .last
        .as_ref()
        .map(|l| l.lt.parse::<u64>())
        .transpose()?
        .unwrap_or(0);
    let last_trans_hash = api_account
        .last
        .as_ref()
        .map(|l| HashBytes::from_str(&l.hash))
        .transpose()?
        .unwrap_or(HashBytes::ZERO);

    let state = match api_account.state {
        StateFromAPI::Uninit => AccountState::Uninit,
        StateFromAPI::Active { data, code } => {
            let data = data.map(Boc::decode_base64).transpose()?;
            let code = code.map(Boc::decode_base64).transpose()?;
            AccountState::Active(tycho_types::models::StateInit {
                split_depth: None,
                special: None,
                code,
                data,
                libraries: Default::default(),
            })
        }
        StateFromAPI::Frozen { state_hash } => {
            AccountState::Frozen(HashBytes::from_str(&state_hash)?)
        }
    };

    let coins = api_account.balance.coins.parse::<u128>()?;

    let storage_stat = api_account.storage_stat.unwrap_or(StorageStat {
        last_paid: 0,
        due_payment: None,
        used: StorageUsed {
            bits: 0,
            cells: 0,
            public_cells: None,
        },
    });

    let account = Account {
        address: IntAddr::Std(address.clone()),
        storage_stat: StorageInfo {
            used: tycho_types::models::StorageUsed {
                cells: VarUint56::new(storage_stat.used.cells),
                bits: VarUint56::new(storage_stat.used.bits),
            },
            last_paid: storage_stat.last_paid as u32,
            due_payment: storage_stat
                .due_payment
                .map(|d| d.parse::<u128>())
                .transpose()?
                .map(Tokens::new),
            storage_extra: StorageExtra::None,
        },
        last_trans_lt,
        balance: CurrencyCollection {
            tokens: Tokens::new(coins),
            other: Default::default(),
        },
        state,
    };

    let shard_account = ShardAccount {
        account: Lazy::new(&OptionalAccount(Some(account)))?,
        last_trans_lt,
        last_trans_hash,
    };

    Ok(shard_account)
}

/// Extract the final `c5` register (action list) from emulation results,
/// decode it into an array of `OutAction`s and
/// return both the list and the original `c5` cell.
///
/// ## Params
/// - res — successful emulation result.
///
/// ## Returns
/// A tuple: (list of actions, original c5 cell).
pub(crate) fn find_final_actions(res: &ResultSuccess) -> (Vec<OutAction>, Option<Cell>) {
    let Some(actions_b64) = &res.actions else {
        return (Vec::new(), None);
    };

    let Ok(actions_cell) = Boc::decode_base64(actions_b64) else {
        return (Vec::new(), None);
    };

    let Ok(slice) = actions_cell.as_slice() else {
        return (Vec::new(), None);
    };

    let mut actions: Vec<OutAction> = OutActionsRevIter::new(slice)
        .filter_map(|res| res.ok())
        .collect();

    actions.reverse();
    (actions, Some(actions_cell))
}

/// Sum the value (`tokens`) of every *internal* outgoing message
/// produced by a transaction. External messages are ignored since its
/// value is always 0.
///
/// # Arguments
///
/// * `tx`  — Parsed `Transaction`.
///
/// # Returns
///
/// Returns the total tokens sent out by the contract in this tx.
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

/// Extract the opcode from the incoming message of a transaction.
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

/// Convert the raw [`ResultSuccess`] plus the prior balance
/// into a structured set of money movements, compute‑phase stats and
/// convenience fields for higher‑level reporting.
///
/// # Arguments
///
/// * `res`            — Successful result from TVM executor.
/// * `balance_before` — Balance **before** the emulated tx.
///
/// # Returns
///
/// Returns a breakdown containing sender/dest, amounts, gas usage and the parsed `emulated_tx`.
#[allow(clippy::type_complexity)]
pub(crate) fn compute_final_data(
    res: &ResultSuccess,
    balance_before: Tokens,
) -> anyhow::Result<(
    Option<IntAddr>,
    IntAddr,
    Option<Tokens>,
    TraceMoneyResult,
    tycho_types::models::Transaction,
    ComputeInfo,
)> {
    let shard_account_cell = Boc::decode_base64(&res.shard_account)?;
    let shard_account: ShardAccount = shard_account_cell.parse()?;
    let end_balance = shard_account
        .load_account()?
        .map(|a| a.balance.tokens)
        .unwrap_or(Tokens::ZERO);

    let emulated_tx_cell = Boc::decode_base64(&res.transaction)?;
    let emulated_tx: tycho_types::models::Transaction = emulated_tx_cell.parse()?;

    let in_msg = emulated_tx
        .load_in_msg()?
        .ok_or_else(|| anyhow::anyhow!("No in_message was found in result tx"))?;

    let (src, dest, amount) = match &in_msg.info {
        MsgInfo::Int(info) => (
            Some(info.src.clone()),
            info.dst.clone(),
            Some(info.value.tokens),
        ),
        MsgInfo::ExtIn(info) => (None, info.dst.clone(), None),
        MsgInfo::ExtOut(_) => anyhow::bail!("External out message as in_msg"),
    };

    let sent_total = calculate_sent_total(&emulated_tx);
    let total_fees = emulated_tx.total_fees.tokens;

    let TxInfo::Ordinary(info) = emulated_tx.load_info()? else {
        anyhow::bail!("Only ordinary transactions are supported");
    };

    let compute_info = match info.compute_phase {
        tycho_types::models::ComputePhase::Skipped(_) => ComputeInfo::Skipped,
        tycho_types::models::ComputePhase::Executed(exec) => {
            let exit_code = if exec.exit_code == 0 {
                info.action_phase.map(|a| a.result_code).unwrap_or(0)
            } else {
                exec.exit_code
            };
            ComputeInfo::Success {
                success: exec.success,
                exit_code,
                vm_steps: exec.vm_steps,
                gas_used: u64::from(exec.gas_used),
                gas_fees: u128::from(exec.gas_fees) as u64,
            }
        }
    };

    let money = TraceMoneyResult {
        balance_before: u128::from(balance_before) as u64,
        sent_total: u128::from(sent_total) as u64,
        total_fees: u128::from(total_fees) as u64,
        balance_after: u128::from(end_balance) as u64,
    };

    Ok((src, dest, amount, money, emulated_tx, compute_info))
}

/// Load a library cell (T‑lib) from toncenter or dton.io GraphQL by its
/// 256‑bit hash.
///
/// # Arguments
///
/// * `net`  — Mainnet/testnet flag.
/// * `hash` — Hex string of the library hash.
///
/// # Returns
///
/// Returns the decoded [`Cell`] containing actual code.
///
/// # Errors
///
/// Returns an error if the library is missing on the server.
pub(crate) async fn get_library_by_hash(net: Network, hash: &str) -> anyhow::Result<Cell> {
    let api_key = std::env::var("TONCENTER_API_KEY").ok();
    let toncenter = TonCenterClient::new(net, api_key);

    Ok(match toncenter.get_libraries(hash).await {
        Ok(data) => Boc::decode_base64(data)?,
        Err(_) => {
            let dton_api_key = std::env::var("DTON_API_KEY").ok();
            let dton = DtonClient::new(dton_api_key);
            let data = dton.get_lib(net, hash).await?;
            Boc::decode_base64(data)?
        }
    })
}

async fn add_maybe_exotic_library(
    net: Network,
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
    let lib_hash_hex = format!("{:X}", lib_hash);
    let actual_code = get_library_by_hash(net, &lib_hash_hex).await?;
    Ok(Some((lib_hash, actual_code)))
}

/// Inspect the contract’s current code and (optionally) the init
/// code of the pending message, detect all **exotic library cells**
/// (tag 2) and build a dict mapping hash → real library code.
///
/// # Arguments
///
/// * `net`             — Mainnet/testnet flag.
/// * `account`         — Current [`ShardAccount`] snapshot.
/// * `tx`              — Transaction whose `in_message` may include `Init`.
/// * `additional_libs` — Additional libraries to use.
///
/// # Returns
///
/// Returns a tuple: (dictionary cell with libs, actual code cell if original code is exotic lib).
pub(crate) async fn collect_used_libraries(
    net: Network,
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
        if let Some((hash, code)) = add_maybe_exotic_library(net, state.code).await? {
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
        if let Some((hash, code)) = add_maybe_exotic_library(net, init.code).await? {
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

// pub fn build_libs(libs_root: Option<Cell>, owner: &IntAddr) -> Dict<HashBytes, LibDescr> {
//     let mut libs_dict = Dict::<HashBytes, LibDescr>::new();
//     let Some(libs_root) = libs_root else {
//         return libs_dict;
//     };
//
//     let Ok(dict) = Dict::<HashBytes, Cell>::from_root(libs_root) else {
//         return libs_dict;
//     };
//
//     let owner_hash = match owner {
//         IntAddr::Std(std) => std.address,
//         IntAddr::Var(var) => var.address.clone().into(),
//     };
//
//     for entry in dict.iter() {
//         let Ok((hash, lib)) = entry else { continue };
//         let mut publishers = Dict::new();
//         publishers.add(owner_hash, ()).ok();
//
//         libs_dict
//             .add(
//                 hash,
//                 LibDescr {
//                     lib: lib.clone(),
//                     publishers,
//                 },
//             )
//             .ok();
//     }
//     libs_dict
// }

mod boc_ext {
    use base64::Engine;
    use base64::engine::general_purpose;
    use tycho_types::boc::de;
    use tycho_types::boc::de::Options;
    use tycho_types::cell::{Cell, CellContext, CellFamily};

    macro_rules! ok {
        ($e:expr $(,)?) => {
            match $e {
                core::result::Result::Ok(val) => val,
                core::result::Result::Err(err) => return core::result::Result::Err(err),
            }
        };
    }

    pub fn decode_multi_root_base64<T: AsRef<[u8]>>(data: T) -> Result<Vec<Cell>, de::Error> {
        fn decode_base64_impl(data: &[u8]) -> Result<Vec<Cell>, de::Error> {
            match general_purpose::STANDARD.decode(data) {
                Ok(data) => decode_ext(data.as_slice(), Cell::empty_context()),
                Err(_) => Err(de::Error::UnknownBocTag),
            }
        }
        decode_base64_impl(data.as_ref())
    }

    pub fn decode_ext(data: &[u8], context: &dyn CellContext) -> Result<Vec<Cell>, de::Error> {
        let header = ok!(de::BocHeader::decode(
            data,
            &Options {
                max_roots: Some(usize::MAX),
                min_roots: Some(1),
            },
        ));

        let mut final_cells = vec![];
        let cells = ok!(header.finalize(context));
        for root in header.roots() {
            if let Some(root) = cells.get(*root) {
                final_cells.push(root);
            }
        }

        Ok(final_cells)
    }
}
