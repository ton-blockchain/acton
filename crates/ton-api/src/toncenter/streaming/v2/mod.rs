//! Types for `TonCenter` Streaming API v2.
//!
//! Streaming does not publish an `OpenAPI` document. The protocol contract is maintained in the
//! official SSE, WebSocket, and notification reference documentation.

pub mod requests;
pub mod responses;

pub use requests::*;
pub use responses::*;

pub const SSE_URL: &str = "https://toncenter.com/api/streaming/v2/sse";
pub const WEBSOCKET_URL: &str = "wss://toncenter.com/api/streaming/v2/ws";
pub const DOCUMENTATION_URL: &str = "https://docs.ton.org/api/streaming/overview";
