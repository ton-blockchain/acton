use crate::asserts_exts::process_txs_and_search_params;
use crate::context::{AnyExecutor, AssertFailure, Context, FailAssertFailure, KnownAddress};
use crate::debug_context::StepMode;
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
use emulator::{extension, pop_args, register_ext_methods};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::collections::HashMap;
use std::path::Path;
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::MsgAddrIntStd;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, IntAddr, MsgInfo, RelaxedMessage, RelaxedMsgInfo,
    ShardAccount, Transaction, TxInfo,
};

extension!(read_file in (Context) with (path: String) using read_file_impl);
fn read_file_impl(_ctx: &mut Context, stack: &mut Tuple, path: String) {
    match std::fs::read_to_string(&path) {
        Ok(content) => stack.push_string(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
}

extension!(build in (Context) with (path: String, name: String) using build_impl);
fn build_impl(ctx: &mut Context, stack: &mut Tuple, path: String, name: String) {
    if let Some(cached) = ctx.build_cache.built.get(&path) {
        let code_cell = ArcCell::from_boc_b64(&*cached.code_boc64).unwrap();
        stack.push(TupleItem::Cell(code_cell));
        return;
    }

    let result = tolkc::compile(Path::new(&path), ctx.need_debug_info);
    match result {
        tolkc::CompilerResult::Success(success) => {
            ctx.build_cache.memoize(
                &name,
                &path,
                &success.code_boc64,
                &success.code_hash_hex,
                success.source_map.unwrap_or(Default::default()),
            );
            let code_cell = ArcCell::from_boc_b64(&*success.code_boc64).unwrap();
            stack.push(TupleItem::Cell(code_cell))
        }
        tolkc::CompilerResult::Error(error) => {
            *ctx.assert_failure = Some(AssertFailure::Fail(FailAssertFailure {
                message: Some(format!("Compilation failed: {}", error.message)),
                location: None,
            }));
            stack.push(TupleItem::Null);
        }
    };
}

extension!(send_message in (Context) with (mode: BigInt, message: ArcCell) using send_message_impl);
fn send_message_impl(ctx: &mut Context, stack: &mut Tuple, mode: BigInt, message: ArcCell) {
    let blockchain = &mut ctx.blockchain;
    let emulator = &ctx.emulator;

    let msg_b64 = message.to_boc_b64(false).unwrap();
    let msg_cell = Boc::decode_base64(msg_b64).unwrap();

    // Send from null address for now
    let src_addr = IntAddr::default();
    let emulations = emulator.send_message(blockchain, msg_cell, Some(src_addr));

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some(res),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(|emulation| ArcCell::from_boc_b64(&*emulation.raw_transaction).ok())
        .map(|tx| TupleItem::Cell(tx))
        .collect::<Vec<_>>();
    stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
}

extension!(send_message_from in (Context) with (mode: BigInt, from: ArcCell, message: ArcCell) using send_message_from_impl);
fn send_message_from_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    mode: BigInt,
    from: ArcCell,
    message: ArcCell,
) {
    let blockchain = &mut ctx.blockchain;
    let emulator = &ctx.emulator;

    let msg_b64 = message.to_boc_b64(false).unwrap();
    let msg_cell = Boc::decode_base64(msg_b64).unwrap();

    let from_cell = Boc::decode_base64(from.to_boc_b64(false).unwrap()).unwrap();
    let mut from_slice = from_cell.as_slice().unwrap();
    let src_addr = IntAddr::load_from(&mut from_slice);
    let src_addr = match src_addr {
        Ok(src_addr) => src_addr,
        Err(err) => {
            ctx.fail(format!(
                "Failed to decode src address from x{{{}}} with length={}: {}",
                from_slice.display_data(),
                from_slice.size_bits(),
                err
            ));
            return;
        }
    };

    let emulations = if ctx.debug {
        send_message_debug(ctx, &msg_cell, Some(src_addr))
    } else {
        emulator.send_message(blockchain, msg_cell, Some(src_addr))
    };

    ctx.emulations.results.push(emulations.clone());

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some((*res).clone()),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(|emulation| {
            let Ok(tx) = ArcCell::from_boc_b64(&*emulation.raw_transaction) else {
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

            let result = tx.to_boc_b64(false).unwrap();
            let tx_cell: Cell = Boc::decode_base64(&result).unwrap();
            let mut tx_slice = tx_cell.as_slice().unwrap();
            let parsed_tx = Transaction::load_from(&mut tx_slice).unwrap();

            let out_messages = Tuple(
                parsed_tx
                    .iter_out_msgs()
                    .filter_map(|msg| msg.ok())
                    .filter_map(|msg| {
                        let cell = msg.to_cell();
                        let boc = Boc::encode_base64(&cell);
                        ArcCell::from_boc_b64(&boc).ok()
                    })
                    .map(|cell| TupleItem::Cell(cell))
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
                        let boc = Boc::encode_base64(&ext_cell);
                        ArcCell::from_boc_b64(&boc).ok()
                    })
                    .map(|cell| TupleItem::Cell(cell))
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
    src_addr: Option<IntAddr>,
) -> Vec<SendMessageResult> {
    let mut msg_slice = msg_cell.as_slice().unwrap();
    let message_obj = RelaxedMessage::load_from(&mut msg_slice).unwrap();

    let RelaxedMsgInfo::Int(int_message) = &message_obj.info else {
        panic!("Emulator only supports internal messages for now");
    };

    let dest_account = ctx.blockchain.get_account(&int_message.dst.to_string());
    let code = Executor::get_code_cell(&message_obj, &dest_account);

    let step_executor = StepExecutor::new();
    let source_map = ctx
        .build_cache
        .result_for_code(&code)
        .map(|res| res.1.source_map);

    let need_to_stop_on_entry = ctx.dbg_ctx.need_to_stop_child_thread_on_start();

    ctx.dbg_ctx
        .begin_thread(
            2,
            AnyExecutor::Message(step_executor.clone()),
            source_map,
            "Send internal message".to_string(),
            need_to_stop_on_entry,
        )
        .unwrap();

    let msg_cell = Emulator::patch_src_addr(msg_cell.clone(), src_addr.clone());
    let prepare_result = step_executor.prepare_transaction(
        msg_cell.clone(),
        BigInt::from(0),
        RunTransactionArgs {
            config: DEFAULT_CONFIG.to_string(),
            libs: None,
            verbosity: ExecutorVerbosity::FullLocation,
            shard_account: dest_account.clone(),
            now: 0,
            lt: ctx.blockchain.get_lt(),
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
        ctx.dbg_ctx.finish_thread(2).unwrap();
        return vec![];
    }

    // Step to update internal state
    if need_to_stop_on_entry {
        ctx.dbg_ctx.step(StepMode::StepIn);
    } else {
        ctx.dbg_ctx.step(StepMode::Continue);
    }

    if ctx.dbg_ctx.stepper.as_ref().map(|s| s.is_terminated()) == Some(false) {
        // Process requests only if we have something to execute and generates a requests
        ctx.dbg_ctx.process_incoming_requests(false).unwrap();
    }

    let result = step_executor.finish_transaction();

    ctx.dbg_ctx.finish_thread(2).unwrap();

    if ctx.dbg_ctx.performing_step != Some(StepMode::Continue) {
        // When we step out from nested message/get method, send stop message to client to
        // stop on a line after send/call get method
        ctx.dbg_ctx.step(StepMode::StepIn);
    }

    let result = match result {
        EmulationResult::Success(result) => result,
        EmulationResult::Error(_) => {
            return vec![];
        }
    };

    let shard_account_after = &result.shard_account;
    let shard_account_cell = Boc::decode_base64(shard_account_after).unwrap();
    let mut shard_account_slice = shard_account_cell.as_slice().unwrap();
    let shard_account = ShardAccount::load_from(&mut shard_account_slice).unwrap();

    ctx.blockchain
        .update_account(&int_message.dst.to_string(), &shard_account);

    let tx_cell: Cell = Boc::decode_base64(&result.transaction).unwrap();
    let mut tx_slice = tx_cell.as_slice().unwrap();
    let transaction = Transaction::load_from(&mut tx_slice).unwrap();

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

            let mut send_results = send_message_debug(ctx, &msg.to_cell(), None);
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
    _ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
) {
    if txs.0.len() == 0 {
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

    let found = parsed_txs.iter().filter_map(|tx| {
        if let Some(expected_deploy) = params.deploy {
            if expected_deploy {
                if tx.orig_status != AccountStatus::NotExists
                    || tx.end_status != AccountStatus::Active
                {
                    // We expect to deploy contract but we don't
                    return None;
                }
            }
        }

        let in_msg = tx.load_in_msg().unwrap();
        if let Some(in_msg) = &in_msg
            && let MsgInfo::Int(info) = &in_msg.info
        {
            if let Some(expected_opcode) = &params.opcode {
                let opcode = in_msg.body.clone().load_u32().unwrap();
                if *expected_opcode != opcode {
                    // Opcode mismatch
                    return None;
                }
            }

            if let Some(expected_bounced) = &params.bounced {
                if *expected_bounced != info.bounced {
                    // Bounced value mismatch
                    return None;
                }
            }

            if let Some(expected_from_addr) = &params.from {
                if (*expected_from_addr).to_string() != info.src.to_string() {
                    // Source address mismatch
                    return None;
                }
            }

            if let Some(expected_to_addr) = &params.to {
                if (*expected_to_addr).to_string() != info.dst.to_string() {
                    // Destination address mismatch
                    return None;
                }
            }
        };

        let TxInfo::Ordinary(info) = tx.load_info().unwrap() else {
            return None;
        };

        if let Some(expected_compute_skipped) = params.compute_phase_skipped {
            let is_skipped = matches!(info.compute_phase, ComputePhase::Skipped(_));
            if expected_compute_skipped != is_skipped {
                // Compute phase skipped mismatch
                return None;
            }
        }

        if let Some(expected_action_exit_code) = params.action_exit_code {
            if let Some(action_phase) = &info.action_phase {
                if action_phase.result_code != expected_action_exit_code {
                    // Action exit code mismatch
                    return None;
                }
            } else {
                // Action phase is missing but expected
                return None;
            }
        }

        if let ComputePhase::Executed(compute) = info.compute_phase {
            if let Some(expected_exit_code) = params.exit_code {
                if compute.exit_code != expected_exit_code as i32 {
                    // Exit code mismatch
                    return None;
                }
            }
        }

        return Some(tx);
    });

    let txs = found.collect::<Vec<_>>();
    if txs.is_empty() {
        stack.push(TupleItem::Null);
        return;
    }

    let first = txs.first().unwrap();
    let tx_base64 = Boc::encode_base64(first.to_cell());
    let tx_cell = ArcCell::from_boc_b64(&tx_base64).unwrap();

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
    let blockchain = &mut ctx.blockchain;
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let dst_addr_str = format!(
        "{}:{}",
        &address_std.workchain,
        hex::encode(&address_std.address)
    );

    let dest_address = TonAddress::from_msg_address(address_std).unwrap();

    let shard_account = blockchain.get_account(&dst_addr_str);
    let state = shard_account.account.load().unwrap().0.map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or(Cell::default())
    } else {
        Cell::default()
    };

    let method_id = id.to_i32().unwrap_or(0);
    let params = GetMethodParams {
        code: code.to_boc_b64(false).unwrap().to_string(),
        data: Boc::encode_base64(data),
        verbosity: 5,
        libs: "".to_string(),
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

    let result = if ctx.debug {
        let step_get_executor = StepGetExecutor::new(Default::default(), params.clone());

        let source_map = ctx
            .build_cache
            .result_for_code(&Some(
                Boc::decode_base64(code.to_boc_b64(false).unwrap()).unwrap(),
            ))
            .map(|res| res.1.source_map);

        ctx.dbg_ctx
            .begin_thread(
                2,
                AnyExecutor::Get(step_get_executor.clone()),
                source_map,
                "Send internal message".to_string(),
                ctx.dbg_ctx.need_to_stop_child_thread_on_start(),
            )
            .unwrap();

        step_get_executor.run_get_method(method_id, Default::default());

        // Step to update internal state
        if ctx.dbg_ctx.need_to_stop_child_thread_on_start() {
            ctx.dbg_ctx.step(StepMode::StepIn);
        } else {
            ctx.dbg_ctx.step(StepMode::Continue);
        }

        if ctx.dbg_ctx.stepper.as_ref().map(|s| s.is_terminated()) == Some(false) {
            ctx.dbg_ctx.process_incoming_requests(false).unwrap();
        }

        ctx.dbg_ctx.finish_thread(2).unwrap();

        if ctx.dbg_ctx.performing_step != Some(StepMode::Continue) {
            // When we step out from nested message/get method, send stop message to client to
            // stop on a line after send/call get method
            ctx.dbg_ctx.step(StepMode::StepIn);
        }

        step_get_executor.finish_get_method(&params.code)
    } else {
        let executor = GetExecutor::new(params.clone());
        executor.run_get_method(args, params)
    };

    match result {
        GetMethodResult::Success(result) => {
            ctx.emulations.get_results.push(result.clone());

            let cell = ArcCell::from_boc_b64(&result.stack).unwrap();
            let tuple = Tuple::deserialize(&cell).unwrap();

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
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let dst_addr_str = format!(
        "{}:{}",
        &address_std.workchain,
        hex::encode(&address_std.address)
    );

    let is_deployed = ctx.blockchain.is_deployed(&dst_addr_str);
    stack.push_bool(is_deployed);
}

extension!(get_deployed_code in (Context) with (address: ArcCell) using get_deployed_code_impl);
fn get_deployed_code_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let dst_addr_str = format!(
        "{}:{}",
        &address_std.workchain,
        hex::encode(&address_std.address)
    );

    let is_deployed = ctx.blockchain.is_deployed(&dst_addr_str);
    if !is_deployed {
        stack.push(TupleItem::Null);
        return;
    }

    let account = ctx.blockchain.get_account(&dst_addr_str);
    let cell = match get_address_code(&account) {
        Some(value) => value,
        None => {
            stack.push(TupleItem::Null);
            return;
        }
    };

    stack.push(TupleItem::Cell(cell));
}

fn get_address_code(account: &ShardAccount) -> Option<ArcCell> {
    let state = account.account.load().unwrap().0.map(|s| s.state);

    let Some(AccountState::Active(state)) = state else {
        return None;
    };

    let Some(code) = state.code else {
        return None;
    };

    let Ok(cell) = ArcCell::from_boc_b64(&Boc::encode_base64(code)) else {
        return None;
    };

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
    let type_abi = ctx.abi.find_type_by_opcode(id);
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
fn register_address_impl(ctx: &mut Context, stack: &mut Tuple, name: String, address: ArcCell) {
    let address_cell = Boc::decode_base64(address.to_boc_b64(false).unwrap()).unwrap();
    let mut address_slice = address_cell.parse().unwrap();

    let addr = IntAddr::load_from(&mut address_slice).unwrap();

    ctx.known_addresses
        .addresses
        .insert(addr, KnownAddress { name });
}

extension!(register_code in (Context) with (name: String, address: ArcCell) using register_code_impl);
fn register_code_impl(ctx: &mut Context, _stack: &mut Tuple, name: String, code: ArcCell) {
    ctx.known_code_cells
        .insert(code.cell_hash().unwrap().to_hex(), name);
}

pub fn register_extensions(executor: &mut dyn BaseExecutor, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        3 => read_file,
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
    });
}
