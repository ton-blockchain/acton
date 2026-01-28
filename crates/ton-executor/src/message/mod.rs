//! This module provides a thin wrapper over the C++ TON transaction emulator.
//!
//! # Core Components
//!
//! - [`Executor`]: The main entry point for running emulations. It manages the lifecycle
//!   of the underlying C++ emulator instance.
//! - [`RunTransactionArgs`]: A structure containing all necessary parameters for a
//!   transaction, including the message, shard account state, and other parameters.
//! - [`EmulationResult`]: The output of an emulation, containing the resulting transaction
//!   `BoC`, updated shard account, VM logs, and actions.
//!
//! # Data Formats
//!
//! Most data exchanged with the emulator (messages, shard accounts, stacks) is encoded
//! as **Base64 `BoC` (Bag of Cells)** strings.
//!
//! # Extension Methods (Custom Opcodes)
//!
//! The emulator supports custom extension methods that can be triggered from the TVM
//! using the `EXTCALL <ID>` instruction. Use [`Executor::register_ext_method`] to
//! hook into this mechanism and provide custom logic (e.g., for logging, debugging,
//! or mocking environment behavior).
//!
//! # Important Note on Concurrency
//!
//! The underlying C++ implementation uses **global variables**. As a result:
//! - It is impossible to have multiple active [`Executor`] instances simultaneously.
//! - Any tests or logic using this module **must be run in a single thread**.
//! - In Rust tests, use `cargo test -- --test-threads=1` to ensure correct behavior.
//!
//! # Examples
//!
//! Basic usage of the executor:
//!
//! ```rust
//! use ton_executor::message::{Executor, RunTransactionArgs};
//! use ton_executor::ExecutorVerbosity;
//!
//! # fn main() -> anyhow::Result<()> {
//! // Create a new executor with default configuration
//! let exec = Executor::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;
//!
//! let msg = "te6ccg..."; // Base64 encoded message BoC
//! let shard_account = "te6ccg..."; // Base64 encoded shard account BoC
//!
//! // Run the transaction
//! let (result, logs) = exec.run_transaction(
//!     msg,
//!     &RunTransactionArgs {
//!         shard_account: shard_account.to_owned(),
//!         now: 1000,
//!         lt: 1000,
//!         ..Default::default()
//!     },
//! )?;
//!
//! println!("Emulation logs: {}", logs);
//! # Ok(())
//! # }
//! ```
#![allow(unsafe_code)]
pub mod step;
mod tests;
pub mod types;

use core::ffi::{c_char, c_int};
use std::collections::HashSet;
pub use types::*;

use crate::common::ExecutorVerbosity;
use crate::config::DEFAULT_CONFIG;
use crate::{BaseExecutor, ExtMethodCallback};
use anyhow::Context;
use std::ffi::{CStr, CString, c_void};
use std::marker::PhantomData;
use std::ptr::{NonNull, null};
use std::rc::Rc;

/// A thin wrapper around the C++ TON transaction emulator.
///
/// Due to the use of global variables in the C++ implementation, only one
/// `Executor` should exist at a time, and it must be used from a single thread.
pub struct Executor {
    inner: NonNull<c_void>,
    ext_methods: HashSet<i32>, // track extension methods to catch redefinitions
    phantom: PhantomData<Rc<()>>, // mark as !Send and !Sync
}

impl Executor {
    /// Creates a new `Executor` instance.
    ///
    /// # Arguments
    ///
    /// * `verbosity` – The verbosity level for the emulator logs.
    /// * `config_b64` – Optional Base64 encoded blockchain configuration. If `None`,
    ///   the default configuration is used.
    ///
    /// Note: verbosity level influences the overall performance of the emulator.
    /// The more verbose the logs, the slower the emulation.
    pub fn new(verbosity: ExecutorVerbosity, config_b64: Option<&str>) -> anyhow::Result<Executor> {
        let config_b64 = config_b64.unwrap_or(DEFAULT_CONFIG);
        let config_cstr = CString::new(config_b64).context("config contains null bytes")?;

        // SAFETY: config_cstr doesn't outlive `create_emulator` call
        let emulator_ptr = unsafe { create_emulator(config_cstr.as_ptr(), verbosity as i32) };
        let inner = NonNull::new(emulator_ptr).context("create_emulator returned null")?;

        Ok(Executor {
            inner,
            ext_methods: HashSet::new(),
            phantom: PhantomData,
        })
    }

    /// Runs a transaction emulation.
    ///
    /// # Arguments
    ///
    /// * `message` – Base64 encoded message `BoC`.
    /// * `params` – Emulation parameters (shard account, current time, etc.).
    ///
    /// # Returns
    ///
    /// Returns a tuple containing the [`EmulationResult`] and the executor logs as a string.
    pub fn run_transaction(
        &self,
        message: &str,
        params: &RunTransactionArgs,
    ) -> anyhow::Result<(EmulationResult, String)> {
        let message_cstr = CString::new(message).context("message string contains null bytes")?;

        let shard_account_b64_cstr = CString::new(params.shard_account.as_str())
            .context("shard account string contains null bytes")?;

        let libs = params.libs.as_deref();
        let libs_cstr = libs
            .map(|s| CString::new(s).context("libs string contains null bytes"))
            .transpose()?;
        let libs_ptr = libs_cstr.as_ref().map_or(null(), |c| c.as_ptr());

        let internal_params = EmulationInternalParams::from(params);
        let params_str =
            serde_json::to_string(&internal_params).context("cannot serialize params to JSON")?;
        let params_cstr = CString::new(params_str).context("params string contains null bytes")?;

        // SAFETY: `libs_ptr`, `shard_account_b64_cstr`, `message_cstr`, `params_cstr`
        // do not outlive the `emulate_with_emulator` call.
        let result_ptr = unsafe {
            emulate_with_emulator(
                self.inner.as_ptr(),
                libs_ptr,
                shard_account_b64_cstr.as_ptr(),
                message_cstr.as_ptr(),
                params_cstr.as_ptr(),
            )
        };

        if result_ptr.is_null() {
            anyhow::bail!("`emulate_with_emulator` returned null pointer");
        }

        // SAFETY: pointer already checked for null. We assume the C++ side provides a valid C string.
        let output_str = unsafe { CStr::from_ptr(result_ptr).to_string_lossy() };
        let result: EmulationInternalResult = serde_json::from_str(&output_str)
            .with_context(|| format!("Failed to parse emulator output JSON: {output_str}"))?;

        match result {
            EmulationInternalResult::Success { output, logs } => Ok((output, logs)),
            EmulationInternalResult::Fail { message, .. } => {
                anyhow::bail!("Cannot run transaction: {message}");
            }
        }
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
    ///   The callback receives the `ctx` and the current `stack` (as a Base64 `BoC` string),
    ///   and must return the new `stack` (also as a Base64 `BoC` string).
    ///   If the stack remains unchanged, the callback should return the original `stack`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ton_executor::{ExecutorVerbosity, ExtMethodCallback};
    /// use ton_executor::message::Executor;
    /// use std::ffi::{c_char, c_void};
    ///
    /// struct MyContext {
    ///     called_count: u32,
    /// }
    ///
    /// unsafe extern "C" fn my_callback(ctx: *mut MyContext, stack: *const c_char) -> *const c_char {
    ///     let ctx = &mut *ctx;
    ///     ctx.called_count += 1;
    ///
    ///     // In this example, we don't modify the stack, so we return it as is.
    ///     stack
    /// }
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let mut exec = Executor::new(ExecutorVerbosity::Short, None)?;
    /// let mut my_ctx = MyContext { called_count: 0 };
    ///
    /// // Register method with ID 100. It will be called on `EXTCALL 100` instruction.
    /// exec.register_ext_method(100, &mut my_ctx, my_callback);
    /// # Ok(())
    /// # }
    /// ```
    pub fn register_ext_method<Ctx>(
        &mut self,
        id: i32,
        ctx: &mut Ctx,
        callback: ExtMethodCallback<Ctx>,
    ) -> anyhow::Result<()> {
        if !self.ext_methods.insert(id) {
            anyhow::bail!("Extension method with id {id} already registered");
        }

        // SAFETY: `transaction_emulator_register_extmethod` is safe function
        unsafe {
            transaction_emulator_register_extmethod(
                self.inner.as_ptr(),
                id,
                std::ptr::from_mut::<Ctx>(ctx).cast::<c_void>(),
                std::mem::transmute::<
                    unsafe extern "C" fn(*mut Ctx, *const i8) -> *const i8,
                    unsafe extern "C" fn(*mut c_void, *const i8) -> *const i8,
                >(callback),
            );
        };

        Ok(())
    }

    pub fn set_config(&self, config_b64: &str) -> anyhow::Result<bool> {
        let config_cstr = CString::new(config_b64).context("config contains null bytes")?;

        // SAFETY: `transaction_emulator_set_config` is safe function
        let result =
            unsafe { transaction_emulator_set_config(self.inner.as_ptr(), config_cstr.as_ptr()) };

        Ok(result)
    }
}

impl Drop for Executor {
    fn drop(&mut self) {
        // TODO: it's tricky, C++ code can destroy emulator on bad libs, for example
        // TODO: change this behaviour in C++?
        // SAFETY: self.inner is always valid non-null pointer
        // unsafe { destroy_emulator(self.inner.as_ptr()) }
    }
}

impl BaseExecutor for Executor {
    fn register_ext_method<Ctx>(
        &mut self,
        id: i32,
        ctx: &mut Ctx,
        callback: ExtMethodCallback<Ctx>,
    ) -> anyhow::Result<()> {
        self.register_ext_method(id, ctx, callback)
    }
}

unsafe extern "C" {
    /// Creates a new emulator instance.
    pub(crate) fn create_emulator(config: *const c_char, verbosity: c_int) -> *mut c_void;

    // pub(crate) fn destroy_emulator(emulator: *const core::ffi::c_void);

    /// Runs emulation using the emulator instance.
    pub(crate) fn emulate_with_emulator(
        em: *mut c_void,
        libs: *const c_char,
        account: *const c_char,
        message: *const c_char,
        params: *const c_char,
    ) -> *mut c_char;

    /// Registers a custom extension method in the emulator.
    pub(crate) fn transaction_emulator_register_extmethod(
        transaction_emulator: *mut c_void,
        id: c_int,
        ctx: *mut c_void,
        callback: ExtMethodCallback<c_void>,
    ) -> *const c_char;

    pub(crate) fn transaction_emulator_set_config(
        transaction_emulator: *mut c_void,
        config_boc: *const c_char,
    ) -> bool;
}
