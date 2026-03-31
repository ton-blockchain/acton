use crate::commands::common::error_fmt;
use crate::context::{
    AssertFailure, Context, GetMethodAssertFailure, KnownAddress, MessageIterState,
    PendingMessageStep, TransactionNotFoundParams, Wallet, to_cell,
};
use crate::debugger::any_executor::AnyExecutor;
use crate::debugger::session::ChildDebugContextSpec;
use crate::debugger::session::StepMode;
use crate::external_send::{SendBocContext, format_send_boc_error};
use crate::ffi::assert::parse_search_params;
use crate::retrace;
use acton_config::color::OwoColorize;
use acton_config::config::Explorer;
use anyhow::Context as AnyhowContext;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use log::{debug, info, warn};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use rustc_hash::FxHashSet;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tolkc::TolkSourceMap;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton_abi::contract_abi;
use ton_api::{Network, TonApiClient, TonCenterTransaction};
use ton_emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::message::step::StepExecutor;
use ton_executor::message::{EmulationResult, RunTransactionArgs};
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, ComputePhaseSkipReason, HashUpdate, IntAddr,
    LibDescr, Message, MsgInfo, OptionalAccount, OrdinaryTxInfo, RelaxedMessage, RelaxedMsgInfo,
    ShardAccount, SkippedComputePhase, StdAddr, StdAddrFormat, Transaction, TxInfo,
};

fn run_nested_executor_until_finished(
    ctx: &mut Context,
    child_debug_started: bool,
    child_step_mode: StepMode,
    mut direct_step: impl FnMut() -> bool,
) -> anyhow::Result<()> {
    if child_debug_started {
        ctx.debug.step(child_step_mode);

        if !ctx.debug.active_context_is_terminated() {
            ctx.debug
                .process_incoming_requests(false)
                .context("Cannot process nested debug requests")?;
        }

        anyhow::ensure!(
            ctx.debug.active_context_is_terminated(),
            "Nested debug context did not finish before finalization"
        );
    } else {
        while !direct_step() {}
    }

    Ok(())
}
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
            path = dunce::canonicalize(&path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(path);
        } else {
            anyhow::bail!(error_fmt::contract_not_found(ctx.env.config, &name));
        }
    }

    if !path.starts_with('@') && !Path::new(&path).is_absolute() {
        path = ctx
            .env
            .project_root
            .join(Path::new(&path))
            .to_string_lossy()
            .to_string();
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
        let cell = Boc::decode(binary_data.as_slice())
            .map_err(|e| anyhow::anyhow!("Failed to decode code BoC for {path}: {e}"))?;
        stk.push(TupleItem::Cell(cell));
        return Ok(());
    }

    if let Some(cached) = ctx.build.build_cache.built.get(Path::new(&path)) {
        let elapsed = start_time.elapsed();
        info!("Build {path} from memory cache in {elapsed:?}");

        let code_cell = Boc::decode_base64(&cached.code_boc64)
            .map_err(|e| anyhow::anyhow!("Failed to decode cached code BoC for {path}: {e}"))?;
        stk.push(TupleItem::Cell(code_cell));
        return Ok(());
    }

    if let Some(cached_entry) =
        ctx.build
            .file_build_cache
            .get(&path, ctx.build.need_debug_info, 2, "1.3")
    {
        let mappings = ctx.env.config.mappings();
        let elapsed = start_time.elapsed();
        info!("Build {path} from file cache (.acton/cache) in {elapsed:?}");

        let code_cell = Boc::decode_base64(&cached_entry.code_boc64)
            .map_err(|e| anyhow::anyhow!("Failed to decode cached code BoC for {path}: {e}"))?;
        let tolk_source_map = Arc::new(TolkSourceMap::from_code_cell(
            cached_entry.new_source_map.clone().unwrap_or_default(),
            &code_cell,
            cached_entry.debug_mark_base64.as_deref(),
        )?);
        let content: Arc<str> = fs::read_to_string(&path).unwrap_or_default().into();
        ctx.build.build_cache.memoize(
            &name,
            Path::new(&path),
            &cached_entry.code_boc64,
            HashBytes::from_str(&cached_entry.code_hash_hex)?,
            cached_entry.source_map.clone().unwrap_or_default().into(),
            tolk_source_map,
            Some(contract_abi(content, &path, &mappings).into()),
            cached_entry.abi.clone().map(Into::into),
        );

        stk.push(TupleItem::Cell(code_cell));
        return Ok(());
    }

    let compile_start = Instant::now();
    let mappings = ctx.env.config.mappings();
    let compiler = tolkc::Compiler::new(2).with_mappings(&mappings);
    let result = compiler.compile(Path::new(&path), ctx.build.need_debug_info);
    let compile_time = compile_start.elapsed();

    match result {
        tolkc::CompilerResult::Success(success) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Build {path} from source (compilation: {compile_time:?}, total: {total_elapsed:?})"
            );

            if let Err(err) =
                ctx.build
                    .file_build_cache
                    .put(&path, &success, ctx.build.need_debug_info, 2, "1.3")
            {
                warn!("Failed to build cached code BoC for {path}: {err}");
            }

            let content: Arc<str> = fs::read_to_string(&path).unwrap_or_default().into();
            let code_cell = Boc::decode_base64(&success.code_boc64).map_err(|e| {
                anyhow::anyhow!("Failed to decode compiled code BoC for {path}: {e}")
            })?;
            let tolk_source_map = Arc::new(TolkSourceMap::from_code_cell(
                success.new_source_map.unwrap_or_default(),
                &code_cell,
                success.debug_mark_base64.as_deref(),
            )?);
            ctx.build.build_cache.memoize(
                &name,
                Path::new(&path),
                &success.code_boc64,
                HashBytes::from_str(&success.code_hash_hex)?,
                success.source_map.unwrap_or_default().into(),
                tolk_source_map,
                Some(contract_abi(content, &path, &mappings).into()),
                success.abi.clone().map(Into::into),
            );
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

extension!(send_message in (Context) with (src: IntAddr, msg: Cell) using send_message_impl);
fn send_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: IntAddr,
    msg: Cell,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;

    let src_std = match &src {
        IntAddr::Std(addr) => addr,
        IntAddr::Var(_) => anyhow::bail!("Var addresses are not supported anymore"),
    };

    // Internal messages are serialized as RelaxedMessage; ExtIn messages are not.
    let is_external = match msg.parse::<RelaxedMessage<'_>>() {
        Ok(parsed) => match parsed.info {
            RelaxedMsgInfo::ExtOut(_) => {
                anyhow::bail!("External out messages can't initiate transactions!");
            }
            RelaxedMsgInfo::Int(_) => false,
        },
        Err(_) => true,
    };

    if let Some(wallet) = ctx.env.find_wallet_by_address(src_std) {
        send_wallet_message(
            &msg,
            wallet,
            &ctx.network(),
            &ctx.env.api_key,
            ctx.env.config.custom_networks(),
        )
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
            in_msg: Some(msg),
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

        let transaction_cells = vec![TupleItem::Tuple(Tuple(vec![
            TupleItem::Cell(tx_cell),
            TupleItem::Tuple(Tuple::empty()),
            TupleItem::Null,
            TupleItem::Cell(Cell::default()),
            TupleItem::Tuple(Tuple::empty()),
            TupleItem::Int(BigInt::ZERO),
            TupleItem::Tuple(Tuple::empty()),
        ]))];
        stack.push(TupleItem::big_array_from_items(transaction_cells));
        return Ok(());
    }

    let libs = ctx.chain.build_libs(&src);
    let world_state = &mut ctx.chain.world_state;

    let emulations = if ctx.debug.is_enabled() {
        send_message_debug(ctx, &msg, &libs, Some(src))
    } else {
        emulator.send_message(world_state, msg, &libs, Some(src))
    }
    .context("Cannot send message")?;

    if let [SendMessageResult::Error(error), ..] = &emulations[..]
        && emulations.len() == 1
    {
        ctx.chain
            .emulations
            .save_message(&ctx.env.running_id, emulations.clone());

        // TODO return error with type when unions are supported in ffi
        if is_external {
            stack.push(TupleItem::Null);
            return Ok(());
        }

        anyhow::bail!("Cannot send message: {}", error.error)
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
    stack.push(TupleItem::big_array_from_items(transaction_cells));
    Ok(())
}

extension!(run_tick_tock in (Context) with (is_tock: BigInt, on_account: StdAddr) using run_tick_tock_impl);
fn run_tick_tock_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    is_tock: BigInt,
    on_account: StdAddr,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;
    let is_tock = is_tock != BigInt::ZERO;

    let addr = IntAddr::Std(on_account.clone());
    let libs = ctx.chain.build_libs(&addr);

    // TODO: debug mode support for tick-tock (send_message_debug equivalent)
    let emulations = emulator
        .run_tick_tock(ctx.chain.world_state, &on_account, is_tock, &libs)
        .context("Cannot run tick-tock transaction")?;

    if let [SendMessageResult::Error(error), ..] = &emulations[..]
        && emulations.len() == 1
    {
        ctx.chain
            .emulations
            .save_message(&ctx.env.running_id, emulations.clone());

        anyhow::bail!("Cannot run tick-tock transaction: {}", error.error)
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
    stack.push(TupleItem::big_array_from_items(transaction_cells));
    Ok(())
}

fn emulation_to_send_result(emulation: &SendMessageResultSuccess) -> Option<TupleItem> {
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
            Boc::decode_base64(actions_b64.as_ref()).unwrap_or_else(|_| Cell::default())
        }
        None => Cell::default(),
    };

    let parsed_tx = &emulation.transaction;
    let out_messages = Tuple(
        parsed_tx
            .iter_out_msgs()
            .filter_map(Result::ok)
            .map(|msg| to_cell(&msg))
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    let gas_used = match parsed_tx.load_info() {
        Ok(TxInfo::Ordinary(info)) => match info.compute_phase {
            ComputePhase::Executed(compute) => compute.gas_used.into(),
            _ => BigInt::ZERO,
        },
        Ok(TxInfo::TickTock(info)) => match info.compute_phase {
            ComputePhase::Executed(compute) => compute.gas_used.into(),
            _ => BigInt::ZERO,
        },
        _ => BigInt::ZERO,
    };

    let externals_tuple = Tuple(
        emulation
            .externals
            .iter()
            .cloned()
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    let tx_cell = to_cell(&emulation.transaction);

    Some(TupleItem::Tuple(Tuple(vec![
        TupleItem::Cell(tx_cell),
        TupleItem::Tuple(child_txs),
        parent_lt,
        TupleItem::Cell(actions),
        TupleItem::Tuple(out_messages),
        TupleItem::Int(gas_used),
        TupleItem::Tuple(externals_tuple),
    ])))
}

struct SearchMatcher {
    params: TransactionNotFoundParams,
    requires_internal_in_msg: bool,
    expected_body_hash: Option<HashBytes>,
}

#[allow(clippy::large_enum_variant)]
enum IterationStop {
    Steps(usize),
    UntilMatch(SearchMatcher),
    Exhausted,
}

struct MessageIterBatch {
    results: Vec<SendMessageResult>,
    matched: bool,
    hard_error: Option<anyhow::Error>,
}

fn send_results_to_tuple_items(emulations: &[SendMessageResult]) -> Vec<TupleItem> {
    emulations
        .iter()
        .filter_map(|emulation| match emulation {
            SendMessageResult::Success(res) => Some(res),
            SendMessageResult::Error(_) => None,
        })
        .filter_map(emulation_to_send_result)
        .collect::<Vec<_>>()
}

fn save_send_results(ctx: &mut Context, emulations: &[SendMessageResult]) {
    if emulations.is_empty() {
        return;
    }

    ctx.chain
        .emulations
        .save_message(&ctx.env.running_id, emulations.to_vec());
}

fn save_message_iter_results(ctx: &mut Context, cursor_id: u64, emulations: &[SendMessageResult]) {
    if emulations.is_empty() {
        return;
    }

    let trace_index = match ctx.message_iters.trace_index(cursor_id) {
        Some(trace_index) => trace_index,
        None => {
            save_send_results(ctx, emulations);
            return;
        }
    };

    let saved_trace_index = match trace_index {
        Some(trace_index) => ctx.chain.emulations.append_message_to_trace(
            &ctx.env.running_id,
            trace_index,
            emulations.to_vec(),
        ),
        None => ctx
            .chain
            .emulations
            .save_message(&ctx.env.running_id, emulations.to_vec()),
    };

    let _ = ctx
        .message_iters
        .set_trace_index(cursor_id, saved_trace_index);
}

fn finish_message_iter_results(
    ctx: &mut Context,
    cursor_id: u64,
    emulations: &[SendMessageResult],
) {
    save_message_iter_results(ctx, cursor_id, emulations);
    let _ = ctx.message_iters.close_if_done(cursor_id);
}

fn push_successful_send_results(stack: &mut Tuple, emulations: &[SendMessageResult]) {
    stack.push(TupleItem::big_array_from_items(
        send_results_to_tuple_items(emulations),
    ));
}

fn build_search_matcher(params: &Tuple) -> Option<SearchMatcher> {
    let params = parse_search_params(params)?;
    let requires_internal_in_msg = params.opcode.is_some()
        || params.bounced.is_some()
        || params.bounce.is_some()
        || params.value.is_some()
        || params.from.is_some()
        || params.to.is_some();
    let expected_body_hash = params.body.as_ref().map(|body| body.repr_hash()).copied();

    Some(SearchMatcher {
        params,
        requires_internal_in_msg,
        expected_body_hash,
    })
}

fn transaction_matches_search(tx: &Transaction, matcher: &SearchMatcher) -> bool {
    let params = &matcher.params;

    if let Some(expected_deploy) = params.deploy {
        let is_deploy =
            tx.orig_status == AccountStatus::NotExists && tx.end_status == AccountStatus::Active;
        if expected_deploy != is_deploy {
            return false;
        }
    }

    let in_msg = tx.load_in_msg();
    if let Ok(Some(in_msg)) = &in_msg
        && let MsgInfo::Int(info) = &in_msg.info
    {
        if let Some(expected_opcode) = &params.opcode {
            let mut slice = in_msg.body;
            let Ok(opcode) = slice.load_u32() else {
                return false;
            };
            if *expected_opcode != opcode {
                if params.bounced == Some(true) {
                    let Ok(bounced_opcode) = slice.load_u32() else {
                        return false;
                    };
                    if *expected_opcode != bounced_opcode {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }

        if let Some(expected_bounced) = &params.bounced
            && *expected_bounced != info.bounced
        {
            return false;
        }

        if let Some(expected_bounce) = &params.bounce
            && *expected_bounce != info.bounce
        {
            return false;
        }

        if let Some(expected_value) = &params.value
            && (*expected_value) != info.value.tokens.into()
        {
            return false;
        }

        if let Some(expected_from_addr) = &params.from
            && (*expected_from_addr) != info.src
        {
            return false;
        }

        if let Some(expected_to_addr) = &params.to
            && (*expected_to_addr) != info.dst
        {
            return false;
        }

        if let Some(expected_hash) = matcher.expected_body_hash.as_ref() {
            let body_cell = to_cell(&in_msg.body);
            let actual_hash = body_cell.repr_hash();
            if expected_hash != actual_hash {
                return false;
            }
        }
    } else if matcher.requires_internal_in_msg {
        return false;
    }

    let Ok(TxInfo::Ordinary(info)) = tx.load_info() else {
        return false;
    };

    if let Some(expected_compute_skipped) = params.compute_phase_skipped {
        let is_skipped = matches!(info.compute_phase, ComputePhase::Skipped(_));
        if expected_compute_skipped != is_skipped {
            return false;
        }
    }

    if let Some(expected_aborted) = params.aborted
        && expected_aborted != info.aborted
    {
        return false;
    }

    if let Some(expected_action_exit_code) = params.action_exit_code {
        if let Some(action_phase) = &info.action_phase {
            if action_phase.result_code != expected_action_exit_code {
                return false;
            }
        } else {
            return false;
        }
    }

    if let ComputePhase::Executed(compute) = &info.compute_phase
        && let Some(expected_exit_code) = params.exit_code
        && compute.exit_code != expected_exit_code as i32
    {
        return false;
    }

    if let Some(expected_success) = params.success {
        let action_phase_success = if let Some(action_phase) = &info.action_phase {
            action_phase.success
        } else {
            false
        };

        if let ComputePhase::Executed(compute) = &info.compute_phase
            && (action_phase_success && compute.success) != expected_success
        {
            return false;
        }
    }

    true
}

fn backfill_batch_child_transactions(results: &mut [SendMessageResult]) {
    let mut children_by_parent = HashMap::<u64, Vec<u64>>::new();

    for result in results.iter() {
        let SendMessageResult::Success(result) = result else {
            continue;
        };

        if let Some(parent_lt) = result.parent_transaction {
            children_by_parent
                .entry(parent_lt)
                .or_default()
                .push(result.transaction.lt);
        }
    }

    for result in results.iter_mut() {
        let SendMessageResult::Success(result) = result else {
            continue;
        };

        result.child_transactions = children_by_parent
            .remove(&result.transaction.lt)
            .unwrap_or_default();
    }
}

fn execute_message_iter_batch_with<F>(
    message_iters: &mut MessageIterState,
    cursor_id: u64,
    stop: IterationStop,
    mut execute_step: F,
) -> anyhow::Result<MessageIterBatch>
where
    F: FnMut(PendingMessageStep, HashBytes) -> anyhow::Result<SendMessageResult>,
{
    if !message_iters.contains(cursor_id) {
        return Ok(MessageIterBatch {
            results: Vec::new(),
            matched: false,
            hard_error: None,
        });
    }

    let mut executed = 0usize;
    let mut matched = false;
    let mut results = Vec::new();

    loop {
        match &stop {
            IterationStop::Steps(limit) if executed >= *limit => break,
            IterationStop::Steps(_) | IterationStop::UntilMatch(_) | IterationStop::Exhausted => {}
        }

        let Some((pending, libs_owner)) = message_iters.peek_next(cursor_id) else {
            break;
        };

        let mut result = match execute_step(pending.clone(), libs_owner) {
            Ok(result) => result,
            Err(error) if results.is_empty() => return Err(error),
            Err(error) => {
                backfill_batch_child_transactions(&mut results);
                return Ok(MessageIterBatch {
                    results,
                    matched,
                    hard_error: Some(error),
                });
            }
        };
        let _ = message_iters.advance(cursor_id);

        if let SendMessageResult::Success(step) = &mut result {
            step.parent_transaction = pending.parent_lt;
            let tx_lt = step.transaction.lt;
            let out_messages = step.out_messages.clone();
            let mut externals = Vec::new();

            for out_msg_cell in out_messages {
                let Ok(out_msg) = out_msg_cell.parse::<Message<'_>>() else {
                    continue;
                };

                match out_msg.info {
                    MsgInfo::ExtOut(_) => externals.push(out_msg_cell),
                    MsgInfo::Int(_) => {
                        let _ = message_iters.push_child_message(cursor_id, out_msg_cell, tx_lt);
                    }
                    MsgInfo::ExtIn(_) => {}
                }
            }

            step.externals = externals;

            if let IterationStop::UntilMatch(matcher) = &stop
                && transaction_matches_search(&step.transaction, matcher)
            {
                matched = true;
            }
        }

        executed += 1;
        let should_stop = matched;
        results.push(result);

        if should_stop {
            break;
        }
    }

    backfill_batch_child_transactions(&mut results);
    Ok(MessageIterBatch {
        results,
        matched,
        hard_error: None,
    })
}

fn execute_message_iter_batch(
    ctx: &mut Context,
    cursor_id: u64,
    stop: IterationStop,
) -> anyhow::Result<MessageIterBatch> {
    if ctx.is_broadcasting {
        anyhow::bail!("net.sendIter() is available only in emulation mode")
    }

    if ctx.debug.is_enabled() {
        anyhow::bail!("Step-by-step execution is not supported in debug mode yet")
    }

    let chain = &mut ctx.chain;
    execute_message_iter_batch_with(
        &mut ctx.message_iters,
        cursor_id,
        stop,
        |pending, libs_owner| {
            let libs = chain.build_libs_with_hash_owner(&libs_owner);
            chain
                .emulator
                .send_transaction(chain.world_state, pending.message, &libs, pending.from)
                .context("Cannot execute step-by-step transaction")
        },
    )
}

fn send_wallet_message(
    message: &Cell,
    wallet: Wallet,
    network: &Network,
    api_key: &Option<String>,
    custom_networks: HashMap<String, acton_config::config::CustomNetworkUrls>,
) -> anyhow::Result<()> {
    let expired_at_time = std::time::SystemTime::now() + Duration::from_secs(600);
    let expire_at = expired_at_time.duration_since(UNIX_EPOCH)?.as_secs() as u32;

    let client = TonApiClient::new(network.clone(), custom_networks, api_key.clone())?;

    let (seqno, need_state_init) = wallet.seqno(&client)?;
    let message_ton = TonCell::from_boc(Boc::encode(message))?;
    let external =
        wallet
            .wallet
            .create_ext_in_msg(vec![message_ton], seqno, expire_at, need_state_init)?;

    let boc = &external.to_boc_base64()?;
    let network_name = network.to_string();
    let context = SendBocContext::wallet(&wallet, &network_name, seqno, need_state_init);
    client
        .send_boc(boc)
        .map_err(|error| format_send_boc_error(error, context))?;

    Ok(())
}

fn send_message_debug(
    ctx: &mut Context,
    msg_cell: &Cell,
    libs: &Dict<HashBytes, LibDescr>,
    src_addr: Option<IntAddr>,
) -> anyhow::Result<Vec<SendMessageResult>> {
    let msg: RelaxedMessage = msg_cell
        .parse()
        .context("Failed to load message from cell")?;

    let RelaxedMsgInfo::Int(int_message) = &msg.info else {
        anyhow::bail!("Emulator only supports internal messages for now");
    };

    let dst = match &int_message.dst {
        IntAddr::Std(addr) => addr,
        IntAddr::Var(_) => anyhow::bail!("Var addresses are not supported"),
    };
    let dest_account = ctx.chain.world_state.get_account(dst);
    let code = Emulator::get_code_cell(&msg, &dest_account);

    let step_executor = StepExecutor::new().expect("Failed to create executor");
    let compilation_result = ctx
        .build
        .build_cache
        .result_for_code(&code)
        .map(|(_, result)| result);
    let tolk_source_map = compilation_result
        .as_ref()
        .map(|result| result.tolk_source_map.clone());

    let msg_cell = Emulator::patch_message(
        ctx.chain.world_state.get_config(),
        msg_cell.clone(),
        src_addr,
    )?;
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
        return Ok(vec![]);
    }

    let need_to_stop_on_entry = ctx.debug.need_to_stop_child_thread_on_start();
    let child_debug_started = ctx
        .debug
        .begin_child_context(ChildDebugContextSpec {
            thread_id: 2,
            name: "Send internal message".to_string(),
            executor: AnyExecutor::Message(step_executor.clone()),
            tolk_source_map,
            stop_on_entry: need_to_stop_on_entry,
        })
        .context("Cannot start nested debug context")?;

    let child_step_mode = if need_to_stop_on_entry {
        StepMode::StepIn
    } else if ctx.debug.performing_step() == Some(StepMode::ContinueWithoutBreakpoints) {
        StepMode::ContinueWithoutBreakpoints
    } else {
        StepMode::Continue
    };
    run_nested_executor_until_finished(ctx, child_debug_started, child_step_mode, || {
        step_executor.step()
    })
    .context("Cannot finish nested message execution")?;

    let result = step_executor
        .finish_transaction()
        .context("Cannot finish transaction")?;

    if child_debug_started {
        ctx.debug
            .finish_child_context(2)
            .context("Cannot finish nested debug context")?;

        if !matches!(
            ctx.debug.performing_step(),
            Some(StepMode::Continue | StepMode::ContinueWithoutBreakpoints)
        ) {
            // When we step out from nested message/get method, stop on a line after send/call.
            ctx.debug
                .advance_parent_after_child_return()
                .context("Cannot resume parent after nested debug context")?;
        }
    }

    let result = match result {
        EmulationResult::Success(result) => result,
        EmulationResult::Error(_) => {
            return Ok(vec![]);
        }
    };

    let shard_account_after = &result.shard_account;
    let shard_account_cell = Boc::decode_base64(shard_account_after.as_ref())
        .context("Failed to decode shard account BoC")?;
    let shard_account: ShardAccount = shard_account_cell
        .parse()
        .context("Failed to load shard account from cell")?;

    ctx.chain.world_state.update_account(dst, &shard_account);

    let tx_cell = Boc::decode_base64(result.transaction.as_ref())
        .context("Failed to decode transaction BoC")?;
    let transaction: Transaction = tx_cell
        .parse()
        .context("Failed to load transaction from cell")?;

    let out_messages = transaction
        .iter_out_msgs()
        .filter_map(Result::ok)
        .map(|it| to_cell(&it))
        .collect::<Vec<_>>();

    let send_result = SendMessageResultSuccess {
        raw_transaction: result.transaction,
        transaction: transaction.clone(),
        parent_transaction: None,
        child_transactions: vec![],
        shard_account_before: dest_account,
        shard_account,
        out_messages,
        vm_log: result.vm_log,
        executor_logs: Arc::default(),
        actions: result.actions,
        code,
        externals: vec![],
        missing_libraries: FxHashSet::default(),
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

extension!(send_single_message in (Context) with (src: IntAddr, msg: Cell) using send_single_message_impl);
fn send_single_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: IntAddr,
    msg: Cell,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;

    let msg_cell = msg;
    let libs = ctx.chain.build_libs(&src);
    let world_state = &mut ctx.chain.world_state;

    let emulation = emulator
        .send_transaction(world_state, msg_cell, &libs, Some(src))
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

extension!(start_message_iter in (Context) with (src: IntAddr, msg: Cell) using start_message_iter_impl);
fn start_message_iter_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: IntAddr,
    msg: Cell,
) -> anyhow::Result<()> {
    if ctx.is_broadcasting {
        anyhow::bail!("net.sendIter() is available only in emulation mode")
    }

    if ctx.debug.is_enabled() {
        anyhow::bail!("Step-by-step execution is not supported in debug mode yet")
    }

    let std_address = src.as_std().context("Var addresses are not supported")?;
    let libs_owner = std_address.address;
    let cursor_id = ctx
        .message_iters
        .insert_message_cursor(msg, Some(src), libs_owner);
    stack.push(TupleItem::Int(BigInt::from(cursor_id)));
    Ok(())
}

extension!(execute_message_iter_n in (Context) with (count: BigInt, cursor_id: BigInt) using execute_message_iter_n_impl);
fn execute_message_iter_n_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    count: BigInt,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let count = count.to_usize().unwrap_or(0);
    let batch = execute_message_iter_batch(ctx, cursor_id, IterationStop::Steps(count))?;

    finish_message_iter_results(ctx, cursor_id, &batch.results);
    if let Some(error) = batch.hard_error {
        return Err(error);
    }

    if let [SendMessageResult::Error(error)] = &batch.results[..] {
        anyhow::bail!("Cannot execute transaction iterator step: {}", error.error);
    }

    push_successful_send_results(stack, &batch.results);
    Ok(())
}

extension!(execute_message_iter_till in (Context) with (params: Tuple, cursor_id: BigInt) using execute_message_iter_till_impl);
fn execute_message_iter_till_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let Some(matcher) = build_search_matcher(&params) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let batch = execute_message_iter_batch(ctx, cursor_id, IterationStop::UntilMatch(matcher))?;

    finish_message_iter_results(ctx, cursor_id, &batch.results);
    if let Some(error) = batch.hard_error {
        return Err(error);
    }

    if let [SendMessageResult::Error(error)] = &batch.results[..] {
        anyhow::bail!("Cannot execute transaction iterator step: {}", error.error);
    }

    if !batch.matched {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    push_successful_send_results(stack, &batch.results);
    Ok(())
}

extension!(execute_message_iter_from in (Context) with (cursor_id: BigInt) using execute_message_iter_from_impl);
fn execute_message_iter_from_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let batch = execute_message_iter_batch(ctx, cursor_id, IterationStop::Exhausted)?;

    finish_message_iter_results(ctx, cursor_id, &batch.results);
    if let Some(error) = batch.hard_error {
        return Err(error);
    }

    if let [SendMessageResult::Error(error)] = &batch.results[..] {
        anyhow::bail!("Cannot execute transaction iterator step: {}", error.error);
    }

    push_successful_send_results(stack, &batch.results);
    Ok(())
}

extension!(is_message_iter_done in (Context) with (cursor_id: BigInt) using is_message_iter_done_impl);
fn is_message_iter_done_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    stack.push_bool(ctx.message_iters.is_done(cursor_id));
    Ok(())
}

extension!(close_message_iter in (Context) with (cursor_id: BigInt) using close_message_iter_impl);
fn close_message_iter_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let _ = ctx.message_iters.close(cursor_id);
    Ok(())
}

fn root_lt_from_send_results(txs: &[TupleItem]) -> Option<u64> {
    let first = txs.first()?;
    let TupleItem::Tuple(send_result) = first else {
        return None;
    };
    let Some(TupleItem::Cell(tx_cell)) = send_result.first() else {
        return None;
    };
    let tx = tx_cell.parse::<Transaction>().ok()?;
    Some(tx.lt)
}

extension!(save_trace_name in (Context) with (trace_name: String, txs: Vec<TupleItem>) using save_trace_name_impl);
fn save_trace_name_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    trace_name: String,
    txs: Vec<TupleItem>,
) -> anyhow::Result<()> {
    let Some(root_lt) = root_lt_from_send_results(&txs) else {
        return Ok(());
    };

    ctx.chain
        .emulations
        .save_trace_name(&ctx.env.running_id, root_lt, trace_name);
    Ok(())
}

extension!(find_transaction_by_params in (Context) with (params: Tuple, txs: Vec<TupleItem>) using find_transaction_by_params_impl);
fn find_transaction_by_params_impl(
    _ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    txs: Vec<TupleItem>,
) -> anyhow::Result<()> {
    if txs.is_empty() {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let Some(matcher) = build_search_matcher(&params) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let mut found = txs
        .iter()
        .filter_map(|el| match el {
            TupleItem::Tuple(tuple) => match tuple.first() {
                Some(TupleItem::Cell(cell)) => Some(cell),
                _ => None,
            },
            _ => None,
        })
        .filter_map(|cell| Some((cell.parse::<Transaction>().ok()?, cell)))
        .filter(|(tx, _)| transaction_matches_search(tx, &matcher));

    let Some((_, first)) = found.next() else {
        // No transaction found
        stack.push(TupleItem::Null);
        return Ok(());
    };

    stack.push(TupleItem::Cell(first.clone()));
    Ok(())
}

extension!(run_get_method in (Context) with (args: Tuple, return_type_name: String, name: String, id: BigInt, code: Cell, address: StdAddr) using run_get_method_impl);
#[allow(clippy::too_many_arguments)]
fn run_get_method_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    args: Tuple,
    return_type_name: String,
    name: String,
    id: BigInt,
    code: Cell,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let args = args.unwrap_empty().unwrap_tuple();
    let world_state = &mut ctx.chain.world_state;
    let addr_str = addr.to_string();

    let shard_account = world_state.get_account(&addr);
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

    let libs = ctx.chain.build_libs_with_hash_owner(&addr.address);
    let libs_root = libs.into_root();
    let world_state = &mut ctx.chain.world_state;

    let method_id = id.to_i32().unwrap_or(0);

    let now = std::time::SystemTime::now();
    let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("Time went backwards");

    let params = RunGetMethodArgs {
        code: Boc::encode_base64(&code),
        data: Boc::encode_base64(data),
        verbosity: ctx.env.default_log_level,
        libs: libs_root.map(Boc::encode_base64).unwrap_or_default(),
        address: addr_str,
        unixtime: duration_since_epoch.as_secs().try_into()?,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let compilation_result = ctx
        .build
        .build_cache
        .result_for_code(&Some(code))
        .map(|(_, result)| result);
    let tolk_source_map = compilation_result
        .as_ref()
        .map(|result| result.tolk_source_map.clone())
        .unwrap_or_else(|| Arc::new(TolkSourceMap::without_debug_info()));

    let config_b64 = world_state.get_config_b64();
    let args_b64 = serialize_tuple(&args)
        .map(|t| Boc::encode_base64(&t))
        .context("Cannot serialize tuple")?;

    let result = if ctx.debug.is_enabled() {
        let step_executor = StepGetExecutor::new(&args_b64, &params, Some(&config_b64))
            .context("Cannot create get executor")?;
        step_executor
            .prepare(method_id, &args_b64)
            .context("Cannot prepare get method")?;

        let need_to_stop_on_entry = ctx.debug.need_to_stop_child_thread_on_start();
        let child_debug_started = ctx
            .debug
            .begin_child_context(ChildDebugContextSpec {
                thread_id: 2,
                name: "Run get method".to_string(),
                executor: AnyExecutor::Get(step_executor.clone()),
                tolk_source_map: Some(tolk_source_map.clone()),
                stop_on_entry: need_to_stop_on_entry,
            })
            .context("Cannot send response")?;

        let child_step_mode = if need_to_stop_on_entry {
            StepMode::StepIn
        } else if ctx.debug.performing_step() == Some(StepMode::ContinueWithoutBreakpoints) {
            StepMode::ContinueWithoutBreakpoints
        } else {
            StepMode::Continue
        };
        run_nested_executor_until_finished(ctx, child_debug_started, child_step_mode, || {
            step_executor.step()
        })
        .context("Cannot finish nested get method execution")?;

        let result = step_executor
            .finish(&params.code)
            .context("Cannot run get method")?;

        if child_debug_started {
            ctx.debug
                .finish_child_context(2)
                .context("Cannot send response")?;

            if !matches!(
                ctx.debug.performing_step(),
                Some(StepMode::Continue | StepMode::ContinueWithoutBreakpoints)
            ) {
                // When we step out from nested message/get method, stop on a line after call.
                ctx.debug
                    .advance_parent_after_child_return()
                    .context("Cannot resume parent after nested debug context")?;
            }
        }

        result
    } else {
        let executor = GetExecutor::new(&params).context("Cannot create get executor")?;
        executor
            .run_get_method(&args_b64, &params, Some(&config_b64))
            .context("Cannot run get method")?
    };

    match result {
        GetMethodResult::Success(result) => {
            ctx.chain
                .emulations
                .save_get_method(&ctx.env.running_id, result.clone());

            let cell =
                Boc::decode_base64(result.stack.as_ref()).context("Failed to decode stack BoC")?;
            let tuple = Tuple::deserialize(&cell).context("Failed to deserialize tuple")?;

            if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
                let get_method = ctx.env.abi.find_get_method_by_id(&id);

                let id_presentation = format!("({id})");
                let id_presentation = id_presentation.dimmed();

                let get_method_presentation = if let Some(get_method) = get_method {
                    format!("{} {id_presentation}", get_method.name.yellow())
                } else if name.is_empty() {
                    format!("'' {id_presentation}")
                } else {
                    format!("{} {id_presentation}", name.yellow())
                };

                let suggested_name = if result.vm_exit_code == 11 {
                    // TODO: right now get methods may not include all get methods
                    let get_methods: Vec<&str> = ctx
                        .env
                        .abi
                        .get_methods
                        .iter()
                        .map(|m| m.name.as_str())
                        .collect();
                    suggest_name(&name, &get_methods).map(ToOwned::to_owned)
                } else {
                    None
                };

                let location = retrace::find_exception_info(&result.vm_log, &tolk_source_map)
                    .map(|info| info.loc);

                *ctx.asserts.assert_failure =
                    Some(AssertFailure::GetMethod(GetMethodAssertFailure {
                        get_method_presentation,
                        vm_exit_code: result.vm_exit_code,
                        suggested_name,
                        vm_log: result.vm_log,
                        tolk_source_map,
                        caller_trace: None,
                        location,
                    }));

                stack.push(TupleItem::Null);
                return Ok(());
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

extension!(is_deployed in (Context) with (addr: StdAddr) using is_deployed_impl);
fn is_deployed_impl(ctx: &mut Context, stack: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let is_deployed = ctx.chain.world_state.check_deployed(&addr);
    stack.push_bool(is_deployed);
    Ok(())
}

extension!(get_deployed_code in (Context) with (addr: StdAddr) using get_deployed_code_impl);
fn get_deployed_code_impl(ctx: &mut Context, stk: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let is_deployed = ctx.chain.world_state.check_deployed(&addr);
    if !is_deployed {
        stk.push(TupleItem::Null);
        return Ok(());
    }

    let account = ctx.chain.world_state.get_account(&addr);
    let Some(cell) = get_address_code(&account) else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    stk.push(TupleItem::Cell(cell));
    Ok(())
}

fn get_address_code(account: &ShardAccount) -> Option<Cell> {
    let state = account.account.load().ok()?.0.map(|s| s.state);

    let Some(AccountState::Active(state)) = state else {
        return None;
    };

    state.code
}

extension!(crc16 in (Context) with (data: String) using crc16_impl);
fn crc16_impl(_ctx: &mut Context, stack: &mut Tuple, data: String) -> anyhow::Result<()> {
    const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);
    let result = CRC16.checksum(data.as_bytes());
    stack.push(TupleItem::Int(BigInt::from(result)));
    Ok(())
}

extension!(type_name_by_opcode in (Context) with (id: BigInt) using type_name_by_opcode_impl);
fn type_name_by_opcode_impl(ctx: &mut Context, stk: &mut Tuple, id: BigInt) -> anyhow::Result<()> {
    let id = u32::try_from(&id).context("ID is too big for uint32 opcode")?;
    let Some(type_abi) = ctx.env.abi.find_type_by_opcode(id) else {
        stk.push(TupleItem::Null);
        return Ok(());
    };
    stk.push_string(&type_abi.name);
    Ok(())
}

extension!(register_address in (Context) with (name: String, address: StdAddr) using register_address_impl);
fn register_address_impl(
    ctx: &mut Context,
    _: &mut Tuple,
    name: String,
    address: StdAddr,
) -> anyhow::Result<()> {
    ctx.build
        .known_addresses
        .addresses
        .insert(address, KnownAddress { name });
    Ok(())
}

extension!(register_code in (Context) with (name: String, code: Cell) using register_code_impl);
fn register_code_impl(
    ctx: &mut Context,
    _: &mut Tuple,
    name: String,
    code: Cell,
) -> anyhow::Result<()> {
    let hash = code.repr_hash();
    ctx.build.known_code_cells.insert(*hash, name);
    Ok(())
}

extension!(account_state in (Context) with (addr: StdAddr) using account_state_impl);
fn account_state_impl(ctx: &mut Context, stk: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let account = ctx.chain.world_state.get_account(&addr);
    let optional_account = account.account.load();
    let Ok(account) = optional_account.map_err(|e| anyhow::anyhow!("Failed to load account: {e}"))
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

    stk.push(TupleItem::Cell(cell));
    Ok(())
}

extension!(register_lib in (Context) with (lib: Cell) using register_lib_impl);
fn register_lib_impl(ctx: &mut Context, _stack: &mut Tuple, lib: Cell) -> anyhow::Result<()> {
    ctx.chain.world_state.register_lib(lib);
    Ok(())
}

extension!(convert_address in (Context) with (address: String) using convert_address_impl);
fn convert_address_impl(_: &mut Context, stack: &mut Tuple, address: String) -> anyhow::Result<()> {
    let (addr, _) = StdAddr::from_str_ext(&address, StdAddrFormat::any())?;
    stack.push(TupleItem::Cell(to_cell(&addr)));
    Ok(())
}

extension!(cell_from_hex in (Context) with (cell_hex: String) using cell_from_hex_impl);
fn cell_from_hex_impl(_: &mut Context, stack: &mut Tuple, cell_hex: String) -> anyhow::Result<()> {
    let cell = Boc::decode_hex(&cell_hex)
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
    let Ok(hash) = HashBytes::from_str(&hash) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let network = ctx.network();
    let custom_networks = ctx.env.config.custom_networks();

    if let Network::Custom(network_name) = &network
        && !custom_networks.contains_key(network_name.as_ref())
    {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let api_key = ctx.env.api_key.clone();
    let Ok(api_client) = TonApiClient::new(network, custom_networks, api_key) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    match api_client.get_library_by_hash(&hash) {
        Ok(cell) => {
            stack.push(TupleItem::Cell(cell));
        }
        Err(_) => {
            stack.push(TupleItem::Null);
        }
    }

    Ok(())
}

extension!(is_broadcasting in (Context) using is_broadcasting_impl);
fn is_broadcasting_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    stack.push_bool(ctx.is_broadcasting);
    Ok(())
}

extension!(enable_broadcast in (Context) using enable_broadcast_impl);
const fn enable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) -> anyhow::Result<()> {
    ctx.is_broadcasting = true;
    Ok(())
}

extension!(disable_broadcast in (Context) using disable_broadcast_impl);
const fn disable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) -> anyhow::Result<()> {
    ctx.is_broadcasting = false;
    Ok(())
}

extension!(get_wallet_by_name in (Context) with (name: String) using get_wallet_by_name_impl);
fn get_wallet_by_name_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    name: String,
) -> anyhow::Result<()> {
    let Some(wallet) = ctx.env.open_wallets.get(&name) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let addr = wallet.address();
    stack.push(TupleItem::Cell(to_cell(&addr)));

    Ok(())
}

const WAIT_FOR_TRANSACTION_DEFAULT_SLEEP_MS: u64 = 1000;
const WAIT_FOR_TRANSACTION_SETTLE_DELAY_MS: u64 = 1000;

extension!(wait_for_transaction in (Context) with (sleep_duration: BigInt, attempts: BigInt, quiet: bool, ext_message_hash: HashBytes, address: StdAddr) using wait_for_transaction_impl);
#[allow(clippy::too_many_arguments)]
fn wait_for_transaction_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    sleep_duration: BigInt,
    attempts: BigInt,
    quiet: bool,
    ext_message_hash: HashBytes,
    address: StdAddr,
) -> anyhow::Result<()> {
    if !ctx.is_broadcasting {
        // In emulation mode waitForTransaction is a no-op.
        stack.push_bool(true);
        return Ok(());
    }

    let attempts = attempts.to_u32().unwrap_or(20);
    let sleep_duration_ms = sleep_duration
        .to_u64()
        .unwrap_or(WAIT_FOR_TRANSACTION_DEFAULT_SLEEP_MS);

    if attempts == 0 {
        anyhow::bail!("Attempt number must be positive");
    }

    let address_str = address.to_string();

    let network = ctx.network();

    let custom_networks = ctx.env.config.custom_networks();
    let api_key = ctx.env.api_key.clone();
    let api_client = if let Ok(client) = TonApiClient::new(network, custom_networks, api_key) {
        client
    } else {
        stack.push_bool(false);
        return Ok(());
    };

    let ext_message_hash_bytes = ext_message_hash.as_slice();

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
                    // Keep a short settle delay so the next related tx is more likely to be visible.
                    std::thread::sleep(Duration::from_millis(WAIT_FOR_TRANSACTION_SETTLE_DELAY_MS));

                    if !quiet {
                        let hex = base64::engine::general_purpose::STANDARD
                            .decode(tx.transaction_id.hash.clone())
                            .map_or_else(|_| tx.transaction_id.hash.clone(), hex::encode);
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

    stack.push_bool(false);
    Ok(())
}

fn get_transaction_link(
    ctx: &mut Context,
    address_str: String,
    tx: TonCenterTransaction,
    hex: String,
) -> String {
    let network = ctx.network();
    match &network {
        Network::Localnet => {
            if let Some(url) = localnet_transaction_link(ctx, &hex) {
                return url;
            }
        }
        Network::Custom(network_name) => {
            if let Some(url) = custom_network_transaction_link(ctx, network_name.as_ref(), &hex) {
                return url;
            }
        }
        Network::Mainnet | Network::Testnet => {}
    }

    let network_prefix = if network.uses_testnet_address_format() {
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

fn custom_network_transaction_link(
    ctx: &Context,
    network_name: &str,
    tx_hash_hex: &str,
) -> Option<String> {
    let custom_networks = ctx.env.config.custom_networks();
    let network_urls = custom_networks.get(network_name)?;
    configured_network_transaction_link(network_urls, tx_hash_hex)
}

fn localnet_transaction_link(ctx: &Context, tx_hash_hex: &str) -> Option<String> {
    let custom_networks = ctx.env.config.custom_networks();
    let localnet_urls = custom_networks.get("localnet")?;
    configured_network_transaction_link(localnet_urls, tx_hash_hex)
}

fn configured_network_transaction_link(
    network_urls: &acton_config::config::CustomNetworkUrls,
    tx_hash_hex: &str,
) -> Option<String> {
    if let Some(explorer_url) = network_urls.explorer_url.as_deref() {
        return explorer_transaction_link(explorer_url, tx_hash_hex);
    }

    let mut explorer_base = reqwest::Url::parse(network_urls.v2_url.as_ref()).ok()?;
    explorer_base.set_path("/explorer");
    explorer_base.set_query(None);
    explorer_base.set_fragment(None);

    explorer_transaction_link(explorer_base.as_str(), tx_hash_hex)
}

fn explorer_transaction_link(explorer_base: &str, tx_hash_hex: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(explorer_base).ok()?;
    let base_path = url.path().trim_end_matches('/');
    let tx_base = if base_path.is_empty() {
        "/tx".to_string()
    } else if base_path.ends_with("/tx") {
        base_path.to_string()
    } else {
        format!("{base_path}/tx")
    };
    let path = format!("{tx_base}/{tx_hash_hex}");
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

extension!(get_config in (Context) using get_config_impl);
fn get_config_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    let config = ctx.chain.world_state.get_config_cell();
    stack.push(TupleItem::Cell(config));
    Ok(())
}

extension!(set_config in (Context) with (config: Cell) using set_config_impl);
fn set_config_impl(ctx: &mut Context, stack: &mut Tuple, config: Cell) -> anyhow::Result<()> {
    let result = ctx.chain.emulator.set_config(ctx.chain.world_state, config);

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

extension!(get_shard_account in (Context) with (addr: StdAddr) using get_shard_account_impl);
fn get_shard_account_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let shard_account = ctx.chain.world_state.get_account(&addr);
    let shard_account_cell = to_cell(&shard_account);
    stack.push(TupleItem::Cell(shard_account_cell));
    Ok(())
}

extension!(set_shard_account in (Context) with (shard_account: Option<ShardAccount>, addr: StdAddr) using set_shard_account_impl);
fn set_shard_account_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    shard_account: Option<ShardAccount>,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let shard_account = match shard_account {
        Some(shard_account) => shard_account,
        None => ShardAccount {
            account: Lazy::new(&OptionalAccount(None))
                .context("Failed to create empty shard account")?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        },
    };

    ctx.chain.world_state.update_account(&addr, &shard_account);
    Ok(())
}

extension!(save_world_state_snapshot in (Context) with (path: String) using save_world_state_snapshot_impl);
fn save_world_state_snapshot_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    path: String,
) -> anyhow::Result<()> {
    let success = ctx
        .chain
        .world_state
        .snapshot()
        .and_then(|snapshot| serde_json::to_string_pretty(&snapshot).map_err(Into::into))
        .is_ok_and(|json| fs::write(&path, json).is_ok());
    stack.push_bool(success);
    Ok(())
}

extension!(load_world_state_snapshot in (Context) with (path: String) using load_world_state_snapshot_impl);
fn load_world_state_snapshot_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    path: String,
) -> anyhow::Result<()> {
    let success = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .and_then(|snapshot| ctx.chain.world_state.load_snapshot(snapshot).ok())
        .is_some();
    stack.push_bool(success);
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        6 => build : 2,
        8 => run_get_method : 6,
        9 => send_message : 2,
        10 => find_transaction_by_params : 2,
        11 => is_deployed : 1,
        12 => get_deployed_code : 1,
        13 => crc16 : 1,
        14 => type_name_by_opcode : 1,
        15 => register_address : 2,
        16 => register_code : 2,
        17 => account_state : 1,
        18 => register_lib : 1,
        19 => convert_address : 1,
        20 => cell_from_hex : 1,
        21 => load_library_by_hash : 1,
        23 => is_broadcasting : 0,
        24 => get_wallet_by_name : 1,
        25 => wait_for_transaction : 5,
        26 => enable_broadcast : 0,
        27 => disable_broadcast : 0,
        28 => set_now : 1,
        29 => get_now : 0,
        30 => send_single_message : 2,
        31 => get_config : 0,
        32 => set_config : 1,
        33 => get_shard_account : 1,
        34 => set_shard_account : 2,
        35 => save_trace_name : 2,
        36 => run_tick_tock : 2,
        37 => save_world_state_snapshot : 1,
        38 => load_world_state_snapshot : 1,
        39 => start_message_iter : 2,
        40 => execute_message_iter_n : 2,
        41 => execute_message_iter_till : 2,
        42 => execute_message_iter_from : 1,
        43 => is_message_iter_done : 1,
        44 => close_message_iter : 1,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::sync::Arc;

    fn test_hash(byte: u8) -> HashBytes {
        HashBytes([byte; 32])
    }

    fn test_transaction(lt: u64) -> Transaction {
        Transaction {
            account: Default::default(),
            lt,
            prev_trans_hash: Default::default(),
            prev_trans_lt: 0,
            now: 0,
            out_msg_count: Default::default(),
            orig_status: AccountStatus::Uninit,
            end_status: AccountStatus::Uninit,
            in_msg: None,
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
        }
    }

    fn test_success(lt: u64) -> SendMessageResult {
        let empty_shard_account = ShardAccount {
            account: Lazy::new(&OptionalAccount(None)).expect("empty optional account"),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        };
        SendMessageResult::Success(SendMessageResultSuccess {
            raw_transaction: Arc::from(format!("tx-{lt}")),
            transaction: test_transaction(lt),
            parent_transaction: None,
            child_transactions: vec![],
            shard_account_before: empty_shard_account.clone(),
            shard_account: empty_shard_account,
            out_messages: vec![],
            vm_log: Arc::default(),
            executor_logs: Arc::default(),
            actions: None,
            code: None,
            externals: vec![],
            missing_libraries: FxHashSet::default(),
        })
    }

    fn test_error(message: &str) -> SendMessageResult {
        SendMessageResult::Error(ton_executor::message::RunTransactionResultError {
            error: message.to_string(),
            vm_log: None,
            vm_exit_code: None,
            executor_logs: None,
            missing_libraries: FxHashSet::default(),
        })
    }

    #[test]
    fn configured_network_transaction_link_prefers_explorer_url() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: Some(Arc::from("https://explorer.example/explorer")),
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("explorer link should be built");
        assert_eq!(url, "https://explorer.example/explorer/tx/abc123");
    }

    #[test]
    fn configured_network_transaction_link_keeps_existing_tx_suffix() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: Some(Arc::from("https://explorer.example/explorer/tx/")),
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("explorer link should be built");
        assert_eq!(url, "https://explorer.example/explorer/tx/abc123");
    }

    #[test]
    fn configured_network_transaction_link_appends_tx_for_host_only_explorer() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: Some(Arc::from("http://localhost:3006")),
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("explorer link should be built");
        assert_eq!(url, "http://localhost:3006/tx/abc123");
    }

    #[test]
    fn configured_network_transaction_link_falls_back_to_v2() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: None,
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("fallback link should be built");
        assert_eq!(url, "http://localhost:3010/explorer/tx/abc123");
    }

    #[test]
    fn step_batch_keeps_later_siblings_after_error_result() {
        let mut message_iters = MessageIterState::new();
        let cursor_id = message_iters.insert_message_cursor(Cell::default(), None, test_hash(1));
        message_iters
            .advance(cursor_id)
            .expect("root step must be consumed");
        message_iters
            .push_child_message(cursor_id, Cell::default(), 100)
            .expect("first child must be queued");
        message_iters
            .push_child_message(cursor_id, Cell::default(), 100)
            .expect("second child must be queued");

        let mut calls = 0usize;
        let batch = execute_message_iter_batch_with(
            &mut message_iters,
            cursor_id,
            IterationStop::Exhausted,
            |_pending, _| {
                calls += 1;
                Ok(if calls == 1 {
                    test_error("first child error")
                } else {
                    test_success(202)
                })
            },
        )
        .expect("batch execution should succeed");

        assert!(batch.hard_error.is_none());
        assert_eq!(batch.results.len(), 2);
        assert!(matches!(batch.results[0], SendMessageResult::Error(_)));
        assert!(matches!(batch.results[1], SendMessageResult::Success(_)));
        assert!(message_iters.is_done(cursor_id));
    }

    #[test]
    fn step_batch_preserves_pending_step_on_hard_error_after_partial_progress() {
        let mut message_iters = MessageIterState::new();
        let cursor_id = message_iters.insert_message_cursor(Cell::default(), None, test_hash(2));
        message_iters
            .advance(cursor_id)
            .expect("root step must be consumed");
        let second_child = Cell::default();
        message_iters
            .push_child_message(cursor_id, Cell::default(), 200)
            .expect("first child must be queued");
        message_iters
            .push_child_message(cursor_id, second_child.clone(), 200)
            .expect("second child must be queued");

        let mut calls = 0usize;
        let batch = execute_message_iter_batch_with(
            &mut message_iters,
            cursor_id,
            IterationStop::Exhausted,
            |_pending, _| {
                calls += 1;
                if calls == 1 {
                    Ok(test_success(303))
                } else {
                    Err(anyhow!("hard step failure"))
                }
            },
        )
        .expect("partial progress should be returned instead of dropped");

        assert_eq!(batch.results.len(), 1);
        assert!(matches!(batch.results[0], SendMessageResult::Success(_)));
        assert_eq!(
            batch
                .hard_error
                .expect("hard error must be preserved")
                .to_string(),
            "hard step failure"
        );

        let (pending, owner) = message_iters
            .peek_next(cursor_id)
            .expect("failed step must stay pending for retry");
        assert_eq!(owner, test_hash(2));
        assert_eq!(pending.parent_lt, Some(200));
        assert_eq!(pending.message, second_child);
        assert!(!message_iters.is_done(cursor_id));
    }
}
