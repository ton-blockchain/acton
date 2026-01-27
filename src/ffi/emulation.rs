use crate::commands::common::error_fmt;
use crate::context::{Context, KnownAddress, Wallet, to_cell};
use crate::debugger::any_executor::AnyExecutor;
use crate::debugger::debug_context::StepMode;
use crate::ffi::assert::process_txs_and_search_params;
use crate::formatter::FormatterContext;
use abi::contract_abi;
use acton_config::config::Explorer;
use anyhow::Context as AnyhowContext;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use emulator::{extension, register_ext_methods, remote};
use log::{debug, info, warn};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant, UNIX_EPOCH};
use ton_api::{Network, TonApiClient, TonCenterTransaction};
use ton_executor::BaseExecutor;
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::message::step::StepExecutor;
use ton_executor::message::{EmulationResult, RunTransactionArgs};
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::{MsgAddrIntStd, MsgAddress};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Load, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, ComputePhaseSkipReason, HashUpdate, IntAddr,
    LibDescr, MsgInfo, OrdinaryTxInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount,
    SkippedComputePhase, Transaction, TxInfo,
};

extension!(build in (Context) with (path: String, name: String) using build_impl);
fn build_impl(
    ctx: &mut Context,
    stk: &mut Tuple,
    path: String,
    name: String,
) -> anyhow::Result<()> {
    debug!("Building {name}");
    let id = name.clone();
    let start_time = Instant::now();

    let mut path = path;
    let mut name = name;

    if path.is_empty() {
        debug!("No path provided, search in contracts");
        let found_contract = ctx.env.find_contract(name.as_str());

        if let Some(found_contract) = found_contract {
            debug!("Found contract with info: {found_contract:?}");
            name = found_contract.name; // use actual name instead of id
            path = found_contract.src;
            path = fs::canonicalize(&path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(path);
        } else {
            anyhow::bail!(error_fmt::contract_not_found(ctx.env.config, &name));
        }
    }

    if let Some(override_code) = ctx.env.build_override.get(id.as_str()) {
        debug!("Overriding code for {name}");
        stk.push(TupleItem::Cell(override_code.clone()));
        return Ok(());
    }

    // TODO: add test for this case
    if path.ends_with(".boc") {
        // For BoC source we just return it as a Cell
        let binary_data =
            fs::read(&path).with_context(|| format!("Cannot read BoC file {path}"))?;
        let cell = ArcCell::from_boc(binary_data.as_slice())
            .map_err(|e| anyhow::anyhow!("Failed to decode code BoC for {path}: {e}"))?;
        stk.push(TupleItem::Cell(cell));
        return Ok(());
    }

    if let Some(cached) = ctx.build.build_cache.built.get(&path) {
        let elapsed = start_time.elapsed();
        info!("Build {path} from memory cache in {elapsed:?}");

        let code_cell = ArcCell::from_boc_b64(&cached.code_boc64)
            .map_err(|e| anyhow::anyhow!("Failed to decode cached code BoC for {path}: {e}"))?;
        stk.push(TupleItem::Cell(code_cell));
        return Ok(());
    }

    if let Some(cached_entry) =
        ctx.build
            .file_build_cache
            .get(&path, ctx.build.need_debug_info, 2, "1.2".to_string())
    {
        let elapsed = start_time.elapsed();
        info!("Build {path} from file cache (.acton/cache) in {elapsed:?}");

        let content = fs::read_to_string(&path).unwrap_or_default();
        ctx.build.build_cache.memoize(
            &name,
            &path,
            &cached_entry.code_boc64,
            &cached_entry.code_hash_hex,
            cached_entry.source_map.clone().unwrap_or_default(),
            Some(contract_abi(&content, &path)),
        );

        let code_cell = ArcCell::from_boc_b64(&cached_entry.code_boc64)
            .map_err(|e| anyhow::anyhow!("Failed to decode cached code BoC for {path}: {e}"))?;
        stk.push(TupleItem::Cell(code_cell));
        return Ok(());
    }

    let compile_start = Instant::now();
    let result = tolkc::compile(Path::new(&path), ctx.build.need_debug_info);
    let compile_time = compile_start.elapsed();

    match result {
        tolkc::CompilerResult::Success(success) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Build {path} from source (compilation: {compile_time:?}, total: {total_elapsed:?})"
            );

            if let Err(err) = ctx.build.file_build_cache.put(
                &path,
                &success,
                ctx.build.need_debug_info,
                2,
                "1.2".to_string(),
            ) {
                warn!("Failed to build cached code BoC for {path}: {err}");
            }

            let content = fs::read_to_string(&path).unwrap_or_default();
            ctx.build.build_cache.memoize(
                &name,
                &path,
                &success.code_boc64,
                &success.code_hash_hex,
                success.source_map.unwrap_or_default(),
                Some(contract_abi(&content, &path)),
            );
            let code_cell = ArcCell::from_boc_b64(&success.code_boc64).map_err(|e| {
                anyhow::anyhow!("Failed to decode compiled code BoC for {path}: {e}")
            })?;
            stk.push(TupleItem::Cell(code_cell));
        }
        tolkc::CompilerResult::Error(error) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Build {} failed after {:?}: {}",
                path, total_elapsed, error.message
            );

            anyhow::bail!("Compilation failed: {}", error.message);
        }
    }

    Ok(())
}

extension!(send_message in (Context) with (mode: BigInt, src: ArcCell, msg: ArcCell) using send_message_impl);
fn send_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    _: BigInt,
    src: ArcCell,
    msg: ArcCell,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;

    let msg_cell = Boc::decode(msg.to_boc(false)?)?;
    let src_cell = Boc::decode(src.to_boc(false)?)?;

    let src_addr = src_cell.parse::<IntAddr>().map_err(|err| {
        let from_slice = src_cell.as_slice_allow_exotic();
        anyhow::anyhow!(
            "Failed to decode src address from x{{{}}} with length={}: {}",
            from_slice.display_data(),
            from_slice.size_bits(),
            err
        )
    })?;

    if let Some(wallet) = ctx.env.find_wallet_by_address(&src_addr) {
        send_wallet_message(&msg, wallet, &ctx.network(), &ctx.env.api_key)
            .context("Failed to send message to real network")?;

        // Add pseudo transaction to the result list to wait on it
        let tx = Transaction {
            account: Default::default(),
            lt: 0,
            prev_trans_hash: Default::default(),
            prev_trans_lt: 0,
            now: ctx.chain.world_state.get_now(),
            out_msg_count: Default::default(),
            orig_status: AccountStatus::Uninit,
            end_status: AccountStatus::Uninit,
            in_msg: Some(msg_cell),
            out_msgs: Default::default(),
            total_fees: Default::default(),
            state_update: Lazy::new(&HashUpdate {
                old: Default::default(),
                new: Default::default(),
            })
            .expect("Invalid state update"),
            info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
                credit_first: false,
                storage_phase: None,
                credit_phase: None,
                compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                    reason: ComputePhaseSkipReason::NoState,
                }),
                action_phase: None,
                aborted: false,
                bounce_phase: None,
                destroyed: false,
            }))
            .expect("Invalid transaction info"),
        };

        let tx_cell = to_cell(&tx);
        let tx_cell = ArcCell::from_boc_hex(&Boc::encode_hex(tx_cell))
            .expect("Unreachable, cannot decode/encode cell");

        let transaction_cells = vec![TupleItem::Tuple(Tuple(vec![
            TupleItem::Cell(tx_cell),
            TupleItem::Tuple(Tuple::empty()),
            TupleItem::Null,
            TupleItem::Cell(ArcCell::default()),
            TupleItem::Tuple(Tuple::empty()),
            TupleItem::Int(BigInt::ZERO),
            TupleItem::Tuple(Tuple::empty()),
        ]))];
        stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
        return Ok(());
    }

    let libs = ctx.chain.build_libs(&src_addr);
    let world_state = &mut ctx.chain.world_state;

    let emulations = if ctx.debug.is_enabled() {
        send_message_debug(ctx, &msg_cell, &libs, Some(src_addr))?
    } else {
        emulator.send_message(world_state, msg_cell, &libs, Some(src_addr))?
    };

    if let [SendMessageResult::Error(error), ..] = &emulations[..]
        && emulations.len() == 1
    {
        ctx.asserts
            .fail(format!("Cannot send message: {}", error.error));
    }

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some(res),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(emulation_to_send_result)
        .collect::<Vec<_>>();

    ctx.chain
        .emulations
        .save_message(&ctx.env.running_id, emulations);
    stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
    Ok(())
}

fn emulation_to_send_result(emulation: &SendMessageResultSuccess) -> Option<TupleItem> {
    let Ok(tx) = ArcCell::from_boc_b64(&emulation.raw_transaction) else {
        return None;
    };
    let child_txs = Tuple(
        emulation
            .child_transactions
            .iter()
            .map(|lt| TupleItem::Int(BigInt::from(*lt)))
            .collect::<Vec<_>>(),
    );
    let parent_lt = match &emulation.parent_transaction {
        Some(parent_lt) => TupleItem::Int(BigInt::from(*parent_lt)),
        None => TupleItem::Null,
    };
    let actions = match &emulation.actions {
        Some(actions_b64) => {
            ArcCell::from_boc_b64(actions_b64).unwrap_or_else(|_| ArcCell::default())
        }
        None => ArcCell::default(),
    };

    let result = tx.to_boc(false).ok()?;
    let tx_cell = Boc::decode(&result).ok()?;
    let mut tx_slice = tx_cell.as_slice().ok()?;
    let parsed_tx = Transaction::load_from(&mut tx_slice).ok()?;

    let out_messages = Tuple(
        parsed_tx
            .iter_out_msgs()
            .filter_map(Result::ok)
            .filter_map(|msg| {
                let cell = to_cell(&msg);
                let boc = Boc::encode_base64(&cell);
                ArcCell::from_boc_b64(&boc).ok()
            })
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    let gas_used = match parsed_tx.load_info() {
        Ok(TxInfo::Ordinary(info)) => match info.compute_phase {
            ComputePhase::Executed(compute) => compute.gas_used.into(),
            _ => BigInt::ZERO,
        },
        _ => BigInt::ZERO,
    };

    let externals_tuple = Tuple(
        emulation
            .externals
            .iter()
            .filter_map(|ext_cell| {
                let boc = Boc::encode_base64(ext_cell);
                ArcCell::from_boc_b64(&boc).ok()
            })
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    Some(TupleItem::Tuple(Tuple(vec![
        TupleItem::Cell(tx),
        TupleItem::Tuple(child_txs),
        parent_lt,
        TupleItem::Cell(actions),
        TupleItem::Tuple(out_messages),
        TupleItem::Int(gas_used),
        TupleItem::Tuple(externals_tuple),
    ])))
}

fn send_wallet_message(
    message: &ArcCell,
    wallet: Wallet,
    network: &str,
    api_key: &Option<String>,
) -> anyhow::Result<()> {
    let expired_at_time = std::time::SystemTime::now() + Duration::from_secs(600);
    let expire_at = expired_at_time.duration_since(UNIX_EPOCH)?.as_secs() as u32;

    let (seqno, need_state_init) = wallet.seqno(network)?;
    let external = wallet.wallet.create_external_msg(
        expire_at,
        seqno,
        need_state_init,
        vec![message.clone()],
    )?;

    if api_key.is_none() {
        std::thread::sleep(Duration::from_millis(1000)); // rate limit
    }

    let network = Network::from_str(network)?;
    let client = TonApiClient::new(network, api_key.clone())?;
    client.send_boc(&external.to_boc_b64(false)?)?;

    Ok(())
}

fn send_message_debug(
    ctx: &mut Context,
    msg_cell: &Cell,
    libs: &Dict<HashBytes, LibDescr>,
    src_addr: Option<IntAddr>,
) -> anyhow::Result<Vec<SendMessageResult>> {
    let message_obj: RelaxedMessage = msg_cell
        .parse()
        .context("Failed to load message from cell")?;

    let RelaxedMsgInfo::Int(int_message) = &message_obj.info else {
        anyhow::bail!("Emulator only supports internal messages for now");
    };

    let dest_account = ctx
        .chain
        .world_state
        .get_account(&int_message.dst.to_string());
    let code = Emulator::get_code_cell(&message_obj, &dest_account);

    let step_executor = StepExecutor::new().expect("Failed to create executor");
    let source_map = ctx
        .build
        .build_cache
        .result_for_code(&code)
        .map(|res| res.1.source_map);

    let need_to_stop_on_entry = ctx.debug.ctx().need_to_stop_child_thread_on_start();

    ctx.debug
        .ctx()
        .begin_thread(
            2,
            AnyExecutor::Message(step_executor.clone()),
            source_map,
            "Send internal message".to_string(),
            need_to_stop_on_entry,
        )
        .expect("Cannot send response");

    let msg_cell = Emulator::patch_message(msg_cell.clone(), src_addr)?;
    let prepare_result = step_executor
        .prepare_transaction(
            &Boc::encode_base64(msg_cell),
            &RunTransactionArgs {
                libs: libs.clone().into_root().map(Boc::encode_base64),
                shard_account: Boc::encode_base64(to_cell(&dest_account)),
                now: ctx.chain.world_state.get_now(),
                lt: ctx.chain.world_state.get_lt(),
                random_seed: None,
                ignore_chksig: false,
                debug_enabled: true,
                prev_blocks_info: None,
                is_tick_tock: None,
                is_tock: None,
            },
        )
        .expect("Prepare transaction failed");
    assert!(
        prepare_result.success,
        "Failed to prepare Emulator in debug mode"
    );
    if prepare_result.skipped {
        // Since compute phase is skipped, we don't need to run anything
        ctx.debug
            .ctx()
            .finish_thread(2)
            .context("Cannot send response")?;
        return Ok(vec![]);
    }

    // Step to update internal state
    if need_to_stop_on_entry {
        ctx.debug.ctx().step(StepMode::StepIn);
    } else {
        ctx.debug.ctx().step(StepMode::Continue);
    }

    if !ctx.debug.ctx().stepper.is_terminated() {
        // Process requests only if we have something to execute and generates a requests
        ctx.debug
            .ctx()
            .process_incoming_requests(false)
            .context("Cannot send response")?;
    }

    let result = step_executor
        .finish_transaction()
        .context("Cannot finish transaction")?;

    ctx.debug
        .ctx()
        .finish_thread(2)
        .context("Cannot send response")?;

    if ctx.debug.ctx().performing_step != Some(StepMode::Continue) {
        // When we step out from nested message/get method, send stop message to client to
        // stop on a line after send/call get method
        ctx.debug.ctx().step(StepMode::StepIn);
    }

    let result = match result {
        EmulationResult::Success(result) => result,
        EmulationResult::Error(_) => {
            return Ok(vec![]);
        }
    };

    let shard_account_after = &result.shard_account;
    let shard_account_cell =
        Boc::decode_base64(shard_account_after).context("Failed to decode shard account BoC")?;
    let shard_account: ShardAccount = shard_account_cell
        .parse()
        .context("Failed to load shard account from cell")?;

    ctx.chain
        .world_state
        .update_account(&int_message.dst.to_string(), &shard_account);

    let tx_cell =
        Boc::decode_base64(&result.transaction).context("Failed to decode transaction BoC")?;
    let transaction: Transaction = tx_cell
        .parse()
        .context("Failed to load transaction from cell")?;

    let out_messages = transaction
        .iter_out_msgs()
        .filter_map(Result::ok)
        .map(|it| to_cell(&it))
        .collect::<Vec<_>>();

    let code = Emulator::get_code_cell(&message_obj, &dest_account);

    let send_result = SendMessageResultSuccess {
        raw_transaction: result.transaction,
        transaction: transaction.clone(),
        parent_transaction: None,
        child_transactions: vec![],
        shard_account_before: dest_account,
        shard_account,
        out_messages,
        vm_log: result.vm_log,
        executor_logs: String::new(),
        actions: result.actions,
        code,
        externals: vec![],
    };

    let mut externals: Vec<Cell> = vec![];

    let mut all_results = std::iter::once(SendMessageResult::Success(send_result))
        .chain(transaction.iter_out_msgs().flat_map(|msg| {
            let Ok(msg) = msg else { return vec![] };

            if let MsgInfo::ExtOut(_) = &msg.info {
                externals.push(to_cell(&msg));
                return vec![];
            }

            let mut send_results =
                send_message_debug(ctx, &to_cell(&msg), libs, None).unwrap_or_default();
            for result in &mut send_results {
                match result {
                    SendMessageResult::Success(result) => {
                        result.parent_transaction = Some(transaction.lt);
                    }
                    SendMessageResult::Error(_) => {}
                }
            }

            send_results
        }))
        .collect::<Vec<_>>();

    let child_txs = all_results
        .iter()
        .skip(1)
        .filter_map(|result| match result {
            SendMessageResult::Success(result) => Some(result.transaction.lt),
            SendMessageResult::Error(_) => None,
        })
        .collect();

    if let Some(SendMessageResult::Success(result)) = all_results.get_mut(0) {
        result.child_transactions = child_txs;
        result.externals = externals;
    }

    Ok(all_results)
}

extension!(send_single_message in (Context) with (src: ArcCell, msg: ArcCell) using send_single_message_impl);
fn send_single_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: ArcCell,
    msg: ArcCell,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;

    let msg_cell = Boc::decode(msg.to_boc(false)?)?;
    let src_cell = Boc::decode(src.to_boc(false)?)?;

    let src_addr = src_cell.parse::<IntAddr>().map_err(|err| {
        let from_slice = src_cell.as_slice_allow_exotic();
        anyhow::anyhow!(
            "Failed to decode src address from x{{{}}} with length={}: {}",
            from_slice.display_data(),
            from_slice.size_bits(),
            err
        )
    })?;

    let libs = ctx.chain.build_libs(&src_addr);
    let world_state = &mut ctx.chain.world_state;

    let emulation = emulator
        .send_transaction(world_state, msg_cell, &libs, Some(src_addr))
        .context("Cannot emulate transaction")?;

    let SendMessageResult::Success(emulation) = emulation else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let Some(send_result) = emulation_to_send_result(&emulation) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    ctx.chain.emulations.save_message(
        &ctx.env.running_id,
        vec![SendMessageResult::Success(emulation)],
    );

    stack.push(send_result);
    Ok(())
}

extension!(find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple) using find_transaction_by_params_impl);
fn find_transaction_by_params_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
) -> anyhow::Result<()> {
    if txs.0.is_empty() {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let (params, parsed_txs) = if let Some(value) = process_txs_and_search_params(&txs, params) {
        value
    } else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let found = parsed_txs.iter().filter(|tx| {
        if let Some(expected_deploy) = params.deploy
            && expected_deploy
            && (tx.orig_status != AccountStatus::NotExists
                || tx.end_status != AccountStatus::Active)
        {
            // We expect to deploy contract but we don't
            return false;
        }

        let in_msg = tx.load_in_msg();
        if let Ok(Some(in_msg)) = &in_msg
            && let MsgInfo::Int(info) = &in_msg.info
        {
            if let Some(expected_opcode) = &params.opcode {
                let mut slice = in_msg.body;
                let Ok(opcode) = slice.load_u32() else {
                    // No opcode at all
                    return false;
                };
                if *expected_opcode != opcode {
                    if params.bounced == Some(true) {
                        // if bounced, try to match opcode after 0xFFFFFFFF
                        let Ok(bounced_opcode) = slice.load_u32() else {
                            // No bounced opcode at all
                            return false;
                        };
                        if *expected_opcode != bounced_opcode {
                            // Bounced opcode mismatch
                            return false;
                        }
                    } else {
                        // Opcode mismatch
                        return false;
                    }
                }
            }

            if let Some(expected_bounced) = &params.bounced
                && *expected_bounced != info.bounced
            {
                // Bounced value mismatch
                return false;
            }

            if let Some(expected_bounce) = &params.bounce
                && *expected_bounce != info.bounce
            {
                // Bounce value mismatch
                return false;
            }

            if let Some(expected_from_addr) = &params.from
                && (*expected_from_addr) != info.src
            {
                // Source address mismatch
                return false;
            }

            if let Some(expected_to_addr) = &params.to
                && (*expected_to_addr) != info.dst
            {
                // Destination address mismatch
                return false;
            }

            if let Some(expected_body) = &params.body {
                let expected_hash = expected_body.repr_hash();
                let body_cell = to_cell(&in_msg.body);
                let actual_hash = body_cell.repr_hash();
                if expected_hash != actual_hash {
                    // Message body hash mismatch
                    return false;
                }
            }
        }

        let Ok(TxInfo::Ordinary(info)) = tx.load_info() else {
            return false;
        };

        if let Some(expected_compute_skipped) = params.compute_phase_skipped {
            let is_skipped = matches!(info.compute_phase, ComputePhase::Skipped(_));
            if expected_compute_skipped != is_skipped {
                // Compute phase skipped mismatch
                return false;
            }
        }

        if let Some(expected_aborted) = params.aborted
            && expected_aborted != info.aborted
        {
            // Aborted mismatch
            return false;
        }

        if let Some(expected_action_exit_code) = params.action_exit_code {
            if let Some(action_phase) = &info.action_phase {
                if action_phase.result_code != expected_action_exit_code {
                    // Action exit code mismatch
                    return false;
                }
            } else {
                // Action phase is missing but expected
                return false;
            }
        }

        if let ComputePhase::Executed(compute) = &info.compute_phase
            && let Some(expected_exit_code) = params.exit_code
            && compute.exit_code != expected_exit_code as i32
        {
            // Exit code mismatch
            return false;
        }

        if let Some(expected_success) = params.success {
            let action_phase_success = if let Some(action_phase) = &info.action_phase {
                action_phase.success
            } else {
                false // np action phase, no success
            };

            if let ComputePhase::Executed(compute) = &info.compute_phase
                && (action_phase_success && compute.success) != expected_success
            {
                // Success mismatch
                return false;
            }
        }

        true
    });

    let txs = found.collect::<Vec<_>>();
    let Some(first) = txs.first() else {
        // No transaction found
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let tx_boc = Boc::encode(to_cell(&first));
    let tx_cell = ArcCell::from_boc(&tx_boc)?;

    stack.push(TupleItem::Cell(tx_cell));
    Ok(())
}

extension!(run_get_method in (Context) with (args: Tuple, return_type_name: String, name: String, id: BigInt, code: ArcCell, address: ArcCell) using run_get_method_impl);
#[allow(clippy::too_many_arguments)]
fn run_get_method_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    args: Tuple,
    return_type_name: String,
    name: String,
    id: BigInt,
    code: ArcCell,
    address: ArcCell,
) -> anyhow::Result<()> {
    let args = args.unwrap_empty().unwrap_tuple();
    let world_state = &mut ctx.chain.world_state;
    let address_boc = address
        .to_boc_hex(false)
        .context("Failed to encode address to BOC hex")?;

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str())
        .context("Failed to parse address from BOC hex")?;
    let address_hash = address_std.address.clone();
    let dst_addr_str = format!("{}:{}", &address_std.workchain, hex::encode(&address_hash));

    let dest_address =
        TonAddress::from_msg_address(address_std).context("Failed to create TonAddress")?;

    let shard_account = world_state.get_account(&dst_addr_str);
    let state = shard_account
        .account
        .load()
        .context("Failed to load account")?
        .0
        .map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or_default()
    } else {
        Cell::default()
    };

    let libs = ctx
        .chain
        .build_libs_with_hash_owner(&HashBytes::from_slice(address_hash.as_slice()));
    let libs_root = libs.into_root();
    let world_state = &mut ctx.chain.world_state;

    let method_id = id.to_i32().unwrap_or(0);

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: code.to_boc_b64(false)?,
        data: Boc::encode_base64(data),
        verbosity: ctx.env.default_log_level,
        libs: libs_root.map(Boc::encode_base64).unwrap_or_default(),
        address: dest_address.to_string(),
        unixtime: duration_since_epoch.as_secs().try_into()?,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let config_b64 = world_state
        .get_config()
        .root()
        .clone()
        .map(Boc::encode_base64)
        .expect("Config has no root");

    let result = if ctx.debug.is_enabled() {
        let args = serialize_tuple(&args)
            .map(|t| t.to_boc_b64(false))
            .context("Cannot serialize tuple")?
            .context("Cannot serialize tuple")?;

        let step_executor = StepGetExecutor::new(&args, &params, Some(&config_b64))
            .context("Cannot create get executor")?;

        let source_map = ctx
            .build
            .build_cache
            .result_for_code(&Some(Boc::decode(code.to_boc(false)?)?))
            .map(|res| res.1.source_map);

        let dbg_ctx = ctx.debug.ctx();
        dbg_ctx
            .begin_thread(
                2,
                AnyExecutor::Get(step_executor.clone()),
                source_map,
                "Send internal message".to_string(),
                dbg_ctx.need_to_stop_child_thread_on_start(),
            )
            .context("Cannot send response")?;

        step_executor
            .prepare(method_id, &args)
            .context("Cannot prepare get method")?;

        // Step to update internal state
        if dbg_ctx.need_to_stop_child_thread_on_start() {
            dbg_ctx.step(StepMode::StepIn);
        } else {
            dbg_ctx.step(StepMode::Continue);
        }

        if !dbg_ctx.stepper.is_terminated() {
            dbg_ctx
                .process_incoming_requests(false)
                .context("Cannot send response")?;
        }

        dbg_ctx.finish_thread(2).context("Cannot send response")?;

        if dbg_ctx.performing_step != Some(StepMode::Continue) {
            // When we step out from nested message/get method, send stop message to client to
            // stop on a line after send/call get method
            dbg_ctx.step(StepMode::StepIn);
        }

        step_executor
            .finish(&params.code)
            .context("Cannot run get method")?
    } else {
        let executor = GetExecutor::new(&params).context("Cannot create get executor")?;
        let args = serialize_tuple(&args)
            .map(|t| t.to_boc_b64(false))
            .context("Cannot serialize tuple")?
            .context("Cannot serialize tuple")?;
        executor
            .run_get_method(&args, &params, Some(&config_b64))
            .context("Cannot run get method")?
    };

    match result {
        GetMethodResult::Success(result) => {
            ctx.chain
                .emulations
                .save_get_method(&ctx.env.running_id, result.clone());

            let cell =
                ArcCell::from_boc_b64(&result.stack).context("Failed to decode stack BoC")?;
            let tuple = Tuple::deserialize(&cell).context("Failed to deserialize tuple")?;

            if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
                let get_method = ctx.env.abi.find_get_method_by_id(&id);
                let get_method_presentation = if let Some(get_method) = get_method {
                    format!("'{}' ({id})", get_method.name)
                } else {
                    format!("'{name}' ({id})")
                };

                if result.vm_exit_code == 11 {
                    // TODO: right now get methods may not include all get methods
                    let get_methods: Vec<&str> = ctx
                        .env
                        .abi
                        .get_methods
                        .iter()
                        .map(|m| m.name.as_str())
                        .collect();
                    let suggested_name = suggest_name(&name, &get_methods);

                    if let Some(suggested_name) = suggested_name {
                        anyhow::bail!(
                            "Cannot execute unknown get method {get_method_presentation}, did you mean '{suggested_name}'",
                        );
                    }
                    anyhow::bail!("Cannot execute unknown get method {get_method_presentation}",);
                } else if result.vm_exit_code == 2 {
                    anyhow::bail!(
                        "Get method {get_method_presentation} failed due to stack underflow. Make sure you passed all parameters to the get method.",
                    );
                }
                anyhow::bail!(
                    "Cannot execute get method {get_method_presentation}: exit code {}",
                    FormatterContext::format_exit_code(result.vm_exit_code)
                );
            }

            stack.push(TupleItem::TypedTuple {
                type_name: return_type_name,
                inner: tuple,
            });
        }
        GetMethodResult::Error(result) => {
            println!("Error: {}", result.error);
        }
    }

    Ok(())
}

fn suggest_name<'a>(input: &str, candidates: &'a [&'a str]) -> Option<&'a str> {
    let mut best = None;
    let mut best_dist = usize::MAX;

    for &cand in candidates {
        let d = strsim::levenshtein(input, cand);
        if d < best_dist {
            best_dist = d;
            best = Some(cand);
        }
    }

    if best_dist <= 3 { best } else { None }
}

extension!(is_deployed in (Context) with (address: ArcCell) using is_deployed_impl);
fn is_deployed_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) -> anyhow::Result<()> {
    // TODO: looks ugly
    let dst_addr_str = cell_address_to_raw(address).context("Failed to decode address")?;

    let is_deployed = ctx.chain.world_state.check_deployed(&dst_addr_str);
    stack.push_bool(is_deployed);
    Ok(())
}

extension!(get_deployed_code in (Context) with (addr: ArcCell) using get_deployed_code_impl);
fn get_deployed_code_impl(ctx: &mut Context, stk: &mut Tuple, addr: ArcCell) -> anyhow::Result<()> {
    let dst_addr_str = cell_address_to_raw(addr).context("Failed to decode address")?;

    let is_deployed = ctx.chain.world_state.check_deployed(&dst_addr_str);
    if !is_deployed {
        stk.push(TupleItem::Null);
        return Ok(());
    }

    let account = ctx.chain.world_state.get_account(&dst_addr_str);
    let cell = if let Some(value) = get_address_code(&account) {
        value
    } else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    stk.push(TupleItem::Cell(cell));
    Ok(())
}

fn cell_address_to_raw(address: ArcCell) -> anyhow::Result<String> {
    let address_std = TonAddress::from_msg_address(MsgAddress::from_cell(&address)?)?;
    Ok(address_std.to_hex())
}

fn get_address_code(account: &ShardAccount) -> Option<ArcCell> {
    let state = account.account.load().ok()?.0.map(|s| s.state);

    let Some(AccountState::Active(state)) = state else {
        return None;
    };

    let code = state.code?;
    let cell = ArcCell::from_boc_b64(&Boc::encode_base64(code)).ok()?;

    Some(cell)
}

extension!(crc16 in (Context) with (data: String) using crc16_impl);
fn crc16_impl(_ctx: &mut Context, stack: &mut Tuple, data: String) -> anyhow::Result<()> {
    let crc = Crc::<u16>::new(&CRC_16_XMODEM);
    let result = crc.checksum(data.as_bytes());
    stack.push(TupleItem::Int(BigInt::from(result)));
    Ok(())
}

extension!(type_name_by_opcode in (Context) with (id: BigInt) using type_name_by_opcode_impl);
fn type_name_by_opcode_impl(ctx: &mut Context, stk: &mut Tuple, id: BigInt) -> anyhow::Result<()> {
    let id = u32::try_from(&id).context("ID is too big for uint32 opcode")?;
    let type_abi = ctx.env.abi.find_type_by_opcode(id);
    match type_abi {
        None => {
            stk.push(TupleItem::Null);
        }
        Some(type_abi) => {
            stk.push_string(&type_abi.name);
        }
    }
    Ok(())
}

extension!(register_address in (Context) with (name: String, address: ArcCell) using register_address_impl);
fn register_address_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    name: String,
    address: ArcCell,
) -> anyhow::Result<()> {
    let address_boc = address
        .to_boc(false)
        .context("Failed to encode address to BoC")?;
    let address_cell = Boc::decode(address_boc).context("Failed to decode address from BoC")?;

    let addr = address_cell
        .parse::<IntAddr>()
        .context("Failed to load address from slice")?;

    ctx.build
        .known_addresses
        .addresses
        .insert(addr, KnownAddress { name });
    Ok(())
}

extension!(register_code in (Context) with (name: String, address: ArcCell) using register_code_impl);
fn register_code_impl(
    ctx: &mut Context,
    _: &mut Tuple,
    name: String,
    code: ArcCell,
) -> anyhow::Result<()> {
    let hash = code.cell_hash().context("Failed to get cell hash")?;
    ctx.build.known_code_cells.insert(hash.to_hex(), name);
    Ok(())
}

extension!(account_state in (Context) with (addr: ArcCell) using account_state_impl);
fn account_state_impl(ctx: &mut Context, stk: &mut Tuple, addr: ArcCell) -> anyhow::Result<()> {
    let address_cell = Boc::decode(addr.to_boc(false)?)?;
    let addr = address_cell
        .parse::<IntAddr>()
        .context("Failed to load internal address from slice")?;

    let Ok(account) = ctx
        .chain
        .world_state
        .get_account(&addr.to_string())
        .account
        .load()
        .map_err(|e| anyhow::anyhow!("Failed to load account: {e}"))
    else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    let Some(account) = account.0 else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    let mut builder = CellBuilder::new();
    builder.store_bit(true)?;
    account.store_into(&mut builder, Cell::empty_context())?;
    let cell = builder.build()?;

    let Ok(cell) = ArcCell::from_boc_b64(&Boc::encode_base64(cell)) else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    stk.push(TupleItem::Cell(cell));
    Ok(())
}

extension!(register_lib in (Context) with (lib: ArcCell) using register_lib_impl);
fn register_lib_impl(ctx: &mut Context, _stack: &mut Tuple, lib: ArcCell) -> anyhow::Result<()> {
    let cell = Boc::decode(lib.to_boc(false)?)?;
    ctx.chain.world_state.register_lib(cell);
    Ok(())
}

extension!(convert_address in (Context) with (address: String) using convert_address_impl);
fn convert_address_impl(_: &mut Context, stack: &mut Tuple, address: String) -> anyhow::Result<()> {
    let addr = TonAddress::from_str(address.as_str())
        .with_context(|| format!("Failed to convert address from {address}"))?;
    let cell = addr.to_msg_address().to_cell()?;
    stack.push(TupleItem::Cell(cell.to_arc()));
    Ok(())
}

extension!(cell_from_hex in (Context) with (cell_hex: String) using cell_from_hex_impl);
fn cell_from_hex_impl(_: &mut Context, stack: &mut Tuple, cell_hex: String) -> anyhow::Result<()> {
    let cell = ArcCell::from_boc_hex(cell_hex.as_str())
        .with_context(|| format!("Failed to decode cell hex {cell_hex}"))?;
    stack.push(TupleItem::Cell(cell));
    Ok(())
}

extension!(load_library_by_hash in (Context) with (hash: String) using load_library_by_hash_impl);
fn load_library_by_hash_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    hash: String,
) -> anyhow::Result<()> {
    let lib = remote::get_library_by_hash(&ctx.network(), hash.as_str(), None);
    match lib {
        Ok(lib) => {
            let cell = ArcCell::from_boc(&Boc::encode(lib))?;
            stack.push(TupleItem::Cell(cell));
        }
        Err(_) => stack.push(TupleItem::Null),
    }

    Ok(())
}

extension!(is_broadcasting in (Context) using is_broadcasting_impl);
fn is_broadcasting_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    stack.push_bool(ctx.is_broadcasting);
    Ok(())
}

extension!(get_wallet_by_name in (Context) with (name: String) using get_wallet_by_name_impl);
fn get_wallet_by_name_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    name: String,
) -> anyhow::Result<()> {
    if let Some(wallet) = ctx.env.open_wallets.get(&name) {
        let cell = wallet.address().to_msg_address().to_cell();
        let cell = cell.context("Cannot build cell from wallet address:")?;
        stack.push(TupleItem::Cell(cell.to_arc()));
        return Ok(());
    }

    stack.push(TupleItem::Null);

    Ok(())
}

extension!(wait_for_transaction in (Context) with (sleep_duration: BigInt, attempts: BigInt, quiet: BigInt, ext_message_hash: ArcCell, address: ArcCell) using wait_for_transaction_impl);
#[allow(clippy::too_many_arguments)]
fn wait_for_transaction_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    sleep_duration: BigInt,
    attempts: BigInt,
    quiet: BigInt,
    ext_message_hash: ArcCell,
    address: ArcCell,
) -> anyhow::Result<()> {
    if !ctx.is_broadcasting {
        return Ok(());
    }

    let quiet = !quiet.is_zero();
    let attempts = attempts.to_u32().unwrap_or(20);
    let sleep_duration_ms = sleep_duration.to_u64().unwrap_or(2000);

    if attempts == 0 {
        anyhow::bail!("Attempt number must be positive");
    }

    let address_str = cell_address_to_raw(address).context("Failed to decode address")?;

    let network = Network::from_str(&ctx.network()).context("Failed to parse network")?;

    let api_key = ctx.env.api_key.clone();
    let api_client = TonApiClient::new(network, api_key.clone())?;

    let ext_message_hash_bytes = ext_message_hash.data();

    if api_key.is_none() {
        std::thread::sleep(Duration::from_millis(1000)); // rate limit
    }

    for attempt in 1..=attempts {
        if !quiet {
            println!("Awaiting transaction... [Attempt {attempt}/{attempts}]");
        }

        let txs = if let Ok(txs) = api_client.get_transactions(&address_str, Some(100), None, None)
        {
            txs
        } else {
            std::thread::sleep(Duration::from_millis(sleep_duration_ms));
            continue;
        };

        for tx in txs {
            if let Some(in_msg) = &tx.in_msg
                && let Some(body_hash) = &in_msg.body_hash
            {
                let msg_hash_bytes = base64::engine::general_purpose::STANDARD
                    .decode(body_hash)
                    .context("Failed to decode message body hash")?;

                if msg_hash_bytes == ext_message_hash_bytes {
                    std::thread::sleep(Duration::from_millis(1000)); // wait a bit more for txs in row

                    if !quiet {
                        let hex = base64::engine::general_purpose::STANDARD
                            .decode(tx.transaction_id.hash.clone())
                            .map(hex::encode)
                            .unwrap_or_else(|_| tx.transaction_id.hash.clone());
                        println!("Transaction successfully applied!");

                        let url = get_transaction_link(ctx, address_str, tx, hex);
                        println!("You can view it at {}", url.underline());
                    }
                    stack.push_bool(true);
                    return Ok(());
                }
            }
        }

        if attempt < attempts {
            std::thread::sleep(Duration::from_millis(sleep_duration_ms));
        }
    }

    ctx.asserts.fail(
        "Transaction was not applied after {} attempts. Check your wallet's transactions"
            .to_owned(),
    );
    stack.push_bool(false);
    Ok(())
}

fn get_transaction_link(
    ctx: &mut Context,
    address_str: String,
    tx: TonCenterTransaction,
    hex: String,
) -> String {
    let network_prefix = if ctx.network() == "testnet" {
        "testnet."
    } else {
        ""
    };
    let explorer = ctx.env.explorer.unwrap_or(Explorer::Tonviewer);
    match explorer {
        Explorer::Tonscan => {
            format!("https://{network_prefix}tonscan.org/tx/{hex}")
        }
        Explorer::Toncx => format!(
            "https://{}ton.cx/tx/{}:{}:{}",
            network_prefix, tx.transaction_id.lt, hex, address_str
        ),
        Explorer::Dton => format!(
            "https://{}dton.io/tx/{}?time={}",
            network_prefix, hex, tx.utime
        ),
        Explorer::Tonviewer => format!("https://{network_prefix}tonviewer.com/transaction/{hex}"),
    }
}

extension!(enable_broadcast in (Context) using enable_broadcast_impl);
const fn enable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) -> anyhow::Result<()> {
    ctx.is_broadcasting = true;
    Ok(())
}

extension!(get_config in (Context) using get_config_impl);
fn get_config_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    let config = ctx.chain.world_state.get_config();
    let config_cell = config
        .root()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Config has no root cell"))?;
    let arc = ArcCell::from_boc(&Boc::encode(config_cell))
        .map_err(|e| anyhow::anyhow!("Failed to encode config from world state: {e}"))?;

    stack.push(TupleItem::Cell(arc));
    Ok(())
}

extension!(set_config in (Context) with (config: ArcCell) using set_config_impl);
fn set_config_impl(ctx: &mut Context, stack: &mut Tuple, config: ArcCell) -> anyhow::Result<()> {
    let config_boc = config.to_boc(false)?;
    let config_cell = Boc::decode(config_boc)?;

    let result = ctx
        .chain
        .emulator
        .set_config(ctx.chain.world_state, config_cell);

    match result {
        Ok(res) => {
            stack.push_bool(res);
        }
        Err(_) => {
            stack.push_bool(false);
        }
    }

    Ok(())
}

extension!(disable_broadcast in (Context) using disable_broadcast_impl);
const fn disable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) -> anyhow::Result<()> {
    ctx.is_broadcasting = false;
    Ok(())
}

extension!(set_now in (Context) with (now: BigInt) using set_now_impl);
fn set_now_impl(ctx: &mut Context, _: &mut Tuple, now: BigInt) -> anyhow::Result<()> {
    let now_u32 = now.to_u32().unwrap_or(0);
    ctx.chain.world_state.set_now(now_u32);
    Ok(())
}

extension!(get_now in (Context) using get_now_impl);
fn get_now_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    let now = ctx.chain.world_state.get_now();
    stack.push(TupleItem::Int(BigInt::from(now)));
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        6 => build,
        8 => run_get_method,
        9 => send_message,
        10 => find_transaction_by_params,
        11 => is_deployed,
        12 => get_deployed_code,
        13 => crc16,
        14 => type_name_by_opcode,
        15 => register_address,
        16 => register_code,
        17 => account_state,
        18 => register_lib,
        19 => convert_address,
        20 => cell_from_hex,
        21 => load_library_by_hash,
        23 => is_broadcasting,
        24 => get_wallet_by_name,
        25 => wait_for_transaction,
        26 => enable_broadcast,
        27 => disable_broadcast,
        28 => set_now,
        29 => get_now,
        30 => send_single_message,
        31 => get_config,
        32 => set_config,
    });
}
