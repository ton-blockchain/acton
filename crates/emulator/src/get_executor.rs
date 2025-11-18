use crate::config::DEFAULT_CONFIG;
use crate::executor::{ExecutorVerbosity, ExtFunc};
use crate::traits::{BaseExecutor, RegisterExtMethodCallback};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CString, c_void};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

pub struct GetExecutor {
    inner: *mut c_void,
}

impl BaseExecutor for GetExecutor {
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
            tvm_emulator_register_extmethod(self.inner, id, ctx, Some(callback));
        };
    }
}

impl GetExecutor {
    pub fn new(params: GetMethodParams) -> Self {
        let params = serde_json::to_string(&params).unwrap();
        let params_cstr = CString::new(params.as_str()).unwrap();
        GetExecutor {
            inner: unsafe { create_tvm_emulator(params_cstr.as_ptr()) },
        }
    }

    pub fn run_get_method(&self, stack: Tuple, params: GetMethodParams) -> GetMethodResult {
        let params_str = serde_json::to_string(&params).unwrap();
        let config_cstr = CString::new(DEFAULT_CONFIG)
            .expect("Cannot convert Config string to CString, should not happen");

        let stack = stack.serialize().unwrap();
        let stack_b64 = stack.to_boc_b64(false).unwrap();

        let run_result = unsafe {
            tvm_emulator_set_gas_limit(self.inner, i64::MAX - 1000);

            let params_cstr = CString::new(params_str.as_str()).unwrap();
            let stack_b64_cstr = CString::new(stack_b64).unwrap();
            run_get_method(
                self.inner,
                params_cstr.into_raw(),
                stack_b64_cstr.into_raw(),
                config_cstr.into_raw(),
            )
        };

        let output_str = unsafe { CString::from_raw(run_result).to_string_lossy().to_string() };

        let result = serde_json::from_str::<GetInternalResult>(&output_str)
            .expect("Failed to parse output, should not happen");
        match result.output {
            GetMethodRawResult::Success(result) => {
                GetMethodResult::Success(GetMethodResultSuccess {
                    success: result.success,
                    stack: result.stack,
                    gas_used: result.gas_used,
                    vm_exit_code: result.vm_exit_code,
                    vm_log: result.vm_log,
                    missing_library: result.missing_library,
                    code: Boc::decode_base64(params.code).ok(),
                })
            }
            GetMethodRawResult::Error(err) => GetMethodResult::Error(err),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct GetMethodParams {
    pub code: String,
    pub data: String,
    pub verbosity: ExecutorVerbosity,
    pub libs: String,
    pub address: String,
    pub unixtime: i64,
    pub balance: String,
    pub rand_seed: String,
    pub gas_limit: String,
    pub method_id: i32,
    pub debug_enabled: bool,
    #[serde(default)]
    pub extra_currencies: HashMap<String, String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_blocks_info: Option<String>,
}

#[derive(Deserialize, Debug)]
struct GetInternalResult {
    output: GetMethodRawResult,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum GetMethodRawResult {
    Success(GetMethodRawResultSuccess),
    Error(GetMethodResultError),
}

#[derive(Deserialize, Clone, Debug)]
pub struct GetMethodRawResultSuccess {
    pub success: bool,
    pub stack: String,
    pub gas_used: String,
    pub vm_exit_code: i32,
    pub vm_log: String,
    pub missing_library: Option<String>,
}

#[derive(Debug, Clone)]
pub enum GetMethodResult {
    Success(GetMethodResultSuccess),
    Error(GetMethodResultError),
}

#[derive(Clone, Debug)]
pub struct GetMethodResultSuccess {
    pub success: bool,
    pub stack: String,
    pub gas_used: String,
    pub vm_exit_code: i32,
    pub vm_log: String,
    pub missing_library: Option<String>,
    pub code: Option<Cell>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GetMethodResultError {
    pub success: bool,
    pub error: String,
}

unsafe extern "C" {
    pub fn tvm_emulator_register_extmethod(
        transaction_emulator: *mut std::os::raw::c_void,
        id: std::os::raw::c_int,
        ctx: *mut std::os::raw::c_void,
        callback: ExtFunc,
    ) -> *const std::os::raw::c_char;
}
unsafe extern "C" {
    pub fn create_tvm_emulator(params: *const std::os::raw::c_char) -> *mut std::os::raw::c_void;
}
unsafe extern "C" {
    pub fn run_get_method(
        em: *mut std::os::raw::c_void,
        params: *const std::os::raw::c_char,
        stack: *const std::os::raw::c_char,
        config: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
}

unsafe extern "C" {
    pub fn tvm_emulator_set_gas_limit(
        tvm_emulator: *mut std::os::raw::c_void,
        gas_limit: i64,
    ) -> bool;
}
