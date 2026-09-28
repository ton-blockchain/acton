//! Composable, full-fidelity building blocks for TON indexers.

mod block;
mod canonical;
mod checkpoint;
mod error;
mod message;
mod model;
mod pipeline;
mod traits;

pub mod trace;

pub use block::{Batch, BlockData, DecodeError};
pub use canonical::{BlockGraphClient, BlockIdShort, CanonicalBlockSource, RawBlock, SourceError};
pub use checkpoint::{FileCheckpointStore, MemoryCheckpointStore};
pub use error::{BoxError, Error, Result};
pub use message::normalized_external_message_hash;
pub use model::{BlockId, Hash256, HashParseError};
pub use pipeline::{IndexPipeline, RunOutcome};
pub use rston;
pub use traits::{BlockSource, CheckpointStore, Sink};
