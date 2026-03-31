//! Shared Tolk debugging and replay primitives used by `acton`.
//!
//! The crate intentionally exposes a small public surface:
//! - [`replayer`] for source-level replay over VM logs or live executors
//! - [`multi`] for multi-context DAP transport/session glue
//! - [`single`] for standalone serving over one replayer
//! - convenience re-exports at the crate root for the common entry points

mod core;
pub mod multi;
pub mod single;
mod transport;

pub mod replayer {
    pub use crate::core::replayer::*;
}

pub use core::AnyExecutor;
pub use multi::{ChildDebugContextSpec, ReplayerDebugSession};
pub use multi::{
    DapMessage, DapTransport, reserve_dap_listener, start_dap_server,
    start_dap_server_with_listener,
};
pub use single::serve_single_replayer_dap;

pub(crate) use core::types_render;
