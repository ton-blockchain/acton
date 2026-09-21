//! Native TON P2P discovery, block downloads, and a resumable disk cache.
//!
//! The client checks block hashes, predecessor links, and proof roots. It does
//! not validate consensus or execute state transitions. Consumers select shard
//! IDs and decide how to process downloaded blocks.

mod benchmark;
mod client;
mod config;
mod download;
mod identity;
mod network;
mod peers;
mod rldp;
mod storage;

pub use benchmark::{BenchmarkReport, benchmark_peers};
pub use client::{Client, ClientOptions};
pub use config::NetworkConfig;
pub use identity::load_identity;
pub use network::{BootstrapReport, NetworkOptions, bootstrap};
