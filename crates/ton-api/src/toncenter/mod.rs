//! `TonCenter` wire types grouped by API generation.
//!
//! The modules follow the upstream schemas instead of the client call sites that
//! happen to consume them. This keeps request and response DTOs reusable by both
//! the remote client and the localnet-compatible server.

mod common;
pub mod emulate;
pub mod streaming;
pub mod v2;
pub mod v3;
