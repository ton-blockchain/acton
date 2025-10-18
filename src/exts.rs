use crate::compiler::{Compiler, TolkCompilerResult};
use crate::executor::{EXECUTOR, EmulationResult, Executor, get_account, update_account};
use crate::exts_lib::Tuple;
use crate::get_executor::{GetExecutor, GetMethodArgs, GetMethodInternalParams, GetMethodResult};
use crate::stack_serialization::{TupleItem, parse_tuple};
use crate::{extension, pop_args, register_ext_methods};
use core::ffi::c_char;
use num_bigint::BigInt;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::MsgAddrIntStd;
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Load, Store};
use tycho_types::models::{
    AccountState, IntAddr, RelaxedMessage, RelaxedMsgInfo, ShardAccount, StdAddr,
};

#[derive(Debug, Clone)]
pub struct AssertFailure {
    pub left: Tuple,
    pub right: Tuple,
    pub message: Option<String>,
    pub location: Option<String>,
}

static LAST_ASSERT_FAILURE: Mutex<Option<AssertFailure>> = Mutex::new(None);
static TEST_OUTPUT_BUFFER: Mutex<String> = Mutex::new(String::new());
static TEST_STDERR_BUFFER: Mutex<String> = Mutex::new(String::new());
static CAPTURE_TEST_OUTPUT: Mutex<bool> = Mutex::new(false);

pub fn get_last_assert_failure() -> Option<AssertFailure> {
    LAST_ASSERT_FAILURE.lock().unwrap().clone()
}

pub fn clear_last_assert_failure() {
    *LAST_ASSERT_FAILURE.lock().unwrap() = None;
}

pub fn start_capturing_test_output() {
    *CAPTURE_TEST_OUTPUT.lock().unwrap() = true;
    *TEST_OUTPUT_BUFFER.lock().unwrap() = String::new();
    *TEST_STDERR_BUFFER.lock().unwrap() = String::new();
}

pub fn stop_capturing_test_output() -> (String, String) {
    *CAPTURE_TEST_OUTPUT.lock().unwrap() = false;
    (
        TEST_OUTPUT_BUFFER.lock().unwrap().clone(),
        TEST_STDERR_BUFFER.lock().unwrap().clone(),
    )
}

pub fn is_capturing_test_output() -> bool {
    *CAPTURE_TEST_OUTPUT.lock().unwrap()
}

extension!(print, (s: TupleItem, type_name: String), |_stack: &mut Tuple, (s, type_name)| {
    if is_capturing_test_output() {
        let typed_tuple = if let TupleItem::Tuple(tuple) = &s {
            TupleItem::TypedTuple { type_name, items: tuple.clone() }
        } else {
            s
        };
        TEST_OUTPUT_BUFFER.lock().unwrap().push_str(&format!("{}\n", typed_tuple));
    } else {
        println!("{}", s);
    }
});

extension!(eprint, (s: String), |_stack: &mut Tuple, (s,)| {
    if is_capturing_test_output() {
        TEST_STDERR_BUFFER.lock().unwrap().push_str(&format!("{}\n", s));
    } else {
        eprintln!("{}", s);
    }
});

extension!(read_file, (path: String), |stack: &mut Tuple, (path,)| {
    match std::fs::read_to_string(&path) {
        Ok(content) => stack.push_string(&content),
        Err(_) => stack.push(TupleItem::Null),
    }
});

extension!(assert_equal, (location: String, message: String, right: Tuple, left: Tuple), |stack: &mut Tuple, (location, message, right, left): (String, String, Tuple, Tuple)| {
    if left == right {
        stack.push_bool_as_int(true);
    } else {
        *LAST_ASSERT_FAILURE.lock().unwrap() = Some(AssertFailure {
            left,
            right,
            message: Some(message),
            location: Some(location),
        });
        stack.push_bool_as_int(false);
    }
});

extension!(build, (path: String), |stack: &mut Tuple, (path,): (String,)| {
    let compiler = Compiler::new();
    let result = compiler.compile(Path::new(&path));
    match result {
        Ok(TolkCompilerResult::Success(success)) => {
            let code_cell = ArcCell::from_boc_b64(&*success.code_boc64).unwrap();
            stack.push(TupleItem::Cell(code_cell))
        }
        Ok(TolkCompilerResult::Error(error)) => {
            println!("Compilation failed: {}", error.message);
            return;
        }
        Err(e) => {
            println!("Failed to parse compilation result: {}", e);
            return;
        }
    };
});

extension!(send_message, (mode: BigInt, message: ArcCell), |stack: &mut Tuple, (mode, message): (BigInt, ArcCell)| {
    let executor = EXECUTOR.lock().unwrap();

    let msg_b64 =  message.to_boc_b64(false).unwrap();
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

    let mut builder= CellBuilder::new();
    msg2.store_into(&mut builder, Cell::empty_context()).unwrap();
    let new_cell = builder.build().unwrap();
    let base64_new = Boc::encode_base64(new_cell);

    let new_final_message = ArcCell::from_boc_b64(&base64_new).unwrap();

    let result = executor.run_transaction_cell(dst_addr.to_string(), new_final_message.clone());

    match result {
        EmulationResult::Success(result) => {
            let shard_account_after = result.shard_account;
            let acc_b64_bytes = base64::decode(&shard_account_after).unwrap();
            let acc_b64_cell = Boc::decode(acc_b64_bytes).unwrap();
            let mut slice = acc_b64_cell.as_slice().unwrap();
            let acc = ShardAccount::load_from(&mut slice).unwrap();

            update_account(dst_addr.to_string(), acc);
            stack.push(TupleItem::Tuple(vec![TupleItem::Cell(ArcCell::from_boc_b64(&*result.transaction).unwrap())]));
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
});

extension!(run_get_method, (id: BigInt, code: ArcCell, address: ArcCell), |stack: &mut Tuple, (id, code, address): (BigInt, ArcCell, ArcCell)| {
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let dst_addr_str = format!("{}:{}", &address_std.workchain, hex::encode(&address_std.address));

    let dest_address = TonAddress::from_msg_address(address_std).unwrap();

    let shard_account = get_account(dst_addr_str);
    let state = shard_account.account.load().unwrap().0.map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or(Cell::default())
    } else {
        Cell::default()
    };

    let params = GetMethodInternalParams {
        code: code.to_boc_b64(false).unwrap().to_string(),
        data: Boc::encode_base64(data),
        verbosity: 5,
        libs: "".to_string(),
        address: dest_address.to_string(),
        unixtime: 0,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: if id == BigInt::default() {0} else {id.to_u64_digits().1[0] as i32},
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let executor = GetExecutor::new(params.clone());

    let result = executor.run_get_method(GetMethodArgs{
        params,
        stack: Tuple::empty(),
    });

    match (result) {
        GetMethodResult::Success(result) => {
            let cell = ArcCell::from_boc_b64(&result.stack).unwrap();
            let tuple = parse_tuple(&cell).unwrap();

            stack.push(TupleItem::Tuple(tuple))
        }
        GetMethodResult::Error(result) => {
            println!("Error: {}", result.error);
        }
    };
});

pub fn register_extensions(executor: &mut Executor) {
    register_ext_methods!(executor, {
        1 => print,
        2 => eprint,
        3 => read_file,
        4 => assert_equal,
        6 => build,
        7 => send_message,
        8 => run_get_method,
    });
}

pub fn register_get_extensions(executor: &mut GetExecutor) {
    register_ext_methods!(executor, {
        1 => print,
        2 => eprint,
        3 => read_file,
        4 => assert_equal,
        6 => build,
        7 => send_message,
        8 => run_get_method,
    });
}
