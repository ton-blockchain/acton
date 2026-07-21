use thiserror::Error;

/// Liteserver error codes from TON `common/errorcode.h`.
///
/// Keep this list explicit so the `LiteAPI` adapter does not rely on ad-hoc
/// numeric literals when converting localnet failures into `liteServer.error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
#[allow(dead_code)] // Some TON codes are reference-only until localnet can emit them.
pub(crate) enum LiteServerErrorCode {
    Failure = 601,
    Error = 602,
    Warning = 603,
    ProtoViolation = 621,
    NotReady = 651,
    Timeout = 652,
    Cancelled = 653,
}

impl From<LiteServerErrorCode> for i32 {
    fn from(value: LiteServerErrorCode) -> Self {
        value as Self
    }
}

#[derive(Debug, Error)]
pub(crate) enum LocalnetError {
    #[error("Block {seqno} not found")]
    BlockNotFound { seqno: u32 },

    #[error("Block not found for seqno={seqno:?}, lt={lt:?}, unixtime={unixtime:?}")]
    BlockLookupNotFound {
        seqno: Option<u32>,
        lt: Option<u64>,
        unixtime: Option<u32>,
    },

    #[error("Block BoC not found for seqno={seqno}")]
    BlockDataNotFound { seqno: u32 },

    #[error("Protocol violation: {message}")]
    ProtocolViolation { message: String },

    #[error("{message}")]
    InvalidRequest { message: String },

    #[error("Timed out waiting for masterchain seqno {seqno}")]
    MasterchainWaitTimeout { seqno: u32 },

    #[error("transaction was not found")]
    TransactionNotFound,
}

impl LocalnetError {
    pub(crate) fn protocol_violation(message: impl Into<String>) -> Self {
        Self::ProtocolViolation {
            message: message.into(),
        }
    }

    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
        }
    }

    /// Maps typed localnet failures to the liteserver status codes used by TON.
    pub(crate) const fn lite_server_code(&self) -> LiteServerErrorCode {
        match self {
            Self::BlockNotFound { .. }
            | Self::BlockLookupNotFound { .. }
            | Self::BlockDataNotFound { .. }
            | Self::TransactionNotFound => LiteServerErrorCode::NotReady,
            Self::ProtocolViolation { .. } | Self::InvalidRequest { .. } => {
                LiteServerErrorCode::ProtoViolation
            }
            Self::MasterchainWaitTimeout { .. } => LiteServerErrorCode::Timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LiteServerErrorCode, LocalnetError};

    #[test]
    fn lite_server_error_codes_match_ton_errorcode_header() {
        assert_eq!(i32::from(LiteServerErrorCode::Failure), 601);
        assert_eq!(i32::from(LiteServerErrorCode::Error), 602);
        assert_eq!(i32::from(LiteServerErrorCode::Warning), 603);
        assert_eq!(i32::from(LiteServerErrorCode::ProtoViolation), 621);
        assert_eq!(i32::from(LiteServerErrorCode::NotReady), 651);
        assert_eq!(i32::from(LiteServerErrorCode::Timeout), 652);
        assert_eq!(i32::from(LiteServerErrorCode::Cancelled), 653);
    }

    #[test]
    fn block_not_found_is_reported_as_not_ready() {
        let error = LocalnetError::BlockNotFound { seqno: 42 };
        assert_eq!(error.lite_server_code(), LiteServerErrorCode::NotReady);
    }
}
