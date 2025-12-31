use core::ffi::{c_char, c_void};
use serde::{Serialize, Serializer};

/// Verbosity level for the executor logs.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub enum ExecutorVerbosity {
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
        serializer.serialize_i8(*self as i8)
    }
}

/// Callback type for custom extension methods (external opcodes).
///
/// # Arguments
///
/// * `ctx`   — User-defined context.
/// * `stack` — The current TVM stack, encoded as a Base64 BoC string.
///
/// # Returns
///
/// Must return the new stack as a Base64 BoC string. If the stack is not modified,
/// return the original `stack` pointer.
pub type ExtMethodCallback<Ctx = c_void> =
    unsafe extern "C" fn(ctx: *mut Ctx, stack: *const c_char) -> *const c_char;

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
    /// * `cb`  — The function to be called when the extension method is invoked.
    fn register_ext_method<Ctx>(
        &mut self,
        id: i32,
        ctx: &mut Ctx,
        cb: ExtMethodCallback<Ctx>,
    ) -> anyhow::Result<()>;
}
