//! Durable and in-memory checkpoint stores.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{BlockId, CheckpointStore, Error, Result};

/// Process-local checkpoint store for tests and ephemeral indexers.
#[derive(Clone, Debug, Default)]
pub struct MemoryCheckpointStore {
    value: Arc<RwLock<Option<BlockId>>>,
}

impl MemoryCheckpointStore {
    /// Creates a memory store with an initial value.
    #[must_use]
    pub fn with_checkpoint(checkpoint: BlockId) -> Self {
        Self {
            value: Arc::new(RwLock::new(Some(checkpoint))),
        }
    }
}

#[async_trait]
impl CheckpointStore for MemoryCheckpointStore {
    async fn load(&self) -> Result<Option<BlockId>> {
        Ok(*self.value.read().await)
    }

    async fn save(&self, checkpoint: &BlockId) -> Result<()> {
        *self.value.write().await = Some(*checkpoint);
        Ok(())
    }
}

/// Checkpoint store backed by an atomically replaced JSON file.
#[derive(Clone, Debug)]
pub struct FileCheckpointStore {
    path: PathBuf,
}

impl FileCheckpointStore {
    /// Creates a file checkpoint store.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the checkpoint path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[async_trait]
impl CheckpointStore for FileCheckpointStore {
    async fn load(&self) -> Result<Option<BlockId>> {
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Error::checkpoint(error)),
        };

        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(Error::checkpoint)
    }

    async fn save(&self, checkpoint: &BlockId) -> Result<()> {
        static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(Error::checkpoint)?;

        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("checkpoint.json");
        let suffix = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(".{file_name}.tmp-{}-{suffix}", std::process::id()));
        let bytes = serde_json::to_vec_pretty(checkpoint).map_err(Error::checkpoint)?;

        tokio::fs::write(&temp_path, bytes)
            .await
            .map_err(Error::checkpoint)?;
        if let Err(error) = tokio::fs::rename(&temp_path, &self.path).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(Error::checkpoint(error));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hash256;

    #[tokio::test]
    async fn file_store_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let store = FileCheckpointStore::new(directory.path().join("nested/checkpoint.json"));
        let checkpoint = BlockId {
            workchain: BlockId::MASTERCHAIN_WORKCHAIN,
            shard: BlockId::FULL_SHARD,
            seqno: 42,
            root_hash: Hash256::new([1; 32]),
            file_hash: Hash256::new([2; 32]),
        };

        assert_eq!(store.load().await.unwrap(), None);
        store.save(&checkpoint).await.unwrap();
        assert_eq!(store.load().await.unwrap(), Some(checkpoint));
    }
}
