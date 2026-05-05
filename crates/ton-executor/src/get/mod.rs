//! This module provides a thin wrapper over the C++ TON get-method emulator.
//!
//! # Core Components
//!
//! - [`GetExecutor`]: The main entry point for running get-methods.
//! - [`RunGetMethodArgs`]: Parameters for running a get-method.
//! - [`GetMethodResult`]: The outcome of get-method execution.
//!
//! # Examples
//!
//! Basic usage of the get-method executor:
//!
//! ```rust,no_run
//! use ton_executor::get::{GetExecutor, RunGetMethodArgs, GetMethodResult};
//! use ton_executor::ExecutorVerbosity;
//!
//! # fn main() -> anyhow::Result<()> {
//! let args = RunGetMethodArgs {
//!     code: "te6ccg...".to_owned(),
//!     data: "te6ccg...".to_owned(),
//!     method_id: 0, // e.g., for "main" or a specific CRC32
//!     ..Default::default()
//! };
//!
//! let exec = GetExecutor::new(&args)?;
//!
//! // Base64 encoded stack BoC
//! let stack_b64 = "te6ccgEBAQEABQAABgAAAA==";
//!
//! let result = exec.run_get_method(stack_b64, &args, None)?;
//!
//! if let GetMethodResult::Success(res) = result {
//!     println!("Exit code: {}", res.vm_exit_code);
//!     println!("Result stack BoC: {}", res.stack);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Important Note on Concurrency
//!
//! Native emulator state is not safe for unsynchronized shared access.
//! [`GetExecutor`] serializes FFI calls per instance, so one executor can be shared
//! across threads, but concurrent calls on that executor are executed one at a time.

#![allow(unsafe_code)]
pub mod step;
mod tests;
pub mod types;

use core::ffi::{c_char, c_int, c_void};
pub use types::*;

use crate::{BaseExecutor, ExtMethodCallback, MissingLibraryCallback};
use anyhow::Context;
use parking_lot::ReentrantMutex;
use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::ptr::{NonNull, null};
use std::slice;
use std::str;
use std::sync::Arc;

// Opaque native emulator handle guarded by `GetExecutor::inner`.
struct RawGetExecutorHandle(NonNull<c_void>);

// SAFETY: the native emulator handle is only accessed while holding `GetExecutor::inner`.
unsafe impl Send for RawGetExecutorHandle {}

/// A thin wrapper around the C++ TON get-method emulator.
pub struct GetExecutor {
    inner: ReentrantMutex<RawGetExecutorHandle>,
    ext_methods: HashSet<i32>, // track extension methods to catch redefinitions
    params_cstr: CString,
}

impl GetExecutor {
    /// Creates a new `GetExecutor` instance.
    pub fn new(args: &RunGetMethodArgs) -> anyhow::Result<Self> {
        let params_str = serde_json::to_string(args).context("Failed to serialize args to JSON")?;
        let params_cstr = CString::new(params_str).context("Args JSON contains null bytes")?;

        // SAFETY: `create_tvm_emulator` is safe function
        let emulator_ptr = unsafe { create_tvm_emulator(params_cstr.as_ptr()) };
        let inner = NonNull::new(emulator_ptr).context("create_tvm_emulator returned null")?;

        Ok(Self {
            inner: ReentrantMutex::new(RawGetExecutorHandle(inner)),
            ext_methods: HashSet::new(),
            params_cstr,
        })
    }

    /// Runs a get-method execution.
    ///
    /// # Arguments
    ///
    /// * `stack_b64` - Base64 encoded stack `BoC`.
    /// * `args` - Execution arguments.
    /// * `config_b64` - Optional Base64 encoded blockchain configuration.
    pub fn run_get_method(
        &self,
        stack_b64: &str,
        args: &RunGetMethodArgs,
        config_b64: Option<&str>,
    ) -> anyhow::Result<GetMethodResult> {
        let stack_b64_cstr = CString::new(stack_b64).context("Stack BoC contains null bytes")?;

        let config_cstr = config_b64
            .map(|c| CString::new(c).context("Config contains null bytes"))
            .transpose()?;
        let config_ptr = config_cstr.as_ref().map_or(null(), |c| c.as_ptr());

        let inner = self.inner.lock();

        // SAFETY: native pointers come from live CStrings and the locked executor handle.
        let result_ptr = unsafe {
            // We set a very high gas limit by default for get-methods,
            // as they are typically executed off-chain and for some reason,
            // Tolk compilation consumes gas :D
            tvm_emulator_set_gas_limit(inner.0.as_ptr(), i64::MAX - 1000);

            run_get_method_struct(
                inner.0.as_ptr(),
                self.params_cstr.as_ptr(),
                stack_b64_cstr.as_ptr(),
                config_ptr,
            )
        };

        let result_ptr =
            NonNull::new(result_ptr).context("run_get_method_struct returned null pointer")?;
        let result_guard = NativeGetMethodResultGuard(result_ptr);
        // SAFETY: `result_ptr` was checked for null and is owned by `result_guard`.
        let result = unsafe { result_guard.0.as_ref() };

        if result.fail != 0 {
            let message = native_lossy_string(result.error, result.error_len);
            anyhow::bail!("Cannot run get method {}: {}", args.method_id, message);
        }

        if result.success == 0 {
            return Ok(GetMethodResult::Error(GetMethodResultError {
                success: false,
                error: Arc::from(native_lossy_string(result.error, result.error_len)),
            }));
        }

        let stack = native_required_utf8(result.stack, result.stack_len, "stack", |value| {
            Arc::from(value)
        })?;

        Ok(GetMethodResult::Success(GetMethodResultSuccess {
            success: true,
            stack,
            gas_used: result.gas_used.to_string(),
            vm_exit_code: result.vm_exit_code,
            vm_log: native_log_arc_str(result.vm_log, result.vm_log_len),
            missing_library: native_optional_string(
                result.missing_library,
                result.missing_library_len,
                "missing_library",
            )?,
            code: args.code.clone().into(),
        }))
    }

    /// Registers a custom extension method (external opcode) for the TVM.
    ///
    /// This allows extending the TVM with custom logic that can be invoked from
    /// the contract code. The registered method will be triggered whenever the
    /// `EXTCALL <ID>` instruction is executed.
    ///
    /// # Arguments
    ///
    /// * `id`       — The unique identifier for the extension method.
    /// * `ctx`      — User-defined context that will be passed back to the callback.
    /// * `callback` — The function to be called when the extension method is invoked.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ton_executor::get::GetExecutor;
    /// use ton_executor::ExtMethodCallback;
    /// use std::ffi::{c_char, c_void};
    ///
    /// struct MyCtx {
    ///     val: u32,
    /// }
    ///
    /// unsafe extern "C" fn my_callback(ctx: *mut MyCtx, stack: *const c_char) -> *const c_char {
    ///     let ctx = &mut *ctx;
    ///     ctx.val += 1;
    ///     stack
    /// }
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// # use ton_executor::get::RunGetMethodArgs;
    /// # let args = RunGetMethodArgs::default();
    /// let mut exec = GetExecutor::new(&args)?;
    /// let mut my_ctx = MyCtx { val: 0 };
    ///
    /// exec.register_ext_method(100, &mut my_ctx, 0, my_callback)?;
    /// # Ok(())
    /// # }
    /// ```
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
            tvm_emulator_register_extmethod(
                self.inner.lock().0.as_ptr(),
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

    /// Runs a serialized TVM continuation directly.
    ///
    /// # Arguments
    ///
    /// * `continuation_boc` - Base64 encoded `BoC` of the serialized `VmCont`.
    /// * `stack_boc` - Base64 encoded `BoC` of the initial stack.
    pub fn run_continuation(
        &self,
        continuation_boc: &str,
        stack_boc: &str,
    ) -> anyhow::Result<GetMethodResult> {
        let cont_cstr =
            CString::new(continuation_boc).context("Continuation BoC contains null bytes")?;
        let stack_cstr = CString::new(stack_boc).context("Stack BoC contains null bytes")?;

        let inner = self.inner.lock();

        // SAFETY: `tvm_emulator_set_gas_limit` and `tvm_emulator_run_continuation` are safe C API functions.
        let result_ptr = unsafe {
            tvm_emulator_set_gas_limit(inner.0.as_ptr(), i64::MAX - 1000);

            tvm_emulator_run_continuation(inner.0.as_ptr(), cont_cstr.as_ptr(), stack_cstr.as_ptr())
        };

        if result_ptr.is_null() {
            anyhow::bail!("tvm_emulator_run_continuation returned null pointer");
        }

        // SAFETY: The C++ side is expected to return a valid null-terminated C string.
        let output_str = unsafe { CStr::from_ptr(result_ptr).to_string_lossy() };
        let result: GetMethodResult = serde_json::from_str(&output_str)
            .with_context(|| format!("Failed to parse emulator output JSON: {output_str}"))?;

        Ok(result)
    }

    /// Registers callback that is called when TVM fails to resolve a library by hash.
    pub fn register_missing_library_callback<Ctx>(
        &mut self,
        ctx: &mut Ctx,
        callback: MissingLibraryCallback<Ctx>,
    ) -> anyhow::Result<()> {
        // SAFETY: `tvm_emulator_register_missing_library_callback` is a safe C API function.
        unsafe {
            tvm_emulator_register_missing_library_callback(
                self.inner.lock().0.as_ptr(),
                std::ptr::from_mut::<Ctx>(ctx).cast::<c_void>(),
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut Ctx, *const c_char),
                    unsafe extern "C" fn(*mut c_void, *const c_char),
                >(callback),
            );
        };

        Ok(())
    }
}

impl BaseExecutor for GetExecutor {
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
    pub(crate) fn create_tvm_emulator(params: *const c_char) -> *mut c_void;

    fn run_get_method_struct(
        em: *mut c_void,
        params: *const c_char,
        stack: *const c_char,
        config: *const c_char,
    ) -> *mut NativeGetMethodResult;

    fn tvm_emulator_get_method_result_destroy(result: *mut NativeGetMethodResult);

    pub(crate) fn tvm_emulator_register_extmethod(
        tvm_emulator: *mut c_void,
        id: c_int,
        ctx: *mut c_void,
        stack_items_count: c_int,
        callback: ExtMethodCallback<c_void>,
    ) -> *const c_char;

    pub(crate) fn tvm_emulator_register_missing_library_callback(
        tvm_emulator: *mut c_void,
        ctx: *mut c_void,
        callback: MissingLibraryCallback<c_void>,
    ) -> *const c_char;

    pub(crate) fn tvm_emulator_set_gas_limit(tvm_emulator: *mut c_void, gas_limit: i64) -> bool;

    pub(crate) fn tvm_emulator_run_continuation(
        tvm_emulator: *mut c_void,
        continuation_boc: *const c_char,
        stack_boc: *const c_char,
    ) -> *mut c_char;
}

#[repr(C)]
struct NativeGetMethodResult {
    _owner: *mut c_void,
    error: *const c_char,
    error_len: usize,
    stack: *const c_char,
    stack_len: usize,
    vm_log: *const c_char,
    vm_log_len: usize,
    missing_library: *const c_char,
    missing_library_len: usize,
    gas_used: u64,
    vm_exit_code: i32,
    success: u8,
    fail: u8,
}

struct NativeGetMethodResultGuard(NonNull<NativeGetMethodResult>);

impl Drop for NativeGetMethodResultGuard {
    fn drop(&mut self) {
        // SAFETY: the pointer came from the native get-method API and this guard owns it.
        unsafe {
            tvm_emulator_get_method_result_destroy(self.0.as_ptr());
        }
    }
}

fn native_required_utf8<T>(
    ptr: *const c_char,
    len: usize,
    field: &str,
    convert: impl FnOnce(&str) -> T,
) -> anyhow::Result<T> {
    if ptr.is_null() {
        anyhow::bail!("native get-method result field `{field}` is null");
    }
    // SAFETY: native result fields are valid for `len` bytes while the result guard is alive.
    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), len) };
    let value = str::from_utf8(bytes)
        .with_context(|| format!("native get-method result field `{field}` is not UTF-8"))?;
    Ok(convert(value))
}

fn native_optional_string(
    ptr: *const c_char,
    len: usize,
    field: &str,
) -> anyhow::Result<Option<String>> {
    if ptr.is_null() {
        return Ok(None);
    }
    native_required_utf8(ptr, len, field, ToOwned::to_owned).map(Some)
}

fn native_lossy_string(ptr: *const c_char, len: usize) -> String {
    if ptr.is_null() {
        return String::new();
    }
    // SAFETY: native result fields are valid for `len` bytes while the result guard is alive.
    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), len) };
    String::from_utf8_lossy(bytes).into_owned()
}

fn native_log_arc_str(ptr: *const c_char, len: usize) -> Arc<str> {
    if ptr.is_null() {
        return Arc::from("");
    }
    // SAFETY: native result fields are valid for `len` bytes while the result guard is alive.
    let bytes = unsafe { slice::from_raw_parts(ptr.cast::<u8>(), len) };
    if bytes.is_ascii() {
        // SAFETY: `bytes.is_ascii()` guarantees valid UTF-8.
        return Arc::from(unsafe { str::from_utf8_unchecked(bytes) });
    }
    Arc::from(String::from_utf8_lossy(bytes).into_owned())
}
