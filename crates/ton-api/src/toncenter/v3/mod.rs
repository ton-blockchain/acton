//! `TonCenter` API v3 contract.
//!
//! Source schema: <https://toncenter.com/api/v3/doc.json>
//! (Swagger `1.2.6` when these definitions were consolidated).

pub mod requests;
pub mod responses;

pub use super::common::StringOrNumber;
pub use requests::*;
pub use responses::*;

pub const OPENAPI_VERSION: &str = "1.2.6";
pub const OPENAPI_URL: &str = "https://toncenter.com/api/v3/doc.json";
