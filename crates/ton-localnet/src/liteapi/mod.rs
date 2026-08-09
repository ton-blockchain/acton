//! `LiteServer` TL interface for localnet.
//!
//! The module adapts the existing localnet HTTP/node state to the binary
//! `LiteServer` protocol used by TON indexers. It intentionally keeps transport,
//! type conversion, and proof/data cell construction in separate files so the
//! localnet node model stays focused on execution and block storage.

mod convert;
mod handler;
mod proof;
mod server;

pub(crate) use server::spawn_liteapi_server;

const LITEAPI_CAPABILITY_MASTERCHAIN_INFO_EXT: u64 = 1 << 1;
const LITEAPI_CAPABILITY_RUN_SMC_METHOD: u64 = 1 << 2;

// Local protocol version for the compatibility LiteAPI surface. The value
// matches the upstream liteserver 1.1 version that introduced advertised
// capability bits.
pub(super) const LITEAPI_VERSION: u32 = 0x101;

// Capabilities advertise only the calls localnet can answer with its stored
// block/state model. General block proof-chain support is not advertised.
pub(super) const LITEAPI_CAPABILITIES: u64 =
    LITEAPI_CAPABILITY_MASTERCHAIN_INFO_EXT | LITEAPI_CAPABILITY_RUN_SMC_METHOD;
