//! A library for fully reproducing (re‑tracing) any TON blockchain transaction
//! inside a local, deterministic TON Sandbox environment.
//!
//! # Overview
//!
//! `retrace` allows developers to download a specific transaction from the TON
//! network (mainnet or testnet) and replay it locally with full VM verbosity.
//! It is useful for debugging smart contracts, analyzing transaction failures,
//! and verifying on‑chain behavior.
//!
//! The library automatically handles:
//!
//! *   **Transaction Discovery**: Locating the transaction and its containing blocks.
//! *   **State Reconstruction**: Reconstructing the exact account state prior to the transaction
//!     by re-playing all preceding transactions in the same block.
//! *   **Library Resolution**: Resolving exotic library cells via external APIs (TonCenter/Dton).
//! *   **Execution Replay**: Sequential replay of all account transactions within the same master‑block.
//! *   **Detailed Reporting**: Providing a breakdown of money movements, VM logs, and generated actions.
//!
//! # Configuration
//!
//! This library can work without API keys, but a `TonCenter` API key is strongly
//! recommended for higher limits and faster execution.
//! You can also provide a Dton API key for alternative library resolution.
//!
//! *   `TONCENTER_MAINNET_API_KEY`: Your `TonCenter` mainnet V3 API key.
//! *   `TONCENTER_TESTNET_API_KEY`: Your `TonCenter` testnet V3 API key.
//! *   `DTON_API_KEY`: (Optional) Your dton.io API key for fallback library lookups.
//!
//! # Main Entry Points
//!
//! *   [`retrace`]: The simplest way to re‑trace a transaction by its hex hash.
//! *   [`find_base_tx_by_hash`]: Locates a transaction's identity (lt, hash, address).
//! *   [`retrace_base_tx`]: Re‑emulates a transaction when you already have its [`BaseTxInfo`].
//! *   [`trace::Trace`]: Utility for parsing and analyzing VM execution logs from [`TraceResult`].
//! *   [`trace::InstalledActions`]: Detailed view of actions queued (installed) during VM execution.
//! *   [`trace::ExecutedActions`]: Detailed view of actions actually processed by the sandbox.
//!
//! # Examples
//!
//! ### Basic Retrace
//!
//! ```rust,no_run
//! use ton_retrace::{Network, retrace};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let hash = "3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41";
//!     let result = retrace(Network::Mainnet, hash, Default::default()).await?;
//!
//!     if result.state_update_hash_ok {
//!         println!("Retrace successful!");
//!         println!("Exit code: {:?}", result.emulated_tx.compute_info);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### Analyzing Execution Trace
//!
//! ```rust,no_run
//! use ton_retrace::{Network, retrace};
//! use ton_retrace::trace::{Trace, TraceStep};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let hash = "3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41";
//!     let result = retrace(Network::Mainnet, hash, Default::default()).await?;
//!
//!     let trace = Trace::new(&result.emulated_tx.vm_logs, None);
//!     for step in trace.steps {
//!         match step {
//!             TraceStep::Execute { offset, instr, .. } => {
//!                 println!("{}: {}", offset, instr);
//!             }
//!             TraceStep::Exception { errno, message, handled } => {
//!                 println!("Exception {}: {} (handled: {})", errno, message, handled);
//!             }
//!             TraceStep::FinalC5 { cell } => {
//!                 println!("Final C5: {}", cell);
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ### Working with Actions
//!
//! ```rust,no_run
//! use ton_retrace::{Network, retrace};
//! use ton_retrace::trace::{Trace, ExecutedActions};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let hash = "...";
//!     let result = retrace(Network::Mainnet, hash, Default::default()).await?;
//!
//!     // 1. Actions queued by the contract (from VM logs)
//!     let trace = Trace::new(&result.emulated_tx.vm_logs, None);
//!     let installed = trace.actions();
//!     println!("Queued actions: {}", installed.actions.len());
//!
//!     // 2. Actions actually processed by the sandbox (from executor logs)
//!     let executed = ExecutedActions::from(&result.emulated_tx.executor_logs);
//!     for action in executed.actions {
//!         println!("Executed action: {:?}", action);
//!     }
//!
//!     Ok(())
//! }
//! ```

mod methods;
mod remote;
mod runner;
mod types;

#[cfg(all(test, feature = "only_ci"))]
mod tests;

pub mod trace;

pub use crate::runner::{Network, retrace, retrace_base_tx};
pub use crate::types::{
    BaseTxInfo, ComputeInfo, TraceEmulatedTx, TraceInMessage, TraceMoneyResult, TraceResult,
};
pub use methods::find_base_tx_by_hash;
