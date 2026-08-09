//! User-facing operations performed against a prepared local TON network.
//!
//! Node management, validator elections, wallets, and hardfork creation are
//! shared by CLI commands, HTTP handlers, and periodic runtime maintenance.

pub(crate) mod hardfork;
pub(crate) mod indexer;
pub(crate) mod nodes;
pub(crate) mod snapshots;
pub(crate) mod validators;
pub(crate) mod wallets;
