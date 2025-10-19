use crate::context::Context;
use emulator::executor::{
    EmulationResult, Executor, ExecutorVerbosity, RunTransactionArgs, StoreExt,
};
use emulator::get_executor::{GetExecutor, GetMethodParams, GetMethodResult};
use emulator::tuple::stack::{Tuple, TupleItem, parse_tuple};
use emulator::{extension, pop_args, register_ext_methods};
use num_bigint::BigInt;
use std::collections::HashMap;
use std::path::Path;
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::MsgAddrIntStd;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    AccountState, IntAddr, RelaxedMessage, RelaxedMsgInfo, ShardAccount, StdAddr,
};

extension!(read_file in (Context) with (path: String) using read_file_impl);
fn read_file_impl(_ctx: &mut Context, stack: &mut Tuple, path: String) {
    match std::fs::read_to_string(&path) {
        Ok(content) => stack.push_string(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
}

extension!(build in (Context) with (path: String) using build_impl);
fn build_impl(_ctx: &mut Context, stack: &mut Tuple, path: String) {
    let result = tolkc::compile(Path::new(&path));
    match result {
        tolkc::CompilerResult::Success(success) => {
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

    let msg_b64 = message.to_boc_b64(false).unwrap();
    #[allow(deprecated)]
    let msg_b64_bytes = base64::decode(&msg_b64).unwrap();
    let msg_b64_cell = Boc::decode(msg_b64_bytes).unwrap();
    let mut slice = msg_b64_cell.as_slice().unwrap();
    let mut msg2 = RelaxedMessage::load_from(&mut slice).unwrap();

    let mut dst_addr = IntAddr::default();

    match &mut msg2.info {
        RelaxedMsgInfo::Int(info) => {
            let addr_b64 = "b5ee9c724101010100240000438015a63d6ec5cd11f837442aeba86b361f3890e715eca7c2cd44666017b8d6535d30a1578b99";
            let addr_b64_bytes = hex::decode(&addr_b64).unwrap();
            let addr_b64_cell = Boc::decode(addr_b64_bytes).unwrap();
            let mut slice = addr_b64_cell.as_slice().unwrap();

            info.src = Some(IntAddr::Std(StdAddr::load_from(&mut slice).unwrap()));
            dst_addr = info.dst.clone()
        }
        _ => {}
    }

    let account = blockchain.get_account(dst_addr.to_string());

    let params = RunTransactionArgs {
        config: emulator::config::DEFAULT_CONFIG.to_string(),
        libs: None,
        verbosity: ExecutorVerbosity::Short,
        shard_account: account,
        now: 0,
        lt: Default::default(),
        random_seed: None,
        ignore_chksig: false,
        debug_enabled: true,
        prev_blocks_info: None,
    };
    let result = blockchain
        .executor
        .run_transaction(msg2.to_cell(), mode, params);

    match result {
        EmulationResult::Success(result) => {
            let shard_account_after = result.shard_account;
            #[allow(deprecated)]
            let acc_b64_bytes = base64::decode(&shard_account_after).unwrap();
            let acc_b64_cell = Boc::decode(acc_b64_bytes).unwrap();
            let mut slice = acc_b64_cell.as_slice().unwrap();
            let acc = ShardAccount::load_from(&mut slice).unwrap();

            blockchain.update_account(dst_addr.to_string(), acc);
            stack.push(TupleItem::Tuple(vec![TupleItem::Cell(
                ArcCell::from_boc_b64(&*result.transaction).unwrap(),
            )]));
        }
        EmulationResult::Error(result) => {
            println!("Emulation error: {}", result.error);
            if let Some(vm_log) = result.vm_log {
                println!("VM log: {}", vm_log);
            }
            if let Some(vm_exit_code) = result.vm_exit_code {
                println!("VM exit code: {}", vm_exit_code);
            }

            stack.push(TupleItem::Null);
        }
    }
}

extension!(run_get_method in (Context) with (id: BigInt, code: ArcCell, address: ArcCell) using run_get_method_impl);
fn run_get_method_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    id: BigInt,
    code: ArcCell,
    address: ArcCell,
) {
    let blockchain = &mut ctx.blockchain;
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let dst_addr_str = format!(
        "{}:{}",
        &address_std.workchain,
        hex::encode(&address_std.address)
    );

    let dest_address = TonAddress::from_msg_address(address_std).unwrap();

    let shard_account = blockchain.get_account(dst_addr_str);
    let state = shard_account.account.load().unwrap().0.map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or(Cell::default())
    } else {
        Cell::default()
    };

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
        method_id: if id == BigInt::default() {
            0
        } else {
            id.to_u64_digits().1[0] as i32
        },
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let executor = GetExecutor::new(params.clone());

    let result = executor.run_get_method(Tuple::empty(), params);

    match result {
        GetMethodResult::Success(result) => {
            let cell = ArcCell::from_boc_b64(&result.stack).unwrap();
            let tuple = parse_tuple(&cell).unwrap();

            stack.push(TupleItem::Tuple(tuple))
        }
        GetMethodResult::Error(result) => {
            println!("Error: {}", result.error);
        }
    };
}

pub fn register_extensions(executor: &mut Executor, ctx: *mut std::ffi::c_void) {
    register_ext_methods!(executor, ctx, {
        3 => read_file,
        6 => build,
        7 => send_message,
        8 => run_get_method,
    });
}

pub fn register_get_extensions(executor: &mut GetExecutor, ctx: *mut std::ffi::c_void) {
    register_ext_methods!(executor, ctx, {
        3 => read_file,
        6 => build,
        7 => send_message,
        8 => run_get_method,
    });
}
