//! TON protocol clients and data transformations used by the launcher.
//!
//! These modules parse imported account snapshots, communicate with the local
//! liteserver, and invoke utilities from the selected TON binary distribution.
//! They do not own long-running process lifecycle; bootstrap and operations
//! build on them.

pub(crate) mod accounts;
pub(crate) mod lite;
pub(crate) mod toolchain;
