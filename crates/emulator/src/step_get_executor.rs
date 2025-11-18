use crate::config;
use crate::executor::ExtFunc;
use crate::get_executor::{
    GetMethodParams, GetMethodRawResult, GetMethodResult, GetMethodResultSuccess,
    tvm_emulator_set_gas_limit,
};
use crate::traits::{BaseExecutor, RegisterExtMethodCallback};
use std::ffi::{CString, c_void};
use std::os::raw::c_int;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;

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
    pub fn new(stack: Tuple, params: GetMethodParams) -> Self {
        let params_str = serde_json::to_string(&params).unwrap();
        let config_cstr = CString::new(config::DEFAULT_CONFIG)
            .expect("Cannot convert Config string to CString, should not happen");

        let stack = stack.serialize().unwrap();
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

    pub fn prepare_get_method(&self, method_id: i32, stack: Tuple) {
        let stack = stack.serialize().unwrap();
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

    pub fn get_control_register(&self, idx: usize) -> String {
        let control_cstr =
            unsafe { tvm_emulator_sbs_get_control_register(self.inner, idx as c_int) };
        let control_str = unsafe {
            CString::from_raw(control_cstr)
                .to_string_lossy()
                .to_string()
        };
        control_str
    }

    pub fn finish_get_method(&self, code: &String) -> GetMethodResult {
        let result_cstr = unsafe { sbs_get_method_result(self.inner) };

        let output_str = unsafe { CString::from_raw(result_cstr).to_string_lossy().to_string() };
        let result = serde_json::from_str::<GetMethodRawResult>(&output_str)
            .expect("Failed to parse output, should not happen");

        match result {
            GetMethodRawResult::Success(result) => {
                GetMethodResult::Success(GetMethodResultSuccess {
                    success: result.success,
                    stack: result.stack,
                    gas_used: result.gas_used,
                    vm_exit_code: result.vm_exit_code,
                    vm_log: result.vm_log,
                    missing_library: result.missing_library,
                    code: Boc::decode_base64(code).ok(),
                })
            }
            GetMethodRawResult::Error(err) => GetMethodResult::Error(err),
        }
    }
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
    pub fn tvm_emulator_sbs_get_control_register(
        tvm: *mut std::os::raw::c_void,
        idx: std::os::raw::c_int,
    ) -> *mut std::os::raw::c_char;
    pub fn sbs_get_code_pos(tvm: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn sbs_get_method_result(tvm: *mut std::os::raw::c_void) -> *mut std::os::raw::c_char;
    pub fn tvm_emulator_register_extmethod(
        transaction_emulator: *mut std::os::raw::c_void,
        id: std::os::raw::c_int,
        ctx: *mut std::os::raw::c_void,
        callback: ExtFunc,
    ) -> *const std::os::raw::c_char;
}
