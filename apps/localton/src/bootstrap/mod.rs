//! Local TON network startup and lifecycle management.
//!
//! The bootstrap pipeline prepares persistent state, creates a genesis when
//! necessary, starts TON processes, waits for block production, exposes the
//! configured HTTP services, and performs an orderly shutdown. Each technical
//! part of that sequence lives in a focused submodule so the top-level pipeline
//! remains readable.

mod control;
mod dht;
mod engine_config;
mod files;
mod genesis;
mod keys;
mod nodes;
mod persistence;
mod pipeline;
mod readiness;
mod validator;
mod zerostate;

pub use control::LauncherControl;
pub(crate) use persistence::{acquire_lock, validate_persisted_state};
pub use pipeline::{run, status};
