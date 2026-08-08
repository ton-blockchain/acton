//! Sequential checkpointed pipeline orchestration.

use crate::{BlockId, BlockSource, CheckpointStore, Error, Result, Sink};

/// Result of one pipeline iteration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunOutcome {
    /// No new canonical masterchain block is currently available.
    Idle,
    /// A batch was committed and the checkpoint advanced.
    Committed(BlockId),
}

/// At-least-once source → sink → checkpoint pipeline.
pub struct IndexPipeline<S, K, C> {
    source: S,
    sink: K,
    checkpoints: C,
}

impl<S, K, C> IndexPipeline<S, K, C> {
    /// Creates a pipeline.
    pub const fn new(source: S, sink: K, checkpoints: C) -> Self {
        Self {
            source,
            sink,
            checkpoints,
        }
    }

    /// Returns a shared reference to the sink.
    pub const fn sink(&self) -> &K {
        &self.sink
    }
}

impl<S, K, C> IndexPipeline<S, K, C>
where
    S: BlockSource,
    K: Sink,
    C: CheckpointStore,
{
    /// Processes at most one batch.
    ///
    /// # Errors
    ///
    /// Returns an error from the source, sink, or checkpoint store, or when
    /// the source skips a masterchain sequence number.
    pub async fn run_once(&mut self) -> Result<RunOutcome> {
        let previous = self.checkpoints.load().await?;
        let Some(batch) = self.source.next_batch(previous.as_ref()).await? else {
            return Ok(RunOutcome::Idle);
        };
        let next = batch.checkpoint();

        if let Some(previous) = previous
            && next.seqno != previous.seqno.saturating_add(1)
        {
            return Err(Error::Invariant(format!(
                "source returned masterchain seqno {} after checkpoint {}",
                next.seqno, previous.seqno
            )));
        }

        self.sink.commit(&batch).await?;
        self.checkpoints.save(&next).await?;

        Ok(RunOutcome::Committed(next))
    }

    /// Runs until the source is idle or `max_batches` have committed.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by [`Self::run_once`].
    pub async fn run_until_idle(&mut self, max_batches: usize) -> Result<usize> {
        let mut committed = 0;
        while committed < max_batches {
            match self.run_once().await? {
                RunOutcome::Idle => break,
                RunOutcome::Committed(_) => committed += 1,
            }
        }
        Ok(committed)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use async_trait::async_trait;

    use super::*;
    use crate::{Batch, MemoryCheckpointStore, block::test_batch};

    struct QueueSource(VecDeque<Batch>);

    #[async_trait]
    impl BlockSource for QueueSource {
        async fn next_batch(&mut self, _after: Option<&BlockId>) -> Result<Option<Batch>> {
            Ok(self.0.pop_front())
        }
    }

    #[derive(Default)]
    struct CollectSink(Vec<u32>);

    #[async_trait]
    impl Sink for CollectSink {
        async fn commit(&mut self, batch: &Batch) -> Result<()> {
            self.0.push(batch.masterchain().id().seqno);
            Ok(())
        }
    }

    struct FailingSink;

    #[async_trait]
    impl Sink for FailingSink {
        async fn commit(&mut self, _batch: &Batch) -> Result<()> {
            Err(Error::sink(std::io::Error::other("expected failure")))
        }
    }

    #[tokio::test]
    async fn advances_checkpoint_after_commit() {
        let checkpoints = MemoryCheckpointStore::default();
        let mut pipeline = IndexPipeline::new(
            QueueSource(VecDeque::from([test_batch(1), test_batch(2)])),
            CollectSink::default(),
            checkpoints.clone(),
        );

        assert_eq!(pipeline.run_until_idle(10).await.unwrap(), 2);
        assert_eq!(checkpoints.load().await.unwrap().unwrap().seqno, 2);
        assert_eq!(pipeline.sink().0, vec![1, 2]);
    }

    #[tokio::test]
    async fn does_not_checkpoint_failed_commit() {
        let checkpoints = MemoryCheckpointStore::default();
        let mut pipeline = IndexPipeline::new(
            QueueSource(VecDeque::from([test_batch(1)])),
            FailingSink,
            checkpoints.clone(),
        );

        assert!(pipeline.run_once().await.is_err());
        assert_eq!(checkpoints.load().await.unwrap(), None);
    }
}
