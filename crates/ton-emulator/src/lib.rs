//! # Emulator
//!
//! A high-level library for emulating TON transactions and message flows.
//!
//! This crate provides a convenient API for simulating user interactions with smart contracts,
//! recursive processing of internal messages, and managing the state of multiple accounts
//! (optionally forked from a remote network).
//!
//! ## Core Components
//!
//! - [`Emulator`]: The primary coordinator for emulations. It manages the execution environment
//!   and provides high-level methods for sending messages.
//! - [`WorldState`]: Manages the state of all emulated accounts, logical time (LT),
//!   and global libraries.
//! - [`AccountsState`]: Defines how account data is retrieved, supporting both purely
//!   local in-memory state and forked state from `TonCenter`.
//!
//! ## Key Features
//!
//! - **High-Level Emulation**: Simple methods to send messages and get detailed results
//!   with parsed transactions and VM logs.
//! - **Recursive Flows**: Automatically processes all outgoing internal messages produced
//!   during a transaction, building a full execution trace.
//! - **Forking Support**: Easily fork the state of any account from mainnet or testnet
//!   using the [`RemoteAccountState`].
//! - **Extension DSL**: A macro-based DSL in the [`extensions`] module for defining
//!   custom external opcodes (`EXTCALL`).
//!
//! ## Example
//!
//! ```rust
//! # use ton_emulator::{Emulator, WorldState, AccountsState, LocalAccountsState};
//! # use ton_executor::ExecutorVerbosity;
//! # use tycho_types::cell::Cell;
//! #
//! # fn example(msg: Cell) -> anyhow::Result<()> {
//! // 1. Setup the state
//! let mut state = WorldState::new(AccountsState::Local(LocalAccountsState::new()), None)?;
//!
//! // 2. Create the emulator
//! let emulator = Emulator::new(ExecutorVerbosity::Short, None)?;
//!
//! // 3. Emulate a message flow
//! let results = emulator.send_message(&mut state, msg, &Default::default(), None)?;
//!
//! for result in results {
//!     // Process transaction results...
//! }
//! # Ok(())
//! # }
//! ```

pub mod emulator;
pub mod extensions;
pub mod world_state;

mod tests;

pub use crate::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
pub use crate::world_state::{AccountsState, LocalAccountsState, RemoteAccountState, WorldState};

pub use ton_executor::ExecutorVerbosity;
pub use ton_executor::message::Executor;
