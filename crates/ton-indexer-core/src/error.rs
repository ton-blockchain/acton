//! Shared indexer errors.

use std::error::Error as StdError;

use thiserror::Error;

/// Boxed error accepted at adapter boundaries.
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// Errors produced by the indexer pipeline.
#[derive(Debug, Error)]
pub enum Error {
    /// A block source failed.
    #[error("block source failed: {0}")]
    Source(#[source] BoxError),
    /// A sink failed.
    #[error("sink commit failed: {0}")]
    Sink(#[source] BoxError),
    /// A checkpoint store failed.
    #[error("checkpoint operation failed: {0}")]
    Checkpoint(#[source] BoxError),
    /// Input data violated an indexer invariant.
    #[error("indexer invariant violated: {0}")]
    Invariant(String),
}

impl Error {
    /// Wraps a source-specific error.
    pub fn source(error: impl Into<BoxError>) -> Self {
        Self::Source(error.into())
    }

    /// Wraps a sink-specific error.
    pub fn sink(error: impl Into<BoxError>) -> Self {
        Self::Sink(error.into())
    }

    /// Wraps a checkpoint-specific error.
    pub fn checkpoint(error: impl Into<BoxError>) -> Self {
        Self::Checkpoint(error.into())
    }
}

/// Result type used by core traits.
pub type Result<T> = std::result::Result<T, Error>;
