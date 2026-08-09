//! Type Language (TL) structures for TON ADNL and LiteAPI traffic.
//!
//! This module is available with the `tl` feature and contains the handwritten
//! request, response, common identifier, and ADNL message types currently used
//! by the crate. Serialization is backed by `tl-proto`, while local tests check
//! implemented constructor ids against the checked-in LiteAPI schema snapshot.
//!
//! TL constructor ids are 32-bit little-endian values on the wire. When adding
//! or changing constructors, update the local schema text, keep tests covering
//! flags/vectors/boxed values, and record unsupported upstream fields in
//! `TODO.md` rather than silently omitting them.

pub mod adnl;
pub mod common;
pub mod error;
pub mod request;
pub mod response;
#[cfg(test)]
mod schema_check;
pub mod utils;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use adnl::Message;
pub use common::{
    AccountId, BlockId, BlockIdExt, Int256, NonfinalCandidateId, NonfinalCandidateInfo,
    ZeroStateIdExt,
};
pub use error::TlError;
pub use request::{LiteQuery, LiteQueryRaw, RawWrappedRequest, Request, WrappedRequest};
pub use response::{Error, Response};
