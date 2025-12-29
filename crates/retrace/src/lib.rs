//! A library for fully reproducing (re‑tracing) any TON blockchain transaction
//! inside a local, deterministic TON Sandbox environment.
//!
//! # Overview
//!
//! `retrace` allows developers to download a specific transaction from the TON
//! network (mainnet or testnet) and replay it locally with full VM verbosity.
//! It automatically handles:
//!
//! *   Locating the transaction and its containing blocks.
//! *   Reconstructing the exact account state prior to the transaction.
//! *   Resolving exotic library cells via external APIs.
//! *   Sequential replay of all account transactions within the same master‑block.
//! *   Detailed reporting of money movements, VM logs, and generated actions.
//!
//! # Configuration
//!
//! This library requires an API key for TonCenter to fetch transaction data and libraries.
//! You must set the following environment variable:
//!
//! *   `TONCENTER_API_KEY`: Your TonCenter V3 API key.
//!
//! # Main Entry Points
//!
//! *   [`retrace`]: The simplest way to re‑trace a transaction by its hex hash.
//! *   [`find_base_tx_by_hash`]: Locates a transaction's identity (lt, hash, address).
//! *   [`retrace_base_tx`]: Re‑emulates a transaction when you already have its [`BaseTxInfo`].
//!
//! # Examples
//!
//! ```ignore
//! use retrace::{Network, retrace};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let hash = "3c1b02a33390e596d83b306eab57b3f7271bc90e2e527ea4cafccfde25139d41";
//!     let result = retrace(Network::Mainnet, hash, Default::default()).await?;
//!
//!     if result.state_update_hash_ok {
//!         println!("Retrace successful! Exit code: {:?}", result.emulated_tx.compute_info);
//!     }
//!     Ok(())
//! }
//! ```

mod methods;
mod remote;
mod runner;
mod types;

#[cfg(test)]
mod tests;

pub use crate::runner::{Network, retrace, retrace_base_tx};
pub use crate::types::{
    BaseTxInfo, ComputeInfo, TraceEmulatedTx, TraceInMessage, TraceMoneyResult, TraceResult,
};
pub use methods::find_base_tx_by_hash;
