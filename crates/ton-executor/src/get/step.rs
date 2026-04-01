//! Step-by-step get-method executor.
//!
//! This module allows for detailed inspection and control over the execution of a get-method.
//!
//! # Examples
//!
//! ```rust,no_run
//! use ton_executor::get::step::StepGetExecutor;
//! use ton_executor::get::RunGetMethodArgs;
//! use ton_executor::ExecutorVerbosity;
//!
//! # fn main() -> anyhow::Result<()> {
//! let args = RunGetMethodArgs {
//!     code: "te6ccg...".to_owned(),
//!     data: "te6ccg...".to_owned(),
//!     method_id: 0,
//!     ..Default::default()
//! };
//!
//! // 1. Create the executor (without preparing execution yet)
//! let stack_b64 = "te6ccgEBAQEABQAABgAAAA==";
//! let exec = StepGetExecutor::new(stack_b64, &args, None)?;
//!
//! // 2. Prepare execution
//! exec.prepare(0, stack_b64)?;
//!
//! // 3. Step through execution
//! while !exec.step() {
//!     println!("Current code pos: {}", exec.get_code_pos());
//!     println!("Current stack: {}", exec.get_stack());
//! }
//!
//! // 4. Finish and get result
//! let result = exec.finish(&args.code)?;
//! println!("Result: {:?}", result);
//! # Ok(())
//! # }
//! ```

use crate::get::{GetMethodResult, GetMethodResultSuccess, RunGetMethodArgs};
use crate::{BaseExecutor, ExtMethodCallback, MissingLibraryCallback, get};
use anyhow::Context;
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::marker::PhantomData;
use std::ptr::{NonNull, null};
use std::rc::Rc;

/// A step-by-step get-method executor.
#[derive(Clone)]
pub struct StepGetExecutor {
    inner: NonNull<c_void>,
    ext_methods: HashSet<i32>,
    phantom: PhantomData<Rc<()>>,
}

impl StepGetExecutor {
    /// Creates a new `StepGetExecutor` instance.
    pub fn new(
        stack_b64: &str,
        params: &RunGetMethodArgs,
        config_b64: Option<&str>,
    ) -> anyhow::Result<Self> {
        let params_str =
            serde_json::to_string(params).context("Failed to serialize args to JSON")?;
        let params_cstr = CString::new(params_str).context("Args JSON contains null bytes")?;

        let config_cstr = config_b64
            .map(|c| CString::new(c).context("Config contains null bytes"))
            .transpose()?;
        let config_ptr = config_cstr.as_ref().map_or(null(), |c| c.as_ptr());

        let stack_b64_cstr = CString::new(stack_b64).context("Stack BoC contains null bytes")?;

        // SAFETY: `setup_sbs_get_method` is safe function
        let emulator_ptr = unsafe {
            setup_sbs_get_method(params_cstr.as_ptr(), stack_b64_cstr.as_ptr(), config_ptr)
        };
        let inner = NonNull::new(emulator_ptr).context("setup_sbs_get_method returned null")?;

        Ok(Self {
            inner,
            ext_methods: HashSet::new(),
            phantom: PhantomData,
        })
    }

    /// Prepares the get-method for execution.
    pub fn prepare(&self, method_id: i32, stack_b64: &str) -> anyhow::Result<()> {
        let stack_b64_cstr = CString::new(stack_b64).context("Stack BoC contains null bytes")?;

        // SAFETY: `tvm_emulator_set_gas_limit` and `tvm_emulator_sbs_run_get_method` are safe function
        unsafe {
            // We set a very high gas limit by default for get-methods,
            // as they are typically executed off-chain and for some reason,
            // Tolk compilation consumes gas :D
            get::tvm_emulator_set_gas_limit(self.inner.as_ptr(), i64::MAX - 1000);

            tvm_emulator_sbs_run_get_method(self.inner.as_ptr(), method_id, stack_b64_cstr.as_ptr())
        };
        Ok(())
    }

    /// Executes the next step. Returns `true` if execution is finished, `false` otherwise.
    #[must_use]
    pub fn step(&self) -> bool {
        // SAFETY: `sbs_step` is safe function
        unsafe { sbs_step(self.inner.as_ptr()) }
    }

    /// Gets the current code position (Base64 `BoC`).
    #[must_use]
    pub fn get_code_pos(&self) -> String {
        // SAFETY: `sbs_get_code_pos` is safe function
        let ptr = unsafe { sbs_get_code_pos(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets the current TVM instruction at the current code position.
    #[must_use]
    pub fn get_current_instr(&self) -> String {
        // SAFETY: `sbs_get_current_instr` is safe function
        let ptr = unsafe { sbs_get_current_instr(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets the current stack (Base64 `BoC`).
    #[must_use]
    pub fn get_stack(&self) -> String {
        // SAFETY: `sbs_get_stack` is safe function
        let ptr = unsafe { sbs_get_stack(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets the current C7 register (Base64 `BoC`).
    #[must_use]
    pub fn get_c7(&self) -> String {
        // SAFETY: `sbs_get_c7` is safe function
        let ptr = unsafe { sbs_get_c7(self.inner.as_ptr()) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Gets a specific control register (Base64 `BoC`).
    #[must_use]
    pub fn get_control_register(&self, idx: usize) -> String {
        // SAFETY: `tvm_emulator_sbs_get_control_register` is safe function
        let ptr =
            unsafe { tvm_emulator_sbs_get_control_register(self.inner.as_ptr(), idx as c_int) };
        if ptr.is_null() {
            return String::new();
        }
        // SAFETY: `ptr` is valid non-null pointer
        unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
    }

    /// Finishes the get-method execution and returns the result.
    pub fn finish(&self, code: &str) -> anyhow::Result<GetMethodResult> {
        // SAFETY: `sbs_get_method_result` is safe function
        let result_ptr = unsafe { sbs_get_method_result(self.inner.as_ptr()) };
        if result_ptr.is_null() {
            anyhow::bail!("sbs_get_method_result returned null pointer");
        }

        // SAFETY: `result_ptr` is valid non-null pointer
        let output_str = unsafe { CStr::from_ptr(result_ptr).to_string_lossy() };
        let result: GetMethodResult = serde_json::from_str(&output_str)
            .with_context(|| format!("Failed to parse get method result JSON: {output_str}"))?;

        match result {
            GetMethodResult::Success(success) => {
                Ok(GetMethodResult::Success(GetMethodResultSuccess {
                    code: code.into(),
                    ..success
                }))
            }
            GetMethodResult::Error(err) => Ok(GetMethodResult::Error(err)),
        }
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

        // SAFETY: `tvm_emulator_register_extmethod` is safe function
        unsafe {
            get::tvm_emulator_register_extmethod(
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
        // SAFETY: `tvm_emulator_register_missing_library_callback` is a safe C API function.
        unsafe {
            get::tvm_emulator_register_missing_library_callback(
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

impl BaseExecutor for StepGetExecutor {
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
    fn setup_sbs_get_method(
        params: *const c_char,
        stack: *const c_char,
        config: *const c_char,
    ) -> *mut c_void;
    fn tvm_emulator_sbs_run_get_method(
        em: *mut c_void,
        method_id: c_int,
        stack: *const c_char,
    ) -> *mut c_char;
    fn sbs_step(tvm: *mut c_void) -> bool;
    fn sbs_get_stack(tvm: *mut c_void) -> *mut c_char;
    fn sbs_get_c7(tvm: *mut c_void) -> *mut c_char;
    fn tvm_emulator_sbs_get_control_register(tvm: *mut c_void, idx: c_int) -> *mut c_char;
    fn sbs_get_code_pos(tvm: *mut c_void) -> *mut c_char;
    fn sbs_get_current_instr(tvm: *mut c_void) -> *mut c_char;
    fn sbs_get_method_result(tvm: *mut c_void) -> *mut c_char;
}
