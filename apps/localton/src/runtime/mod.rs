//! Process and background-task runtime used by the instance.
//!
//! This module provides execution of bounded one-shot commands, lifecycle
//! management for long-running child process groups, a shared process registry,
//! and periodic monitoring and validator-maintenance tasks.

pub(crate) mod background;
mod command;
mod process;
mod registry;
mod service;

pub use command::{CommandOutput, run_checked};
pub use process::ManagedProcess;
pub use registry::{ProcessInfo, ProcessRegistry};
pub use service::{ManagedService, ServiceExit, ServiceHandle};
