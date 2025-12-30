use crate::config::DEFAULT_CONFIG;
use crate::executor::{
    RegisterExtMethodCallback, create_emulator, em_sbs_c7, em_sbs_code_pos, em_sbs_result,
    em_sbs_stack, em_sbs_step, emulate_sbs, transaction_emulator_register_extmethod,
    transaction_emulator_sbs_get_control_register,
};
use crate::utils::{BaseExecutor, EmulationInternalParams};
use num_bigint::BigInt;
use serde::Deserialize;
use std::ffi::{CString, c_void};
use std::os::raw::c_int;
use std::ptr::null;
use ton_executor::message::{EmulationResult, RunTransactionArgs};
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

#[derive(Clone)]
pub struct StepExecutor {
    inner: *mut c_void,
}

unsafe impl Send for StepExecutor {}
unsafe impl Sync for StepExecutor {}

impl BaseExecutor for StepExecutor {
    /// Return true if execution is finished and false otherwise.
    fn step(&self) -> bool {
        let res = unsafe { em_sbs_step(self.inner) };
        res
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

impl StepExecutor {
    pub fn new() -> Self {
        let config_cstr = CString::new(DEFAULT_CONFIG).expect("DEFAULT_CONFIG contains null bytes");
        StepExecutor {
            inner: unsafe { create_emulator(config_cstr.as_ptr(), 5) },
        }
    }

    pub fn prepare_transaction(
        &self,
        message: Cell,
        _mode: BigInt,
        params: RunTransactionArgs,
    ) -> PrepareResult {
        let message = CString::new(Boc::encode_base64(message))
            .expect("Failed to create C string from message BOC");

        let shard_account_b64_cst = CString::new(params.shard_account.clone())
            .expect("Failed to create C string from shard account BOC");

        let has_libs = params.libs.is_some();
        let libs = params.libs.as_deref().unwrap_or("");
        let libs_cstr = CString::new(libs).unwrap();
        let libs_ptr = libs_cstr.as_ptr();

        let internal_params = EmulationInternalParams::from(&params);
        let params_str = serde_json::to_string(&internal_params)
            .expect("Failed to serialize emulation params to JSON");
        let params_cstr =
            CString::new(params_str).expect("Failed to create C string from params JSON");

        let result_cstr = unsafe {
            emulate_sbs(
                self.inner,
                if has_libs { libs_ptr } else { null() },
                shard_account_b64_cst.as_ptr(),
                message.as_ptr(),
                params_cstr.as_ptr(),
            )
        };

        let output_str = unsafe { CString::from_raw(result_cstr).to_string_lossy().to_string() };
        let result = serde_json::from_str::<PrepareResult>(&output_str)
            .expect("Failed to parse emulator prepare result JSON");
        result
    }

    pub fn get_code_pos(&self) -> String {
        let code_pos_cstr = unsafe { em_sbs_code_pos(self.inner) };
        let code_pos_str = unsafe {
            CString::from_raw(code_pos_cstr)
                .to_string_lossy()
                .to_string()
        };
        code_pos_str
    }

    pub fn get_stack(&self) -> String {
        let stack_cstr = unsafe { em_sbs_stack(self.inner) };
        let stack_str = unsafe { CString::from_raw(stack_cstr).to_string_lossy().to_string() };
        stack_str
    }

    pub fn get_c7(&self) -> String {
        let c7_cstr = unsafe { em_sbs_c7(self.inner) };
        let c7_str = unsafe { CString::from_raw(c7_cstr).to_string_lossy().to_string() };
        c7_str
    }

    pub fn get_control_register(&self, idx: usize) -> String {
        let control_cstr =
            unsafe { transaction_emulator_sbs_get_control_register(self.inner, idx as c_int) };
        let control_str = unsafe {
            CString::from_raw(control_cstr)
                .to_string_lossy()
                .to_string()
        };
        control_str
    }

    pub fn finish_transaction(&self) -> EmulationResult {
        let result_cstr = unsafe { em_sbs_result(self.inner) };

        let output_str = unsafe { CString::from_raw(result_cstr).to_string_lossy().to_string() };
        let result = serde_json::from_str::<EmulationResult>(&output_str).unwrap();
        result
    }
}

#[derive(Deserialize, Debug)]
pub struct PrepareResult {
    pub success: bool,
    pub skipped: bool,
}
