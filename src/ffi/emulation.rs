use crate::context::{AssertFailure, Context, FailAssertFailure, KnownAddress};
use crate::debugger::debug_context::StepMode;
use crate::ffi::assert::process_txs_and_search_params;
use crc::{CRC_16_XMODEM, Crc};
use emulator::config::DEFAULT_CONFIG;
use emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use emulator::executor::{
    EmulationResult, Executor, ExecutorVerbosity, RunTransactionArgs, StoreExt,
};
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_executor::StepExecutor;
use emulator::step_get_executor::StepGetExecutor;
use emulator::traits::BaseExecutor;
use emulator::{AnyExecutor, extension, pop_args, register_ext_methods, try_ctx};
use log::{debug, info, warn};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::MsgAddrIntStd;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Load, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, IntAddr, LibDescr, MsgInfo, RelaxedMessage,
    RelaxedMsgInfo, ShardAccount, Transaction, TxInfo,
};

extension!(build in (Context) with (path: String, name: String) using build_impl);
fn build_impl(ctx: &mut Context, stack: &mut Tuple, mut path: String, name: String) {
    debug!("Building {name}");
    let start_time = Instant::now();

    if path.is_empty() {
        debug!("No path provided, search in contracts");
        let found_contract = ctx.env.find_contract(name.as_str());

        if let Some(found_contract) = found_contract {
            debug!("Found contract with info: {found_contract:?}");
            path = found_contract.src.clone()
        } else {
            ctx.asserts.fail(format!(
                "Cannot find contract {name} in Acton.toml, please add it or provide an explicit path to the entry point"
            ));
            return;
        }
    }

    if let Some(cached) = ctx.build.build_cache.built.get(&path) {
        let elapsed = start_time.elapsed();
        info!("Build {path} from memory cache in {elapsed:?}");

        let code_cell = try_ctx!(
            ctx,
            ArcCell::from_boc_b64(&cached.code_boc64),
            "Failed to decode cached code BoC for {}: {}",
            path
        );
        stack.push(TupleItem::Cell(code_cell));
        return;
    }

    if let Some(cached_entry) =
        ctx.build
            .file_build_cache
            .get(&path, ctx.build.need_debug_info, 2, "1.2".to_string())
    {
        let elapsed = start_time.elapsed();
        info!("Build {path} from file cache (.acton/cache) in {elapsed:?}");

        ctx.build.build_cache.memoize(
            &name,
            &path,
            &cached_entry.code_boc64,
            &cached_entry.code_hash_hex,
            cached_entry.source_map.clone().unwrap_or_default(),
        );

        let code_cell = try_ctx!(
            ctx,
            ArcCell::from_boc_b64(&cached_entry.code_boc64),
            "Failed to decode cached code BoC for {}: {}",
            path
        );
        stack.push(TupleItem::Cell(code_cell));
        return;
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

            ctx.build.build_cache.memoize(
                &name,
                &path,
                &success.code_boc64,
                &success.code_hash_hex,
                success.source_map.unwrap_or(Default::default()),
            );
            let code_cell = try_ctx!(
                ctx,
                ArcCell::from_boc_b64(&success.code_boc64),
                "Failed to decode compiled code BoC for {}: {}",
                path
            );
            stack.push(TupleItem::Cell(code_cell))
        }
        tolkc::CompilerResult::Error(error) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Build {} failed after {:?}: {}",
                path, total_elapsed, error.message
            );

            *ctx.asserts.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
                message: Some(format!("Compilation failed: {}", error.message)),
                location: None,
            }));
            stack.push(TupleItem::Null);
        }
    };
}

extension!(send_message in (Context) with (mode: BigInt, message: ArcCell) using send_message_impl);
fn send_message_impl(ctx: &mut Context, stack: &mut Tuple, _mode: BigInt, message: ArcCell) {
    let blockchain = &mut ctx.chain.blockchain;
    let emulator = &ctx.chain.emulator;

    let msg_b64 = try_ctx!(
        ctx,
        message.to_boc_b64(false),
        "Failed to encode message to BoC: {}"
    );
    let msg_cell = try_ctx!(
        ctx,
        Boc::decode_base64(msg_b64),
        "Failed to decode message from BoC: {}"
    );

    // Send from null address for now
    let src_addr = IntAddr::default();
    let emulations = emulator.send_message(
        blockchain,
        msg_cell,
        &Dict::default(),
        Some(src_addr),
        Some(ctx.env.default_log_level),
    );

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some(res),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(|emulation| ArcCell::from_boc_b64(&emulation.raw_transaction).ok())
        .map(TupleItem::Cell)
        .collect::<Vec<_>>();
    stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
}

extension!(send_message_from in (Context) with (mode: BigInt, from: ArcCell, message: ArcCell) using send_message_from_impl);
fn send_message_from_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    _mode: BigInt,
    from: ArcCell,
    message: ArcCell,
) {
    let emulator = &ctx.chain.emulator;

    let msg_b64 = try_ctx!(
        ctx,
        message.to_boc_b64(false),
        "Failed to encode message to BoC: {}"
    );
    let msg_cell = try_ctx!(
        ctx,
        Boc::decode_base64(msg_b64),
        "Failed to decode message from BoC: {}"
    );

    let from_b64 = try_ctx!(
        ctx,
        from.to_boc_b64(false),
        "Failed to encode from address to BoC: {}"
    );
    let from_cell = try_ctx!(
        ctx,
        Boc::decode_base64(from_b64),
        "Failed to decode from address from BoC: {}"
    );
    let mut from_slice = try_ctx!(
        ctx,
        from_cell.as_slice(),
        "Failed to create slice `from` from address cell: {}"
    );
    let src_addr = IntAddr::load_from(&mut from_slice);
    let src_addr = match src_addr {
        Ok(src_addr) => src_addr,
        Err(err) => {
            ctx.asserts.fail(format!(
                "Failed to decode src address from x{{{}}} with length={}: {}",
                from_slice.display_data(),
                from_slice.size_bits(),
                err
            ));
            return;
        }
    };

    let libs = ctx.chain.build_libs(&src_addr);
    let blockchain = &mut ctx.chain.blockchain;

    let emulations = if ctx.debug.is_enabled() {
        send_message_debug(
            ctx,
            &msg_cell,
            &libs,
            Some(src_addr),
            Some(ctx.env.default_log_level),
        )
    } else {
        emulator.send_message(
            blockchain,
            msg_cell,
            &libs,
            Some(src_addr),
            Some(ctx.env.default_log_level),
        )
    };

    ctx.chain.emulations.results.push(emulations.clone());

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some((*res).clone()),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(|emulation| {
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
                Some(parent_tx) => TupleItem::Int(BigInt::from(parent_tx.lt)),
                None => TupleItem::Null,
            };
            let actions = match &emulation.actions {
                Some(actions_b64) => {
                    ArcCell::from_boc_b64(actions_b64).unwrap_or_else(|_| ArcCell::default())
                }
                None => ArcCell::default(),
            };

            let result = tx.to_boc_b64(false).ok()?;
            let tx_cell: Cell = Boc::decode_base64(&result).ok()?;
            let mut tx_slice = tx_cell.as_slice().ok()?;
            let parsed_tx = Transaction::load_from(&mut tx_slice).ok()?;

            let out_messages = Tuple(
                parsed_tx
                    .iter_out_msgs()
                    .filter_map(|msg| msg.ok())
                    .filter_map(|msg| {
                        let cell = msg.to_cell();
                        let boc = Boc::encode_base64(&cell);
                        ArcCell::from_boc_b64(&boc).ok()
                    })
                    .map(TupleItem::Cell)
                    .collect::<Vec<_>>(),
            );

            let gas_used = match parsed_tx.load_info() {
                Ok(TxInfo::Ordinary(info)) => match info.compute_phase {
                    ComputePhase::Executed(compute) => compute.gas_used.into(),
                    _ => BigInt::from(0),
                },
                _ => BigInt::from(0),
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
        })
        .collect::<Vec<_>>();
    stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
}

fn send_message_debug(
    ctx: &mut Context,
    msg_cell: &Cell,
    libs: &Dict<HashBytes, LibDescr>,
    src_addr: Option<IntAddr>,
    verbosity: Option<ExecutorVerbosity>,
) -> Vec<SendMessageResult> {
    let mut msg_slice = try_ctx!(
        ctx,
        msg_cell.as_slice(),
        "Failed to create slice from message cell: {}"
    );
    let message_obj = try_ctx!(
        ctx,
        RelaxedMessage::load_from(&mut msg_slice),
        "Failed to load message from slice: {}"
    );

    let RelaxedMsgInfo::Int(int_message) = &message_obj.info else {
        ctx.asserts
            .fail("Emulator only supports internal messages for now".to_string());
        return vec![];
    };

    let dest_account = ctx
        .chain
        .blockchain
        .get_account(&int_message.dst.to_string());
    let code = Executor::get_code_cell(&message_obj, &dest_account);

    let step_executor = StepExecutor::new();
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

    let msg_cell = Emulator::patch_src_addr(msg_cell.clone(), src_addr.clone());
    let prepare_result = step_executor.prepare_transaction(
        msg_cell.clone(),
        BigInt::from(0),
        RunTransactionArgs {
            config: DEFAULT_CONFIG.to_string(),
            libs: libs.clone().into_root(),
            verbosity: verbosity.unwrap_or(ExecutorVerbosity::FullLocation),
            shard_account: dest_account.clone(),
            now: 0,
            lt: ctx.chain.blockchain.get_lt(),
            random_seed: None,
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
        },
    );
    if !prepare_result.success {
        panic!("Failed to prepare Emulator in debug mode");
    }
    if prepare_result.skipped {
        // Since compute phase is skipped, we don't need to run anything
        ctx.debug
            .ctx()
            .finish_thread(2)
            .expect("Cannot send response");
        return vec![];
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
            .expect("Cannot send response");
    }

    let result = step_executor.finish_transaction();

    ctx.debug
        .ctx()
        .finish_thread(2)
        .expect("Cannot send response");

    if ctx.debug.ctx().performing_step != Some(StepMode::Continue) {
        // When we step out from nested message/get method, send stop message to client to
        // stop on a line after send/call get method
        ctx.debug.ctx().step(StepMode::StepIn);
    }

    let result = match result {
        EmulationResult::Success(result) => result,
        EmulationResult::Error(_) => {
            return vec![];
        }
    };

    let shard_account_after = &result.shard_account;
    let shard_account_cell = try_ctx!(
        ctx,
        Boc::decode_base64(shard_account_after),
        "Failed to decode shard account BoC: {}"
    );
    let mut shard_account_slice = try_ctx!(
        ctx,
        shard_account_cell.as_slice(),
        "Failed to create slice from shard account cell: {}"
    );
    let shard_account = try_ctx!(
        ctx,
        ShardAccount::load_from(&mut shard_account_slice),
        "Failed to load shard account from slice: {}"
    );

    ctx.chain
        .blockchain
        .update_account(&int_message.dst.to_string(), &shard_account);

    let tx_cell: Cell = try_ctx!(
        ctx,
        Boc::decode_base64(&result.transaction),
        "Failed to decode transaction BoC: {}"
    );
    let mut tx_slice = try_ctx!(
        ctx,
        tx_cell.as_slice(),
        "Failed to create slice from transaction cell: {}"
    );
    let transaction = try_ctx!(
        ctx,
        Transaction::load_from(&mut tx_slice),
        "Failed to load transaction from slice: {}"
    );

    let out_messages = transaction
        .iter_out_msgs()
        .filter_map(|it| it.ok())
        .map(|it| it.to_cell())
        .collect::<Vec<_>>();

    let code = Executor::get_code_cell(&message_obj, &dest_account);

    let send_result = SendMessageResultSuccess {
        raw_transaction: result.transaction,
        transaction: transaction.clone(),
        parent_transaction: None,
        child_transactions: vec![],
        shard_account,
        out_messages,
        vm_log: result.vm_log,
        logs: "".to_string(),
        debug_logs: "".to_string(),
        actions: result.actions,
        code,
        externals: vec![],
    };

    let mut externals: Vec<Cell> = vec![];

    let mut all_results = std::iter::once(SendMessageResult::Success(send_result.clone()))
        .chain(transaction.iter_out_msgs().flat_map(|msg| {
            let Ok(msg) = msg else { return vec![] };

            if let MsgInfo::ExtOut(_) = &msg.info {
                externals.push(msg.to_cell());
                return vec![];
            };

            let mut send_results = send_message_debug(ctx, &msg.to_cell(), libs, None, verbosity);
            for result in &mut send_results {
                match result {
                    SendMessageResult::Success(result) => {
                        result.parent_transaction = Some(transaction.clone());
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

    all_results
}

extension!(find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple) using find_transaction_by_params_impl);
fn find_transaction_by_params_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
) {
    if txs.0.is_empty() {
        stack.push(TupleItem::Null);
        return;
    }

    let (params, parsed_txs) = match process_txs_and_search_params(&txs, params) {
        Some(value) => value,
        None => {
            stack.push(TupleItem::Null);
            return;
        }
    };

    let found = parsed_txs.iter().filter(|tx| {
        if let Some(expected_deploy) = params.deploy {
            if expected_deploy
                && (tx.orig_status != AccountStatus::NotExists
                    || tx.end_status != AccountStatus::Active)
            {
                // We expect to deploy contract but we don't
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

            if let Some(expected_bounced) = &params.bounced {
                if *expected_bounced != info.bounced {
                    // Bounced value mismatch
                    return false;
                }
            }

            if let Some(expected_from_addr) = &params.from {
                if (*expected_from_addr).to_string() != info.src.to_string() {
                    // Source address mismatch
                    return false;
                }
            }

            if let Some(expected_to_addr) = &params.to {
                if (*expected_to_addr).to_string() != info.dst.to_string() {
                    // Destination address mismatch
                    return false;
                }
            }
        };

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

        if let ComputePhase::Executed(compute) = info.compute_phase {
            if let Some(expected_exit_code) = params.exit_code {
                if compute.exit_code != expected_exit_code as i32 {
                    // Exit code mismatch
                    return false;
                }
            }
        }

        true
    });

    let txs = found.collect::<Vec<_>>();
    let Some(first) = txs.first() else {
        // No transaction found
        stack.push(TupleItem::Null);
        return;
    };

    let tx_base64 = Boc::encode_base64(first.to_cell());
    let tx_cell = try_ctx!(
        ctx,
        ArcCell::from_boc_b64(&tx_base64),
        "Failed to decode transaction BoC: {}"
    );

    stack.push(TupleItem::Cell(tx_cell));
}

extension!(run_get_method in (Context) with (args: Tuple, return_type_name: String, id: BigInt, code: ArcCell, address: ArcCell) using run_get_method_impl);
fn run_get_method_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    args: Tuple,
    return_type_name: String,
    id: BigInt,
    code: ArcCell,
    address: ArcCell,
) {
    let args = args.unwrap_empty().unwrap_tuple();
    let blockchain = &mut ctx.chain.blockchain;
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let address_hash = address_std.address.clone();
    let dst_addr_str = format!("{}:{}", &address_std.workchain, hex::encode(&address_hash));

    let dest_address = TonAddress::from_msg_address(address_std).unwrap();

    let shard_account = blockchain.get_account(&dst_addr_str);
    let state = shard_account.account.load().unwrap().0.map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or(Cell::default())
    } else {
        Cell::default()
    };

    let libs = ctx
        .chain
        .build_libs_with_hash_owner(&HashBytes::from_slice(address_hash.clone().as_slice()));
    let libs_root = libs.clone().into_root();

    let method_id = id.to_i32().unwrap_or(0);
    let params = GetMethodParams {
        code: code.to_boc_b64(false).unwrap(),
        data: Boc::encode_base64(data),
        verbosity: ExecutorVerbosity::FullLocationStackVerbose,
        libs: libs_root.map(Boc::encode_base64).unwrap_or("".to_string()),
        address: dest_address.to_string(),
        unixtime: 0,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let result = if ctx.debug.is_enabled() {
        let step_get_executor = StepGetExecutor::new(Default::default(), params.clone());

        let source_map = ctx
            .build
            .build_cache
            .result_for_code(&Some(
                Boc::decode_base64(code.to_boc_b64(false).unwrap()).unwrap(),
            ))
            .map(|res| res.1.source_map);

        let dbg_ctx = ctx.debug.ctx();
        dbg_ctx
            .begin_thread(
                2,
                AnyExecutor::Get(step_get_executor.clone()),
                source_map,
                "Send internal message".to_string(),
                dbg_ctx.need_to_stop_child_thread_on_start(),
            )
            .expect("Cannot send response");

        step_get_executor.prepare_get_method(method_id, Default::default());

        // Step to update internal state
        if dbg_ctx.need_to_stop_child_thread_on_start() {
            dbg_ctx.step(StepMode::StepIn);
        } else {
            dbg_ctx.step(StepMode::Continue);
        }

        if !dbg_ctx.stepper.is_terminated() {
            dbg_ctx
                .process_incoming_requests(false)
                .expect("Cannot send response");
        }

        dbg_ctx.finish_thread(2).expect("Cannot send response");

        if dbg_ctx.performing_step != Some(StepMode::Continue) {
            // When we step out from nested message/get method, send stop message to client to
            // stop on a line after send/call get method
            dbg_ctx.step(StepMode::StepIn);
        }

        step_get_executor.finish_get_method(&params.code)
    } else {
        let executor = GetExecutor::new(params.clone());
        executor.run_get_method(args, params)
    };

    match result {
        GetMethodResult::Success(result) => {
            ctx.chain.emulations.get_results.push(result.clone());

            let cell = try_ctx!(
                ctx,
                ArcCell::from_boc_b64(&result.stack),
                "Failed to decode stack BoC: {}"
            );
            let tuple = try_ctx!(
                ctx,
                Tuple::deserialize(&cell),
                "Failed to deserialize tuple: {}"
            );

            stack.push(TupleItem::TypedTuple {
                type_name: return_type_name,
                inner: tuple,
            })
        }
        GetMethodResult::Error(result) => {
            println!("Error: {}", result.error);
        }
    };
}

extension!(is_deployed in (Context) with (address: ArcCell) using is_deployed_impl);
fn is_deployed_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let dst_addr_str = try_ctx!(
        ctx,
        cell_address_to_raw(address),
        "Failed to decode address: {}"
    );

    let is_deployed = ctx.chain.blockchain.is_deployed(&dst_addr_str);
    stack.push_bool(is_deployed);
}

extension!(get_deployed_code in (Context) with (address: ArcCell) using get_deployed_code_impl);
fn get_deployed_code_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let dst_addr_str = try_ctx!(
        ctx,
        cell_address_to_raw(address),
        "Failed to decode address: {}"
    );

    let is_deployed = ctx.chain.blockchain.is_deployed(&dst_addr_str);
    if !is_deployed {
        stack.push(TupleItem::Null);
        return;
    }

    let account = ctx.chain.blockchain.get_account(&dst_addr_str);
    let cell = match get_address_code(&account) {
        Some(value) => value,
        None => {
            stack.push(TupleItem::Null);
            return;
        }
    };

    stack.push(TupleItem::Cell(cell));
}

fn cell_address_to_raw(address: ArcCell) -> anyhow::Result<String> {
    let address_boc = address.to_boc_hex(false)?;
    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str())?;
    let dst_addr_str = format!(
        "{}:{}",
        &address_std.workchain,
        hex::encode(&address_std.address)
    );
    Ok(dst_addr_str)
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
fn crc16_impl(_ctx: &mut Context, stack: &mut Tuple, data: String) {
    let crc = Crc::<u16>::new(&CRC_16_XMODEM);
    let result = crc.checksum(data.as_bytes());
    stack.push(TupleItem::Int(BigInt::from(result)));
}

extension!(type_name_by_opcode in (Context) with (id: BigInt) using type_name_by_opcode_impl);
fn type_name_by_opcode_impl(ctx: &mut Context, stack: &mut Tuple, id: BigInt) {
    let type_abi = ctx.env.abi.find_type_by_opcode(id);
    match type_abi {
        None => {
            stack.push(TupleItem::Null);
        }
        Some(type_abi) => {
            stack.push_string(&type_abi.name);
        }
    }
}

extension!(register_address in (Context) with (name: String, address: ArcCell) using register_address_impl);
fn register_address_impl(ctx: &mut Context, _stack: &mut Tuple, name: String, address: ArcCell) {
    let address_boc = try_ctx!(
        ctx,
        address.to_boc_b64(false),
        "Failed to encode address to BoC: {}"
    );
    let address_cell = try_ctx!(
        ctx,
        Boc::decode_base64(address_boc),
        "Failed to decode address from BoC: {}"
    );

    let addr = try_ctx!(
        ctx,
        address_cell.parse::<IntAddr>(),
        "Failed to load address from slice: {}"
    );

    ctx.build
        .known_addresses
        .addresses
        .insert(addr, KnownAddress { name });
}

extension!(register_code in (Context) with (name: String, address: ArcCell) using register_code_impl);
fn register_code_impl(ctx: &mut Context, _stack: &mut Tuple, name: String, code: ArcCell) {
    let hash = try_ctx!(ctx, code.cell_hash(), "Failed to get cell hash: {}");
    ctx.build.known_code_cells.insert(hash.to_hex(), name);
}

extension!(account_state in (Context) with (address: ArcCell) using account_state_impl);
fn account_state_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let address_boc = try_ctx!(
        ctx,
        address.to_boc_b64(false),
        "Failed to encode address to BoC: {}"
    );
    let address_cell = try_ctx!(
        ctx,
        Boc::decode_base64(address_boc),
        "Failed to decode address from BoC: {}"
    );
    let addr = try_ctx!(
        ctx,
        address_cell.parse::<IntAddr>(),
        "Failed to load address from slice: {}"
    );

    let Ok(account) = ctx
        .chain
        .blockchain
        .get_account(&addr.to_string())
        .account
        .load()
    else {
        stack.push(TupleItem::Null);
        return;
    };

    let Some(account) = account.0 else {
        stack.push(TupleItem::Null);
        return;
    };

    let mut builder = CellBuilder::new();
    try_ctx!(
        ctx,
        builder.store_bit(true),
        "Failed to store bit in cell builder: {}"
    );
    try_ctx!(
        ctx,
        account.store_into(&mut builder, Cell::empty_context()),
        "Failed to store account into cell builder: {}"
    );
    let cell = try_ctx!(
        ctx,
        builder.build(),
        "Failed to build cell from builder: {}"
    );

    let Ok(cell) = ArcCell::from_boc_b64(&Boc::encode_base64(cell)) else {
        stack.push(TupleItem::Null);
        return;
    };

    stack.push(TupleItem::Cell(cell))
}

extension!(register_lib in (Context) with (lib: ArcCell) using register_lib_impl);
fn register_lib_impl(ctx: &mut Context, _stack: &mut Tuple, lib: ArcCell) {
    let lib_boc = try_ctx!(
        ctx,
        lib.to_boc_b64(false),
        "Failed to encode lib to BoC: {}"
    );
    let cell = try_ctx!(
        ctx,
        Boc::decode_base64(lib_boc),
        "Failed to decode lib from BoC: {}"
    );
    ctx.chain.libraries.push(cell)
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        6 => build,
        7 => send_message,
        8 => run_get_method,
        9 => send_message_from,
        10 => find_transaction_by_params,
        11 => is_deployed,
        12 => get_deployed_code,
        13 => crc16,
        14 => type_name_by_opcode,
        15 => register_address,
        16 => register_code,
        17 => account_state,
        18 => register_lib,
    });
}
