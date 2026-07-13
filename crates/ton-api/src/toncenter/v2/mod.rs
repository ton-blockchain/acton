//! `TonCenter` API v2 contract.
//!
//! Source schema: <https://toncenter.com/api/v2/openapi.json>
//! (`v2.1.13-7b98025` when these definitions were consolidated).

pub mod requests;
pub mod responses;

pub use super::common::StringOrNumber;
pub use requests::*;
pub use responses::*;
pub use tvm_ffi::json_stack::{
    TvmCell, TvmList, TvmNumberDecimal, TvmSlice, TvmStackEntry, TvmTuple,
};

pub const OPENAPI_VERSION: &str = "v2.1.13-7b98025";
pub const OPENAPI_URL: &str = "https://toncenter.com/api/v2/openapi.json";
