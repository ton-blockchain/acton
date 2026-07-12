//! Types for `TonCenter` Emulate API v1.
//!
//! The published Swagger document describes requests but leaves endpoint `responses` empty.
//! Response types therefore follow the payload returned by `TonCenter` and reuse v3 entities.

pub mod requests;
pub mod responses;

pub use requests::*;
pub use responses::*;

pub const OPENAPI_VERSION: &str = "0.0.1";
pub const OPENAPI_URL: &str = "https://toncenter.com/api/emulate/doc.json";
pub const BASE_PATH: &str = "/api/emulate/v1";
