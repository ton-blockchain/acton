use crate::config::DEFAULT_CONFIG;
use crate::traits::{BaseExecutor, RegisterExtMethodCallback};
use hex;
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::ffi::{CString, c_void};
use std::ptr::null;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellFamily, Store};
use tycho_types::models::ShardAccount;
use tycho_types::prelude::CellBuilder;

pub struct Executor {
    inner: *mut c_void,
}

unsafe impl Send for Executor {}
unsafe impl Sync for Executor {}

pub trait StoreExt: Store {
    fn to_cell(&self) -> Cell;
}

impl<T: Store + ?Sized> StoreExt for T {
    fn to_cell(&self) -> Cell {
        let mut builder = CellBuilder::new();
        self.store_into(&mut builder, Cell::empty_context())
            .unwrap();
        builder.build().unwrap()
    }
}

impl BaseExecutor for Executor {
    fn step(&self) -> bool {
        false
    }

    fn register_ext_method(
        &mut self,
        id: i32,
        ctx: *mut std::os::raw::c_void,
        callback: RegisterExtMethodCallback,
    ) {
        let _ = unsafe {
            transaction_emulator_register_extmethod(self.inner, id, ctx, Some(callback));
        };
    }
}

impl Executor {
    pub fn new() -> Self {
        let config_cstr = CString::new(DEFAULT_CONFIG).unwrap();
        Executor {
            inner: unsafe { create_emulator(config_cstr.as_ptr(), 5) },
        }
    }

    pub fn run_transaction(
        &self,
        message: Cell,
        mode: BigInt,
        params: RunTransactionArgs,
    ) -> EmulationResult {
        let message = CString::new(Boc::encode_base64(message)).unwrap();

        let shard_account_cell = params.shard_account.to_cell();
        let shard_account_b64 = Boc::encode_base64(shard_account_cell);
        let shard_account_b64_cst = CString::new(shard_account_b64).unwrap();

        let params = run_common_args_to_internal_params(&params);
        let params_str = serde_json::to_string(&params).unwrap();
        let params_cstr = CString::new(params_str).unwrap();

        let result_cstr = unsafe {
            emulate_with_emulator(
                self.inner,
                null(),
                shard_account_b64_cst.as_ptr(),
                message.as_ptr(),
                params_cstr.as_ptr(),
            )
        };

        let output_str = unsafe { CString::from_raw(result_cstr).to_string_lossy().to_string() };
        let result = serde_json::from_str::<EmulationInternalResult>(&output_str).unwrap();
        result.output
    }
}

pub fn run_common_args_to_internal_params(args: &RunTransactionArgs) -> EmulationInternalParams {
    let rand_seed = match &args.random_seed {
        Some(seed) => hex::encode(seed),
        None => String::new(),
    };

    let prev_blocks_info = match &args.prev_blocks_info {
        Some(_info) => {
            panic!("TODO: Implement prev_blocks_info serialization")
        }
        None => None,
    };

    EmulationInternalParams {
        utime: args.now,
        lt: args.lt.to_string(),
        rand_seed,
        ignore_chksig: args.ignore_chksig,
        debug_enabled: args.debug_enabled,
        is_tick_tock: None,
        is_tock: None,
        prev_blocks_info,
    }
}

#[derive(Debug, Clone)]
pub enum ExecutorVerbosity {
    Short = 0,
    Full = 1,
    FullLocation = 2,
    FullLocationGas = 3,
    FullLocationStack = 4,
    FullLocationStackVerbose = 5,
}

#[derive(Debug, Clone)]
pub struct PrevBlocksInfo {
    // TODO: Add fields based on actual requirements
}

#[derive(Debug, Clone)]
pub struct RunTransactionArgs {
    pub config: String,
    pub libs: Option<Cell>,
    pub verbosity: ExecutorVerbosity,
    pub shard_account: ShardAccount,
    pub now: u32,
    pub lt: BigInt,
    pub random_seed: Option<Vec<u8>>,
    pub ignore_chksig: bool,
    pub debug_enabled: bool,
    pub prev_blocks_info: Option<PrevBlocksInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulationInternalParams {
    pub utime: u32,
    pub lt: String,
    pub rand_seed: String,
    pub ignore_chksig: bool,
    pub debug_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tick_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_tock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_blocks_info: Option<String>,
}

#[derive(Deserialize)]
struct EmulationInternalResult {
    pub output: EmulationResult,
    pub logs: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum EmulationResult {
    Success(ResultSuccess),
    Error(ResultError),
}

#[derive(Deserialize, Debug, Clone)]
pub struct ResultSuccess {
    pub transaction: String,
    pub shard_account: String,
    pub vm_log: String,
    pub actions: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ResultError {
    pub error: String,
    pub vm_log: Option<String>,
    pub vm_exit_code: Option<i64>,
}

// C FFI types

unsafe extern "C" {
    pub fn create_emulator(
        config: *const std::os::raw::c_char,
        verbosity: std::os::raw::c_int,
    ) -> *mut std::os::raw::c_void;
}
pub type ExtFunc = Option<
    unsafe extern "C" fn(
        ctx: *mut std::os::raw::c_void,
        arg1: *const std::os::raw::c_char,
    ) -> *const std::os::raw::c_char,
>;
unsafe extern "C" {
    pub fn emulate_with_emulator(
        em: *mut std::os::raw::c_void,
        libs: *const std::os::raw::c_char,
        account: *const std::os::raw::c_char,
        message: *const std::os::raw::c_char,
        params: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn emulate_sbs(
        em: *mut std::os::raw::c_void,
        libs: *const std::os::raw::c_char,
        account: *const std::os::raw::c_char,
        message: *const std::os::raw::c_char,
        params: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn em_sbs_step(em: *mut std::os::raw::c_void) -> bool;
}
unsafe extern "C" {
    pub fn em_sbs_result(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn em_sbs_code_pos(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn em_sbs_stack(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn em_sbs_c7(em: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn transaction_emulator_sbs_get_control_register(
        tvm: *mut std::os::raw::c_void,
        idx: std::os::raw::c_int,
    ) -> *mut std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn transaction_emulator_register_extmethod(
        transaction_emulator: *mut std::os::raw::c_void,
        id: std::os::raw::c_int,
        ctx: *mut std::os::raw::c_void,
        callback: ExtFunc,
    ) -> *const std::os::raw::c_char;
}
