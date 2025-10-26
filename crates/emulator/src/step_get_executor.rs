use crate::config;
use crate::executor::ExtFunc;
use crate::get_executor::{
    GetMethodParams, GetMethodResult, create_tvm_emulator, tvm_emulator_set_gas_limit,
};
use crate::traits::{BaseExecutor, RegisterExtMethodCallback};
use crate::tuple::stack::{Tuple, serialize_tuple};
use serde::Deserialize;
use std::ffi::{CString, c_void};
use tonlib_core::tlb_types::tlb::TLB;

#[derive(Clone)]
pub struct StepGetExecutor {
    inner: *mut c_void,
}

unsafe impl Send for StepGetExecutor {}
unsafe impl Sync for StepGetExecutor {}

impl BaseExecutor for StepGetExecutor {
    /// Return true if execution is finished and false otherwise.
    fn step(&self) -> bool {
        let res = unsafe { sbs_step(self.inner) };
        res
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

impl StepGetExecutor {
    pub fn new(params: GetMethodParams) -> Self {
        let params = serde_json::to_string(&params).unwrap();
        let params_cstr = CString::new(params.as_str()).unwrap();
        StepGetExecutor {
            inner: unsafe { create_tvm_emulator(params_cstr.as_ptr()) },
        }
    }

    pub fn prepare_get_method(stack: Tuple, params: GetMethodParams) -> Self {
        let params_str = serde_json::to_string(&params).unwrap();
        let config_cstr = CString::new(config::DEFAULT_CONFIG)
            .expect("Cannot convert Config string to CString, should not happen");

        let stack = serialize_tuple(&**stack).unwrap();
        let stack_b64 = stack.to_boc_b64(false).unwrap();

        let params_cstr = CString::new(params_str).unwrap();
        let stack_b64_cstr = CString::new(stack_b64).unwrap();

        StepGetExecutor {
            inner: unsafe {
                setup_sbs_get_method(
                    params_cstr.as_ptr(),
                    stack_b64_cstr.as_ptr(),
                    config_cstr.as_ptr(),
                )
            },
        }
    }

    pub fn run_get_method(&self, method_id: i32, stack: Tuple) {
        let stack = serialize_tuple(&**stack).unwrap();
        let stack_b64 = stack.to_boc_b64(false).unwrap();
        let stack_b64_cstr = CString::new(stack_b64).unwrap();

        unsafe {
            tvm_emulator_set_gas_limit(self.inner, i64::MAX - 1000);
            tvm_emulator_sbs_run_get_method(self.inner, method_id, stack_b64_cstr.as_ptr())
        };
        // let result_str = unsafe { CString::from_raw(result_cstr).to_string_lossy().to_string() };
        // result_str
    }

    pub fn get_code_pos(&self) -> String {
        let code_pos_cstr = unsafe { sbs_get_code_pos(self.inner) };
        let code_pos_str = unsafe {
            CString::from_raw(code_pos_cstr)
                .to_string_lossy()
                .to_string()
        };
        code_pos_str
    }

    pub fn get_stack(&self) -> String {
        let stack_cstr = unsafe { sbs_get_stack(self.inner) };
        let stack_str = unsafe { CString::from_raw(stack_cstr).to_string_lossy().to_string() };
        stack_str
    }

    pub fn get_c7(&self) -> String {
        let c7_cstr = unsafe { sbs_get_c7(self.inner) };
        let c7_str = unsafe { CString::from_raw(c7_cstr).to_string_lossy().to_string() };
        c7_str
    }

    pub fn finish_get_method(&self) -> GetMethodResult {
        let result_cstr = unsafe { sbs_get_method_result(self.inner) };

        let output_str = unsafe { CString::from_raw(result_cstr).to_string_lossy().to_string() };
        let result = serde_json::from_str::<GetMethodResult>(&output_str)
            .expect("Failed to parse output, should not happen");
        result
    }
}

#[derive(Deserialize, Debug)]
struct GetInternalResult {
    output: GetMethodResult,
}

unsafe extern "C" {
    pub fn setup_sbs_get_method(
        params: *const std::os::raw::c_char,
        stack: *const std::os::raw::c_char,
        config: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_void;
    pub fn tvm_emulator_sbs_run_get_method(
        em: *mut std::os::raw::c_void,
        method_id: std::os::raw::c_int,
        stack: *const std::os::raw::c_char,
    ) -> *mut std::os::raw::c_char;
    pub fn sbs_step(tvm: *mut std::os::raw::c_void) -> bool;
    pub fn sbs_get_stack(tvm: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn sbs_get_c7(tvm: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn sbs_get_code_pos(tvm: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn sbs_get_method_result(tvm: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn tvm_emulator_register_extmethod(
        transaction_emulator: *mut std::os::raw::c_void,
        id: std::os::raw::c_int,
        ctx: *mut std::os::raw::c_void,
        callback: ExtFunc,
    ) -> *const std::os::raw::c_char;
}
