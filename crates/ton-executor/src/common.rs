use core::ffi::{c_char, c_void};
use rustc_hash::FxHashSet;
use serde::{Serialize, Serializer};
use std::ffi::CStr;

/// Verbosity level for the executor logs.
#[repr(i32)]
#[derive(Debug, Clone, Copy, Default)]
pub enum ExecutorVerbosity {
    /// Disable VM logging completely without building log messages in the native emulator.
    Off = -1,
    /// Minimal logging.
    #[default]
    Short = 0,
    /// Detailed logging.
    Full = 1,
    /// Logging with location information.
    FullLocation = 2,
    /// Logging with location and gas consumption.
    FullLocationGas = 3,
    /// Logging with location and stack state.
    FullLocationStack = 4,
    /// Extremely detailed logging with location, stack, and more.
    FullLocationStackVerbose = 5,
}

impl Serialize for ExecutorVerbosity {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(*self as i32)
    }
}

/// Callback type for custom extension methods (external opcodes).
///
/// # Arguments
///
/// * `ctx`   — User-defined context.
/// * `stack` — The current TVM stack, encoded as a Base64 `BoC` string.
///
/// # Returns
///
/// Must return the new stack as a Base64 `BoC` string. If the stack is not modified,
/// return the original `stack` pointer.
pub type ExtMethodCallback<Ctx = c_void> =
    unsafe extern "C" fn(ctx: *mut Ctx, stack: *const c_char) -> *const c_char;

/// Callback type for missing global library notifications.
///
/// # Arguments
///
/// * `ctx` — User-defined context.
/// * `hash` — Missing library hash as lowercase hex string (64 chars).
pub type MissingLibraryCallback<Ctx = c_void> =
    unsafe extern "C" fn(ctx: *mut Ctx, hash: *const c_char);

/// Collector for missing global-library hashes reported by the native emulator.
#[derive(Default)]
pub struct MissingLibrariesContext {
    hashes: FxHashSet<String>,
}

impl MissingLibrariesContext {
    #[must_use]
    pub fn into_set(self) -> FxHashSet<String> {
        self.hashes
    }
}

/// Default callback that records missing library hashes into [`MissingLibrariesContext`].
///
/// # Safety
///
/// `ctx` must either be null or point to a valid [`MissingLibrariesContext`] that
/// remains alive for the duration of the callback. `hash` must either be null or
/// point to a valid NUL-terminated C string for the duration of the call.
#[allow(unsafe_code)]
pub unsafe extern "C" fn missing_library_callback(
    ctx: *mut MissingLibrariesContext,
    hash: *const c_char,
) {
    if ctx.is_null() || hash.is_null() {
        return;
    }

    // SAFETY: `hash` is provided by the emulator callback contract and points to a valid C string
    // for the duration of this callback.
    let hash = unsafe { CStr::from_ptr(hash) }
        .to_string_lossy()
        .into_owned();

    // SAFETY: `ctx` points to `MissingLibrariesContext` owned by the caller and lives
    // until the executor finishes the current run.
    if let Some(state) = unsafe { ctx.as_mut() } {
        state.hashes.insert(hash);
    }
}

pub const EXT_METHOD_STACK_ALL_ITEMS: u8 = u8::MAX;

/// Base trait for all TON executors.
///
/// Provides common functionality shared between standard and step-by-step executors.
pub trait BaseExecutor {
    /// Registers a custom extension method (external opcode) for the TVM.
    ///
    /// This allows extending the TVM with custom logic that can be invoked from
    /// the contract code using the `EXTCALL <ID>` instruction.
    ///
    /// # Arguments
    ///
    /// * `id`  — The unique identifier for the extension method.
    /// * `ctx` — User-defined context that will be passed back to the callback.
    /// * `stack_items_count` defines how many top stack items are passed to callback:
    ///      - `0..=254` — exact number of top items (clamped by current stack depth)
    ///      - `255` — pass all stack items
    /// * `cb`  — The function to be called when the extension method is invoked.
    fn register_ext_method<Ctx>(
        &mut self,
        id: i32,
        ctx: &mut Ctx,
        stack_items_count: u8,
        cb: ExtMethodCallback<Ctx>,
    ) -> anyhow::Result<()>;
}
