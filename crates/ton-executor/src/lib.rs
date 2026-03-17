//! # ton-executor
//!
//! `ton-executor` is a thin Rust wrapper around the C++ TON transaction and TVM emulators.
//! It provides specialized executors for different use cases:
//!
//! ### Transactional Emulation
//! Used for full transaction emulation, including account state updates, gas calculation, and action processing.
//! - [`message::Executor`]: Standard transactional executor.
//! - [`message::step::StepExecutor`]: Step-by-step transactional executor for detailed debugging.
//!
//! ### Get-Method Execution
//! Optimized for executing "get-methods" of smart contracts, allowing off-chain state inspection.
//! - [`get::GetExecutor`]: Standard get-method executor.
//! - [`get::step::StepGetExecutor`]: Step-by-step get-method executor.
//!
//! ## Key Concepts
//!
//! ### Data Format
//! Most data (messages, account states, stacks) is exchanged as **Base64-encoded Bag of Cells (`BoC`)** strings.
//!
//! ### Concurrency and Thread Safety
//! `message::Executor` and `get::GetExecutor` serialize native calls per instance,
//! which makes them safe to move or share across threads. Calls on the same executor
//! still execute one at a time.
//!
//! Step-by-step executors remain session-local and should not be shared across threads.
//!
//! ### Extension Methods
//! All executors support registering custom extension methods (external opcodes) using
//! `register_ext_method`. These are triggered by the `EXTCALL <ID>` instruction in the TVM.

use ton_objs as _;

mod common;
mod config;

pub mod get;
pub mod message;

pub use common::*;
pub use config::*;
