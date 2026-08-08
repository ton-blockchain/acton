//! Minimal source, sink, and checkpoint extension points.

use async_trait::async_trait;

use crate::{Batch, BlockId, Result};

/// Produces canonical batches after an optional durable checkpoint.
#[async_trait]
pub trait BlockSource: Send {
    /// Returns the next batch, or `None` when no new canonical block is available.
    async fn next_batch(&mut self, after: Option<&BlockId>) -> Result<Option<Batch>>;
}

/// Atomically commits a canonical batch to product storage.
#[async_trait]
pub trait Sink: Send {
    /// Commits the batch. Implementations should be idempotent by masterchain id.
    async fn commit(&mut self, batch: &Batch) -> Result<()>;
}

/// Loads and stores the last successfully committed masterchain block.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Loads the current checkpoint.
    async fn load(&self) -> Result<Option<BlockId>>;

    /// Atomically replaces the current checkpoint.
    async fn save(&self, checkpoint: &BlockId) -> Result<()>;
}
