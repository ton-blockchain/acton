//! Common error types.

/// Error type for cell related errors.
#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    /// There were not enough bits or refs in the cell slice.
    #[error("cell underflow")]
    CellUnderflow,
    /// There were not enough bits or refs capacity in the cell builder.
    #[error("cell overflow")]
    CellOverflow,
    /// Something tried to load an exotic cell but an ordinary was required.
    #[error("unexpected exotic cell")]
    UnexpectedExoticCell,
    /// Something tried to load an ordinary cell but an exotic was required.
    #[error("unexpected exotic cell")]
    UnexpectedOrdinaryCell,
    /// Cell contains invalid descriptor or data.
    #[error("invalid cell")]
    InvalidCell,
    /// Data does not satisfy some constraints.
    #[error("invalid data")]
    InvalidData,
    /// Unknown TLB tag.
    #[error("invalid tag")]
    InvalidTag,
    /// Merkle proof does not contain the root cell.
    #[error("empty proof")]
    EmptyProof,
    /// Tree of cells is too deep.
    #[error("cell depth overflow")]
    DepthOverflow,
    /// Signature check failed.
    #[error("invalid signature")]
    InvalidSignature,
    /// Public key is not in a ed25519 valid range.
    #[error("invalid public key")]
    InvalidPublicKey,
    /// Underlying integer type does not fit into the target type.
    #[error("underlying integer is too large to fit in target type")]
    IntOverflow,
    /// Helper error variant for cancelled operations.
    #[error("operation cancelled")]
    Cancelled,
    /// Presented structure is unbalanced.
    #[error("unbalanced structure")]
    Unbalanced,
}

/// Error type for integer parsing related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseIntError {
    /// Error while parsing underlying type.
    #[error("cannot parse underlying integer")]
    InvalidString(#[source] std::num::ParseIntError),
    /// Underlying integer type does not fit into the target type.
    #[error("underlying integer is too large to fit in target type")]
    Overflow,
}

/// Error type for hash bytes parsing related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseHashBytesError {
    /// Failed to parse base64 encoded bytes.
    #[cfg(feature = "base64")]
    #[error("invalid base64 string")]
    InvalidBase64(#[from] base64::DecodeSliceError),
    /// Failed to parse hex encoded bytes.
    #[error("invalid hex string")]
    InvalidHex(#[from] hex::FromHexError),
    /// Error for an unexpected string length.
    #[error("expected string of 44, 64 or 66 bytes")]
    UnexpectedStringLength,
}

/// Error type for address parsing related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseAddrError {
    /// Tried to parse an empty string.
    #[error("cannot parse address from an empty string")]
    Empty,
    /// Workchain id is too large.
    #[error("workchain id is too large to fit in target type")]
    InvalidWorkchain,
    /// Invalid account id hex.
    #[error("cannot parse account id")]
    InvalidAccountId,
    /// Too many address parts.
    #[error("unexpected address part")]
    UnexpectedPart,
    /// Unexpected or invalid address format.
    #[error("invalid address format")]
    BadFormat,
}

/// Error type for block id parsing related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseBlockIdError {
    /// Tried to parse an empty string.
    #[error("cannot parse block id from an empty string")]
    Empty,
    /// Workchain id is too large.
    #[error("cannot parse workchain id")]
    InvalidWorkchain,
    /// Invalid workchain or shard prefix.
    #[error("invalid shard id")]
    InvalidShardIdent,
    /// Invalid block seqno.
    #[error("cannot parse block seqno")]
    InvalidSeqno,
    /// Invalid root hash hex.
    #[error("cannot parse root hash")]
    InvalidRootHash,
    /// Invalid file hash hex.
    #[error("cannot parse file hash")]
    InvalidFileHash,
    /// Too many block id parts.
    #[error("unexpected block id part")]
    UnexpectedPart,
}

/// Error type for global capability parsing related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ParseGlobalCapabilityError {
    /// Tried to parse an unknown string.
    #[error("unknown capability")]
    UnknownCapability,
}
