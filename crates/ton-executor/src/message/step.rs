//! Step-by-step transaction executor.
//!
//! This module allows for detailed inspection and control over the execution of a transaction.
//! It is useful for debugging and analysis tools.
//!
//! # Examples
//!
//! ```rust,no_run
//! use ton_executor::message::step::StepExecutor;
//! use ton_executor::message::RunTransactionArgs;
//! use ton_executor::ExecutorVerbosity;
//!
//! # fn main() -> anyhow::Result<()> {
//! let exec = StepExecutor::new()?;
//!
//! let msg = "te6ccg..."; // Base64 message BoC
//! let shard_account = "te6ccg..."; // Base64 shard account BoC
//!
//! // 1. Prepare the transaction
//! let args = RunTransactionArgs {
//!     shard_account: shard_account.to_owned(),
//!     now: 1000,
//!     lt: 1000,
//!     ..Default::default()
//! };
//!
//! let prepare_result = exec.prepare_transaction(msg, &args)?;
//! if prepare_result.skipped {
//!     println!("Transaction skipped");
//!     return Ok(());
//! }
//!
//! // 2. Step through execution
//! while !exec.step() {
//!     println!("Current code pos: {}", exec.get_code_pos());
//!     println!("Current stack: {}", exec.get_stack());
//! }
//!
//! // 3. Finish and get result
//! let result = exec.finish_transaction()?;
//! println!("Result: {:?}", result);
//! # Ok(())
//! # }
//! ```

use super::create_emulator;
use crate::config::DEFAULT_CONFIG;
use crate::message::types::{EmulationInternalParams, EmulationResult, RunTransactionArgs};
use crate::{BaseExecutor, ExtMethodCallback, MissingLibraryCallback};
use anyhow::Context;
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr::{NonNull, null};
use std::rc::Rc;

/// A step-by-step transaction executor.
#[derive(Clone)]
pub struct StepExecutor {
    inner: NonNull<c_void>,
    ext_methods: HashSet<i32>,
    phantom: PhantomData<Rc<()>>,
}

#[derive(serde::Deserialize, Debug)]
pub struct PrepareResult {
    pub success: bool,
    #[serde(default)]
    pub skipped: bool,
}

impl StepExecutor {
    /// Creates a new `StepExecutor` instance.
    pub fn new() -> anyhow::Result<Self> {
        let config_cstr =
            CString::new(DEFAULT_CONFIG).context("DEFAULT_CONFIG contains null bytes")?;
        // SAFETY: `create_emulator` is safe function
        let emulator_ptr = unsafe { create_emulator(config_cstr.as_ptr(), 5) };
        let inner = NonNull::new(emulator_ptr).context("create_emulator returned null")?;

        Ok(Self {
            inner,
            ext_methods: HashSet::new(),
            phantom: PhantomData,
        })
    }

    /// Prepares a transaction for step-by-step execution.
    pub fn prepare_transaction(
        &self,
        message: &str,
        params: &RunTransactionArgs,
    ) -> anyhow::Result<PrepareResult> {
        let message_cstr = CString::new(message).context("message string contains null bytes")?;

        let shard_account_b64_cstr = CString::new(params.shard_account.as_str())
            .context("shard account string contains null bytes")?;

        let libs = params.libs.as_deref();
        let libs_cstr = libs
            .map(|s| CString::new(s).context("libs string contains null bytes"))
            .transpose()?;
        let libs_ptr = libs_cstr.as_ref().map_or(null(), |c| c.as_ptr());

        let internal_params = EmulationInternalParams::try_from(params)
            .context("cannot build internal emulator params")?;
        let params_str =
            serde_json::to_string(&internal_params).context("cannot serialize params to JSON")?;
        let params_cstr = CString::new(params_str).context("params string contains null bytes")?;

        // SAFETY: `emulate_sbs` is safe function
        let result_ptr = unsafe {
            emulate_sbs(
                self.inner.as_ptr(),
                libs_ptr,
                shard_account_b64_cstr.as_ptr(),
                message_cstr.as_ptr(),
                params_cstr.as_ptr(),
            )
        };

        if result_ptr.is_null() {
            anyhow::bail!("emulate_sbs returned null pointer");
        }

        // SAFETY: `result_ptr` is valid non-null pointer
        let output_str = unsafe { CStr::from_ptr(result_ptr).to_string_lossy() };
        let result: PrepareResult = serde_json::from_str(&output_str).with_context(|| {
            format!("Failed to parse emulator prepare result JSON: {output_str}")
        })?;

        Ok(result)
    }

    /// Executes the next step. Returns `true` if execution is finished, `false` otherwise.
    #[must_use]
    pub fn step(&self) -> bool {
        // SAFETY: `em_sbs_c7` is safe function
        unsafe { em_sbs_step(self.inner.as_ptr()) }
    }

    /// Gets the current code position (Base64 `BoC`).
    #[must_use]
    pub fn get_code_pos(&self) -> String {
        // SAFETY: `em_sbs_code_pos` is safe function
        let ptr = unsafe { em_sbs_code_pos(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets the current TVM instruction at the current code position.
    #[must_use]
    pub fn get_current_instr(&self) -> String {
        // SAFETY: `em_sbs_current_instr` is safe function
        let ptr = unsafe { em_sbs_current_instr(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets the terminal uncaught exception code, if the SBS execution ended with one.
    #[must_use]
    pub fn get_uncaught_exception_code(&self) -> Option<i32> {
        // SAFETY: `transaction_emulator_sbs_get_uncaught_exception_code` is safe function
        let code =
            unsafe { transaction_emulator_sbs_get_uncaught_exception_code(self.inner.as_ptr()) };
        (code >= 0).then_some(code)
    }

    /// Gets the current stack (Base64 `BoC`).
    #[must_use]
    pub fn get_stack(&self) -> String {
        // SAFETY: `em_sbs_stack` is safe function
        let ptr = unsafe { em_sbs_stack(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets the current C7 register (Base64 `BoC`).
    #[must_use]
    pub fn get_c7(&self) -> String {
        // SAFETY: `em_sbs_c7` is safe function
        let ptr = unsafe { em_sbs_c7(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets a specific control register (Base64 `BoC`).
    #[must_use]
    pub fn get_control_register(&self, idx: usize) -> String {
        // SAFETY: `transaction_emulator_sbs_get_control_register` is safe function
        let ptr = unsafe {
            transaction_emulator_sbs_get_control_register(self.inner.as_ptr(), idx as c_int)
        };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Finishes the transaction and returns the result.
    pub fn finish_transaction(&self) -> anyhow::Result<EmulationResult> {
        // SAFETY: `em_sbs_result` is safe function
        let result_ptr = unsafe { em_sbs_result(self.inner.as_ptr()) };
        if result_ptr.is_null() {
            anyhow::bail!("em_sbs_result returned null pointer");
        }

        // SAFETY: `result_ptr` is valid non-null pointer
        let output_str = unsafe { CStr::from_ptr(result_ptr).to_string_lossy() };
        let result = serde_json::from_str::<EmulationResult>(&output_str)
            .with_context(|| format!("Failed to parse emulation result JSON: {output_str}"))?;
        Ok(result)
    }

    /// Registers a custom extension method.
    pub fn register_ext_method<Ctx>(
        &mut self,
        id: i32,
        ctx: &mut Ctx,
        stack_items_count: u8,
        callback: ExtMethodCallback<Ctx>,
    ) -> anyhow::Result<()> {
        if !self.ext_methods.insert(id) {
            anyhow::bail!("Extension method with id {id} already registered");
        }

        // SAFETY: `transaction_emulator_register_extmethod` is safe function
        unsafe {
            crate::message::transaction_emulator_register_extmethod(
                self.inner.as_ptr(),
                id,
                std::ptr::from_mut::<Ctx>(ctx).cast::<c_void>(),
                c_int::from(stack_items_count),
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut Ctx, *const c_char) -> *const c_char,
                    unsafe extern "C" fn(*mut c_void, *const c_char) -> *const c_char,
                >(callback),
            );
        };

        Ok(())
    }

    /// Registers callback that is called when TVM fails to resolve a library by hash.
    pub fn register_missing_library_callback<Ctx>(
        &mut self,
        ctx: &mut Ctx,
        callback: MissingLibraryCallback<Ctx>,
    ) -> anyhow::Result<()> {
        // SAFETY: `transaction_emulator_register_missing_library_callback` is a safe C API function.
        unsafe {
            crate::message::transaction_emulator_register_missing_library_callback(
                self.inner.as_ptr(),
                std::ptr::from_mut::<Ctx>(ctx).cast::<c_void>(),
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut Ctx, *const c_char),
                    unsafe extern "C" fn(*mut c_void, *const c_char),
                >(callback),
            );
        }

        Ok(())
    }
}

impl BaseExecutor for StepExecutor {
    fn register_ext_method<Ctx>(
        &mut self,
        id: i32,
        ctx: &mut Ctx,
        stack_items_count: u8,
        callback: ExtMethodCallback<Ctx>,
    ) -> anyhow::Result<()> {
        self.register_ext_method(id, ctx, stack_items_count, callback)
    }
}

unsafe extern "C" {
    fn emulate_sbs(
        em: *mut c_void,
        libs: *const c_char,
        account: *const c_char,
        message: *const c_char,
        params: *const c_char,
    ) -> *mut c_char;

    fn em_sbs_step(em: *mut c_void) -> bool;
    fn em_sbs_code_pos(em: *mut c_void) -> *mut c_char;
    fn em_sbs_current_instr(em: *mut c_void) -> *mut c_char;
    fn em_sbs_stack(em: *mut c_void) -> *mut c_char;
    fn em_sbs_c7(em: *mut c_void) -> *mut c_char;
    fn transaction_emulator_sbs_get_control_register(em: *mut c_void, idx: c_int) -> *mut c_char;
    fn transaction_emulator_sbs_get_uncaught_exception_code(em: *mut c_void) -> c_int;
    fn em_sbs_result(em: *mut c_void) -> *mut c_char;
}
