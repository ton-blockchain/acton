//! Shared Tolk debugging and replay primitives used by `acton`.
//!
//! The crate intentionally exposes a small public surface:
//! - [`replayer`] for source-level replay over VM logs or live executors
//! - [`multi`] for multi-context DAP transport/session glue
//! - [`single`] for standalone serving over one replayer
//! - convenience re-exports at the crate root for the common entry points

mod core;
pub mod exit_codes;
#[cfg(feature = "dap-server")]
pub mod multi;
#[cfg(feature = "dap-server")]
pub mod single;
#[cfg(feature = "dap-server")]
mod transport;

pub mod replayer {
    pub use crate::core::replayer::*;
}

#[cfg(feature = "live-vm")]
pub use core::DebugExecutorHandle;
#[cfg(feature = "dap-server")]
pub use multi::{ChildDebugContextSpec, ReplayerDebugSession};
#[cfg(feature = "dap-server")]
pub use multi::{
    DapMessage, DapTransport, reserve_dap_listener, start_dap_server,
    start_dap_server_with_listener,
};
#[cfg(feature = "dap-server")]
pub use single::serve_single_replayer_dap;
pub use types_render::{
    PrettyAddressFormat, PrettyRenderOptions, RenderedValue, render_tuple_as_tolk_type,
    render_tuple_item_as_tolk_type, render_unpacked_value_as_tolk_type,
};

pub(crate) use core::types_render;
