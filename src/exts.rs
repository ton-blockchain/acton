use crate::asserts_exts::process_txs_and_search_params;
use crate::context::{AnyExecutor, Context, KnownAddress};
use crc::{CRC_16_XMODEM, Crc};
use dap::prelude::Command;
use emulator::config::DEFAULT_CONFIG;
use emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use emulator::executor::{EmulationResult, ExecutorVerbosity, RunTransactionArgs, StoreExt};
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::step_executor::StepExecutor;
use emulator::step_get_executor::StepGetExecutor;
use emulator::traits::BaseExecutor;
use emulator::tuple::stack::{Tuple, TupleItem, parse_tuple};
use emulator::{extension, pop_args, register_ext_methods};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use std::collections::HashMap;
use std::path::Path;
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::MsgAddrIntStd;
use tonlib_core::tlb_types::tlb::TLB;
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

    let result = tolkc::compile_debug(Path::new(&path));
    match result {
        tolkc::CompilerResult::Success(success) => {
            ctx.build_cache.memoize(
                &name,
                &path,
                &success.code_boc64,
                &success.code_hash_hex,
                success.source_map.unwrap(),
            );
            let code_cell = ArcCell::from_boc_b64(&*success.code_boc64).unwrap();
            stack.push(TupleItem::Cell(code_cell))
        }
        tolkc::CompilerResult::Error(error) => {
            println!("Compilation failed: {}", error.message);
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
    stack.push(TupleItem::Tuple(transaction_cells));
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

    let mut msg_slice = msg_cell.as_slice().unwrap();
    let message_obj = RelaxedMessage::load_from(&mut msg_slice).unwrap();

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

    let successful_emulations = if ctx.debug {
        let RelaxedMsgInfo::Int(int_message) = message_obj.info else {
            panic!("Emulator only supports internal messages for now");
        };

        let dest_account = blockchain.get_account(&int_message.dst.to_string());
        let code = match get_address_code(&dest_account) {
            Some(code) => Some(code),
            None => {
                if let Some(init) = message_obj.init
                    && let Some(code) = init.code
                {
                    Some(ArcCell::from_boc_b64(&Boc::encode_base64(code)).unwrap())
                } else {
                    None
                }
            }
        };

        let step_executor = StepExecutor::new();
        let source_map = ctx
            .build_cache
            .result_for_code(code)
            .map(|res| res.1.source_map);

        ctx.dbg_ctx.begin_thread(
            2,
            AnyExecutor::Message(step_executor.clone()),
            source_map,
            "Send internal message".to_string(),
        );

        let msg_cell = Emulator::patch_src_addr(msg_cell, Some(src_addr));
        let prepare_result = step_executor.prepare_transaction(
            msg_cell.clone(),
            BigInt::from(0),
            RunTransactionArgs {
                config: DEFAULT_CONFIG.to_string(),
                libs: None,
                verbosity: ExecutorVerbosity::FullLocation,
                shard_account: dest_account.clone(),
                now: 0,
                lt: blockchain.get_lt(),
                random_seed: None,
                ignore_chksig: false,
                debug_enabled: true,
                prev_blocks_info: None,
            },
        );
        if !prepare_result.success {
            panic!("Failed to prepare Emulator in debug mode");
        }

        // Step to update internal state
        ctx.dbg_ctx.next(false);

        ctx.dbg_ctx.process_incoming_requests(false).unwrap();

        let result = step_executor.finish_transaction();

        ctx.dbg_ctx.finish_thread(2);

        let result = match result {
            EmulationResult::Success(result) => result,
            EmulationResult::Error(err) => {
                stack.push(TupleItem::Tuple(vec![]));
                return;
            }
        };

        let shard_account_after = &result.shard_account;
        let shard_account_cell = Boc::decode_base64(shard_account_after).unwrap();
        let mut shard_account_slice = shard_account_cell.as_slice().unwrap();
        let shard_account = ShardAccount::load_from(&mut shard_account_slice).unwrap();

        blockchain.update_account(&int_message.dst.to_string(), &shard_account);

        let tx_cell: Cell = Boc::decode_base64(&result.transaction).unwrap();
        let mut tx_slice = tx_cell.as_slice().unwrap();
        let transaction = Transaction::load_from(&mut tx_slice).unwrap();

        let send_result = SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction: transaction.clone(),
            parent_transaction: None,
            shard_account,
            vm_log: result.vm_log,
            actions: result.actions,
        };
        vec![send_result]
    } else {
        let emulations = emulator.send_message(blockchain, msg_cell, Some(src_addr));

        let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
            SendMessageResult::Success(res) => Some((*res).clone()),
            SendMessageResult::Error(_) => None,
        });
        successful_emulations.collect::<Vec<_>>()
    };

    let transaction_cells = successful_emulations
        .iter()
        .filter_map(|emulation| ArcCell::from_boc_b64(&*emulation.raw_transaction).ok())
        .map(|tx| TupleItem::Cell(tx))
        .collect::<Vec<_>>();
    stack.push(TupleItem::Tuple(transaction_cells));
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

            if params.to.to_string() != info.dst.to_string() {
                // Destination address mismatch
                return None;
            }
        };

        let TxInfo::Ordinary(info) = tx.load_info().unwrap() else {
            return None;
        };

        if let ComputePhase::Executed(compute) = info.compute_phase {
            if let Some(expected_exit_code) = params.exit_code {
                if compute.exit_code != expected_exit_code as i32 {
                    // Exit code mismatch
                    return None;
                }
            }
        } else {
            return None;
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
    let args = args.unwrap_empty();
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
            .result_for_code(Some(code))
            .map(|res| res.1.source_map);

        ctx.dbg_ctx.begin_thread(
            2,
            AnyExecutor::Get(step_get_executor.clone()),
            source_map,
            "Send internal message".to_string(),
        );

        step_get_executor.run_get_method(method_id, Default::default());

        // Step to update internal state
        ctx.dbg_ctx.next(false);

        ctx.dbg_ctx.process_incoming_requests(false).unwrap();
        ctx.dbg_ctx.finish_thread(2);

        step_get_executor.finish_get_method()
    } else {
        let executor = GetExecutor::new(params.clone());
        executor.run_get_method(args, params)
    };

    match result {
        GetMethodResult::Success(result) => {
            let cell = ArcCell::from_boc_b64(&result.stack).unwrap();
            let tuple = parse_tuple(&cell).unwrap();

            stack.push(TupleItem::TypedTuple {
                contract_abi: ctx.abi.clone(),
                abi: ctx.abi.find_type(&return_type_name),
                type_name: return_type_name,
                items: tuple,
                accounts: blockchain.get_accounts().clone(),
                build_cache: ctx.build_cache.to_tuple_build_cache(),
                known_addresses: ctx.known_addresses.to_tuple_known_addresses(),
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
    });
}
