//! Shared Tolk debugging and replay primitives used by `acton`.
//!
//! The crate intentionally exposes a small public surface:
//! - [`debugger`] for DAP transport/session glue
//! - [`replayer`] for source-level replay over VM logs or live executors
//! - [`serve_retrace_dap`] for retrace-specific standalone DAP serving

mod commands;
pub mod debugger;
pub mod replayer;
mod types_render;

pub use commands::retrace::dap::serve_retrace_dap;
pub use debugger::{
    AnyExecutor, ChildDebugContextSpec, DapMessage, DapTransport, DebugSession,
    ReplayerDebugSession, reserve_dap_listener, start_dap_server, start_dap_server_with_listener,
};
