use crate::methods::{collect_used_libraries, find_final_actions};
use crate::methods::{
    compute_final_data, compute_min_lt, find_all_transactions_between, find_full_block_for_seqno,
    find_raw_tx_by_hash, find_shard_block_for_tx, get_block_account, get_block_config, tx_opcode,
};
use crate::types::{BaseTxInfo, TraceEmulatedTx, TraceInMessage, TraceResult};
use crate::{ComputeInfo, find_base_tx_by_hash, methods};
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use ton_executor::message::{EmulationResult, Executor, RunTransactionArgs};
use ton_executor::{ExecutorVerbosity, MissingLibrariesContext, missing_library_callback};
pub use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use tycho_types::models::{AccountState, ShardAccount, Transaction};
use tycho_types::num::Tokens;

/// Fully reproduce (re‑trace) a TON transaction inside a local TON Sandbox
/// and return a structured report with VM logs, money flow, generated
/// actions and other data.
///
/// # Workflow (high level)
///
/// 1.  Locate the base transaction on either mainnet or testnet.
/// 2.  Load its shard‑block and the enclosing master‑block; extract
///     `rand_seed`, config‑cell and the account snapshot *prior* to the block.
/// 3.  Re‑create the exact pre‑tx state by sequentially emulating all earlier
///     account transactions that happened inside the same master‑block.
/// 4.  Emulate the target transaction itself with full VM verbosity.
/// 5.  Parse the resulting VM log (`c5`, action list, stack trace), compare the
///     calculated state‑hash with the on‑chain one and assemble a
///     [`TraceResult`] object for the caller.
///
/// # Arguments
///
/// * `net`             — Network to use.
/// * `link`            — Hex hash that uniquely identifies the transaction to retrace.
/// * `additional_libs` — Additional libraries to use.
///
/// # Returns
///
/// Returns a [`TraceResult`] containing:
/// 1. an integrity flag `state_update_hash_ok`
/// 2. decoded an incoming message (sender / contract / amount)
/// 3. balance delta, gas and fees
/// 4. full emulated transaction (`emulated_tx`) with
///    compute‑phase info, `c5`, action list and raw VM log
/// 5. version of the sandbox executor used for emulation
///
/// # Errors
///
/// Returns an error if any network lookup fails; if the corresponding shard‑ /
/// master‑block cannot be found; if deterministic replay
/// diverges (TVM returns non‑success); or if state‑hash
/// mismatch is detected after replay.
///
/// # Examples
///
/// ```ignore
/// let result = retrace(Network::Mainnet, "hash", Default::default()).await?;
/// if result.state_update_hash_ok {
///     println!("Retrace successful!");
/// }
/// ```
pub async fn retrace(
    net: Network,
    link: &str,
    mut additional_libs: HashMap<HashBytes, Cell>,
) -> anyhow::Result<TraceResult> {
    let base_tx = find_base_tx_by_hash(net.clone(), link).await?;

    for _ in 0..5 {
        let result = retrace_base_tx(net.clone(), base_tx.clone(), additional_libs.clone()).await?;

        if matches!(
            &result.emulated_tx.compute_info,
            ComputeInfo::Success { exit_code: 9, .. }
        ) && load_missing_libraries(
            net.clone(),
            &result.emulated_tx.missing_libraries,
            &mut additional_libs,
        )
        .await?
        {
            continue;
        }

        return Ok(result);
    }

    anyhow::bail!("retrace failed to recover exit code 9");
}

async fn load_missing_libraries(
    net: Network,
    missing_libraries: &[String],
    additional_libs: &mut HashMap<HashBytes, Cell>,
) -> anyhow::Result<bool> {
    let mut loaded_any = false;

    for hash_hex in missing_libraries {
        let hash = HashBytes::from_str(hash_hex)
            .with_context(|| format!("Invalid missing library hash: {hash_hex}"))?;
        if additional_libs.contains_key(&hash) {
            continue;
        }

        let hash_for_api = format!("{hash:X}");
        let code = methods::get_library_by_hash(net.clone(), &hash_for_api)
            .await
            .with_context(|| format!("Failed to load missing library {hash_hex}"))?;
        additional_libs.insert(hash, code);
        loaded_any = true;
    }

    Ok(loaded_any)
}

/// Fully reproduce (re‑trace) a TON transaction by transaction triple
/// inside a local TON Sandbox and return a structured report with VM logs,
/// money flow, generated actions and other data.
///
/// See [`crate::retrace`] for the full description of the workflow.
///
/// # Arguments
///
/// * `net`             — Network to use.
/// * `base_tx`         — Handle for locating the transaction.
/// * `additional_libs` — Additional libraries to use.
///
/// # Examples
///
/// ```ignore
/// let base_tx = find_base_tx_by_hash(Network::Mainnet, "hash").await?;
/// let result = retrace_base_tx(Network::Mainnet, base_tx, Default::default()).await?;
/// ```
///
/// # Errors
///
/// Returns an error if any stage of fetching or emulation fails.
pub async fn retrace_base_tx(
    net: Network,
    base_tx: BaseTxInfo,
    additional_libs: HashMap<HashBytes, Cell>,
) -> anyhow::Result<TraceResult> {
    let txs = find_raw_tx_by_hash(net.clone(), base_tx.clone()).await?;
    let Some(tx) = txs.first() else {
        anyhow::bail!("Cannot find transaction info")
    };

    let shard = &tx.block;
    let Some(block) = find_shard_block_for_tx(net.clone(), tx).await? else {
        anyhow::bail!("Cannot find shard block for transaction")
    };

    // check if we correctly select master-block
    if block.root_hash != shard.root_hash {
        anyhow::bail!(
            "root_hash mismatch in mc_seqno getter: {} != {}",
            shard.root_hash,
            block.root_hash
        )
    }

    // master‑block sequence number that references our shard‑block
    let mc_seqno = block.masterchain_block_ref.seqno;
    // pseudorandom seed from the master‑block header — TVM needs it for deterministic RNG
    let rand_seed_vec = general_purpose::STANDARD.decode(block.rand_seed)?;
    let mut rand_seed: [u8; 32] = [0; 32];
    for (i, el) in rand_seed.iter_mut().enumerate() {
        *el = rand_seed_vec.get(i).copied().unwrap_or(0);
    }

    // load the complete master‑block object (includes the list of shard‑blocks)
    let full_block = find_full_block_for_seqno(net.clone(), mc_seqno).await?;

    // determine the earliest logical‑time (lt) for this account in the same master‑block
    let min_lt = compute_min_lt(&tx.tx, &base_tx.address, &full_block);
    // find all transactions between the earliest one and the emulated transaction to correctly
    // recreate all state before execution of the emulated transaction
    let mut prev_txs_in_block =
        find_all_transactions_between(net.clone(), &base_tx, min_lt).await?;
    // order oldest → newest, and remove the base_tx itself (the one we want to retrace)
    prev_txs_in_block.reverse();
    let Some(our_tx) = prev_txs_in_block.pop() else {
        anyhow::bail!("Cannot find transaction to retrace")
    };

    // retrieve block config to pass it to emulator
    let block_config = get_block_config(net.clone(), &full_block).await?;
    // load an account snapshot *before* the master‑block N
    let mut shard_account = get_block_account(net.clone(), &base_tx.address, &full_block).await?;

    let (libs, loaded_code) =
        collect_used_libraries(net, &shard_account, &tx.tx, &additional_libs).await?;

    // retrieve code cell if an account in active mode
    let Some(account_before_tx) = shard_account.load_account()? else {
        anyhow::bail!("Cannot load account")
    };
    let state = account_before_tx.state;
    let code_cell = match state {
        AccountState::Active(state) => state.code,
        _ => our_tx
            .load_in_msg()?
            .and_then(|msg| msg.init)
            .and_then(|init| init.code),
    };

    // for the first transaction (executor doesn't know about last tx)
    shard_account.last_trans_lt = 0;
    shard_account.last_trans_hash = HashBytes::ZERO;

    // first we emulate all transactions before to get a state that is equal to actual
    // state in blockchain before transaction to emulate
    let balance = account_before_tx.balance.tokens;
    let (balance, shard_account) = emulate_previous_transactions(
        &prev_txs_in_block,
        &shard_account,
        &balance,
        libs.as_ref(),
        &block_config,
        rand_seed,
    )?;

    // finally emulate the target transaction
    let (tx_res, executor_logs) = emulate(
        &our_tx,
        &block_config,
        &shard_account,
        libs.as_ref(),
        rand_seed,
    )?;
    let res = match tx_res {
        EmulationResult::Success(res) => res,
        EmulationResult::Error(err) => {
            anyhow::bail!("Emulated transaction failed: {:?}", err.error);
        }
    };
    let mut missing_libraries = res.missing_libraries.iter().cloned().collect::<Vec<_>>();
    missing_libraries.sort_unstable();

    // extract out actions from the c5 control register
    let (final_actions, c5) = find_final_actions(&res);

    let (sender, contract, amount, money, emulated_tx, compute_info) =
        compute_final_data(&res, balance)?;

    // check if the emulated transaction hash is equal to one from the real blockchain
    let state_update_hash_ok =
        emulated_tx.state_update.load()?.new == our_tx.state_update.load()?.new;

    let opcode = tx_opcode(&our_tx);

    Ok(TraceResult {
        state_update_hash_ok,
        code_cell: loaded_code.or_else(|| code_cell.clone()),
        original_code_cell: code_cell,
        in_msg: TraceInMessage {
            sender,
            contract,
            amount: amount.map(|a| u128::from(a) as u64),
            opcode,
        },
        money,
        emulated_tx: TraceEmulatedTx {
            raw: our_tx,
            utime: u64::from(emulated_tx.now),
            lt: emulated_tx.lt,
            compute_info,
            executor_logs,
            actions: final_actions,
            c5,
            vm_logs: res.vm_log,
            missing_libraries,
        },
    })
}

/// Re-emulates all transactions that occurred in the same account within
/// the same master-block *before* the target transaction.
///
/// This is necessary because the sandbox starts with an account state from
/// the *previous* master-block. To get the exact state before our target tx,
/// we must apply all intermediate transactions in order.
fn emulate_previous_transactions(
    prev_txs_in_block: &Vec<Transaction>,
    shard_account: &ShardAccount,
    balance: &Tokens,
    libs: Option<&Cell>,
    block_config: &str,
    rand_seed: [u8; 32],
) -> anyhow::Result<(Tokens, ShardAccount)> {
    let mut balance = *balance;
    let mut shard_account = shard_account.clone();

    for prev_tx in prev_txs_in_block {
        let (tx_res, _) = emulate(prev_tx, block_config, &shard_account, libs, rand_seed)?;
        let res = match tx_res {
            EmulationResult::Success(res) => res,
            EmulationResult::Error(err) => {
                anyhow::bail!("Previous transaction failed: {:?}", err.error);
            }
        };
        // since we change state at each transaction we need to save new state as current one
        shard_account = Boc::decode_base64(res.shard_account.as_ref())?.parse()?;
        balance = shard_account
            .load_account()?
            .map_or(Tokens::ZERO, |a| a.balance.tokens);
    }
    Ok((balance, shard_account))
}

/// Helper function to run a single transaction through the TVM executor.
fn emulate(
    tx: &Transaction,
    block_config: &str,
    shard_account: &ShardAccount,
    libs: Option<&Cell>,
    rand_seed: [u8; 32],
) -> anyhow::Result<(EmulationResult, Arc<str>)> {
    let Some(in_msg) = &tx.in_msg else {
        anyhow::bail!("No in_message was found in transaction")
    };

    let emulator = Executor::new(
        ExecutorVerbosity::FullLocationStackVerbose,
        Some(block_config),
    )?;
    let mut missing_libraries_ctx = MissingLibrariesContext::default();
    emulator
        .register_missing_library_callback(&mut missing_libraries_ctx, missing_library_callback)
        .context("Cannot register missing library callback")?;

    let (mut tx_res, executor_logs) = emulator.run_transaction(
        &Boc::encode_base64(in_msg),
        &RunTransactionArgs {
            libs: libs.map(Boc::encode_base64),
            shard_account: Boc::encode_base64(to_cell(shard_account)),
            now: tx.now,
            lt: tx.lt,
            random_seed: Some(rand_seed),
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
            is_tick_tock: None,
            is_tock: None,
        },
    )?;
    let mut missing_libraries = Some(missing_libraries_ctx.into_set());
    match &mut tx_res {
        EmulationResult::Success(result) => {
            result.missing_libraries = missing_libraries.take().unwrap_or_default();
        }
        EmulationResult::Error(error) => {
            error.missing_libraries = missing_libraries.take().unwrap_or_default();
        }
    }

    Ok((tx_res, executor_logs))
}

fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("Failed to store data into cell builder");
    builder.build().expect("Failed to build cell from builder")
}
