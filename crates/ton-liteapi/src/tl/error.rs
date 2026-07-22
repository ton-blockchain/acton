use thiserror::Error;

#[derive(Debug, Error)]
pub enum TlError {
    #[error("{0}")]
    ParseError(String),
    #[error("Invalid data")]
    InvalidData,
    #[error("Unexpected end of data")]
    UnexpectedEof,
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

// Re-export from tl_proto for convenience
pub use tl_proto::{TlPacket, TlRead, TlResult, TlWrite};
