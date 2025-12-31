use crate::methods::{collect_used_libraries, find_final_actions};
use crate::methods::{
    compute_final_data, compute_min_lt, find_all_transactions_between, find_full_block_for_seqno,
    find_raw_tx_by_hash, find_shard_block_for_tx, get_block_account, get_block_config, tx_opcode,
};
use crate::types::{BaseTxInfo, TraceEmulatedTx, TraceInMessage, TraceResult};
use crate::{ComputeInfo, find_base_tx_by_hash, methods};
use base64::Engine;
use base64::engine::general_purpose;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{EmulationResult, Executor, RunTransactionArgs};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use tycho_types::models::{AccountState, ShardAccount, Transaction};
use tycho_types::num::Tokens;
use vmlogs::parser::{CellLike, VmLine, VmStackValue, parse_lines};

/// Supported TON networks for transaction retracing.
#[derive(Debug, Clone, Copy)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Display for Network {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Network::Mainnet => write!(f, "mainnet"),
            Network::Testnet => write!(f, "testnet"),
        }
    }
}

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
    let base_tx = find_base_tx_by_hash(net, link).await?;

    for _ in 0..5 {
        let result = retrace_base_tx(net, base_tx.clone(), additional_libs.clone()).await?;

        if let ComputeInfo::Success { exit_code: 9, .. } = result.emulated_tx.compute_info {
            // This can be both a simple cell underflow and failed to load a library cell.
            // Parse vm_logs to find out.

            // Example logs:
            //
            // stack: [ ... C{B5EE9C72010101010023000842029468B29F43AC803FC9F621953FDD069A432E4CD1D9A56B9C299B587FE6898FAB} ]
            // code cell hash: 4F5F4CE417F91358B532A9670A09D20AC7E01850E9B704A4DF1CC5373EE6EDE4 offset: 887
            // execute CTOS
            // handling exception code 9: failed to load library cell
            // default exception handler, terminating vm with exit code 9

            let lines = parse_lines(&result.emulated_tx.vm_logs);
            let lines: Vec<_> = lines.into_iter().filter_map(|l| l.ok()).collect();

            if lines.len() < 6 {
                return Ok(result);
            }

            let n = lines.len();
            let exception_handler_line = &lines[n - 1];
            let exception_line = &lines[n - 2];
            let ctos_line = &lines[n - 3];
            let stack_line = &lines[n - 5];

            if let (
                VmLine::VmExceptionHandler { .. },
                VmLine::VmException { message, .. },
                VmLine::VmExecute { instr },
                VmLine::VmStack { stack },
            ) = (
                exception_handler_line,
                exception_line,
                ctos_line,
                stack_line,
            ) && message == &"failed to load library cell"
                && instr == &"CTOS"
                && let Some(VmStackValue::Cell(CellLike::Cell(hex_boc))) = stack.parsed().last()
                && let Some((hash, code)) = try_load_as_library(net, hex_boc).await?
            {
                // So we find out that the transaction failed to load a library cell.
                // Stack before CTOS will contain the library cell as the top element.

                // Now we have the library content and hash, so we try again with this library.
                additional_libs.insert(hash, code);
                continue;
            }
        }

        return Ok(result);
    }

    anyhow::bail!("retrace failed to recover exit code 9");
}

async fn try_load_as_library(
    net: Network,
    hex_boc: &str,
) -> anyhow::Result<Option<(HashBytes, Cell)>> {
    let cell = Boc::decode_hex(hex_boc)?;

    const EXOTIC_LIBRARY_TAG: u8 = 2;
    let slice = cell.as_slice_allow_exotic();
    if slice.size_bits() != 256 + 8 {
        return Ok(None);
    }

    let mut cs = cell.as_slice_allow_exotic();
    let tag = cs.load_u8()?;
    if tag != EXOTIC_LIBRARY_TAG {
        return Ok(None);
    }

    let lib_hash = cs.load_u256()?;
    let lib_hash_hex = format!("{:X}", lib_hash);
    let actual_code = methods::get_library_by_hash(net, &lib_hash_hex).await?;
    Ok(Some((lib_hash, actual_code)))
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
    let txs = find_raw_tx_by_hash(net, base_tx.clone()).await?;
    let Some(tx) = txs.first() else {
        anyhow::bail!("Cannot find transaction info")
    };

    let shard = &tx.block;
    let Some(block) = find_shard_block_for_tx(net, tx).await? else {
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
        *el = rand_seed_vec.get(i).cloned().unwrap_or(0)
    }

    // load the complete master‑block object (includes the list of shard‑blocks)
    let full_block = find_full_block_for_seqno(net, mc_seqno).await?;

    // determine the earliest logical‑time (lt) for this account in the same master‑block
    let min_lt = compute_min_lt(&tx.tx, &base_tx.address, &full_block);
    // find all transactions between the earliest one and the emulated transaction to correctly
    // recreate all state before execution of the emulated transaction
    let mut prev_txs_in_block = find_all_transactions_between(net, &base_tx, min_lt).await?;
    // order oldest → newest, and remove the base_tx itself (the one we want to retrace)
    prev_txs_in_block.reverse();
    let Some(our_tx) = prev_txs_in_block.pop() else {
        anyhow::bail!("Cannot find transaction to retrace")
    };

    // retrieve block config to pass it to emulator
    let block_config = get_block_config(net, &full_block).await?;
    // load an account snapshot *before* the master‑block N
    let mut shard_account = get_block_account(net, &base_tx.address, &full_block).await?;

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
        block_config.clone(),
        shard_account,
        libs.as_ref(),
        rand_seed,
    )?;
    let res = match tx_res {
        EmulationResult::Success(res) => res,
        EmulationResult::Error(err) => {
            anyhow::bail!("Emulated transaction failed: {:?}", err.error);
        }
    };

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
        code_cell: loaded_code.or(code_cell.clone()),
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
            utime: emulated_tx.now as u64,
            lt: emulated_tx.lt,
            compute_info,
            executor_logs,
            actions: final_actions,
            c5,
            vm_logs: res.vm_log,
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
        let (tx_res, _) = emulate(
            prev_tx,
            block_config.to_owned(),
            shard_account.clone(),
            libs,
            rand_seed,
        )?;
        let res = match tx_res {
            EmulationResult::Success(res) => res,
            EmulationResult::Error(err) => {
                anyhow::bail!("Previous transaction failed: {:?}", err.error);
            }
        };
        // since we change state at each transaction we need to save new state as current one
        shard_account = Boc::decode_base64(&res.shard_account)?.parse()?;
        balance = shard_account
            .load_account()?
            .map(|a| a.balance.tokens)
            .unwrap_or(Tokens::ZERO);
    }
    Ok((balance, shard_account))
}

/// Helper function to run a single transaction through the TVM executor.
fn emulate(
    tx: &Transaction,
    block_config: String,
    shard_account: ShardAccount,
    libs: Option<&Cell>,
    rand_seed: [u8; 32],
) -> anyhow::Result<(EmulationResult, String)> {
    let Some(in_msg) = &tx.in_msg else {
        anyhow::bail!("No in_message was found in transaction")
    };

    let emulator = Executor::new(
        ExecutorVerbosity::FullLocationStackVerbose,
        Some(&block_config),
    )?;
    let (tx_res, executor_logs) = emulator.run_transaction(
        &Boc::encode_base64(in_msg),
        RunTransactionArgs {
            libs: libs.map(Boc::encode_base64),
            shard_account: Boc::encode_base64(to_cell(&shard_account)),
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
    Ok((tx_res, executor_logs))
}

fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("Failed to store data into cell builder");
    builder.build().expect("Failed to build cell from builder")
}
