//! Embedded, checkpointed metrics indexing for a Localton network.

pub mod session_stats;

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use rusqlite::{Connection, params};
use serde::Serialize;
use thiserror::Error;
use tokio::sync::watch;
use ton_indexer_core::{
    Batch, BlockId, CheckpointStore, Error as IndexerError, IndexPipeline, Sink,
};
use ton_indexer_liteserver::{CanonicalBlockSource, TonutilsLiteClient};
use tracing::{info, warn};

// Covers at least fifteen minutes at Localton's fastest supported 400 ms cadence.
const INITIAL_LOOKBACK_BLOCKS: u32 = 2_700;
const MAX_BATCHES_PER_TICK: usize = 64;
const RETAIN_SECONDS: u64 = 24 * 60 * 60;
const RETRY_DELAY: Duration = Duration::from_secs(2);
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const UNKNOWN_QUEUE_SIZE: u64 = u64::MAX;

/// One time bucket returned to dashboard consumers.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TpsPoint {
    /// Unix timestamp at the beginning of the bucket.
    pub timestamp: u64,
    /// Transactions committed by all masterchain and shard blocks in the bucket.
    pub transactions: u64,
    /// Average transactions per second across the bucket duration.
    pub tps: f64,
    /// Masterchain blocks produced during the bucket.
    pub masterchain_blocks: u64,
    /// Mean interval between masterchain blocks observed in the complete bucket.
    ///
    /// `None` means that the bucket has no interval sample: either it contains no
    /// blocks or its only block has no retained predecessor. The source `gen_utime`
    /// field has second precision, so sub-second intervals become accurate only
    /// when averaged across several blocks.
    pub block_time_ms: Option<f64>,
}

/// Recent transaction throughput and block timing with indexed chain-time bounds.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TpsSeries {
    /// Duration represented by each point.
    pub bucket_seconds: u64,
    /// Oldest block time included in the response.
    pub indexed_from: Option<u64>,
    /// Newest block time included in the response.
    pub indexed_to: Option<u64>,
    /// Messages currently waiting in shard outbound queues.
    pub queue_size: Option<u64>,
    /// Chronologically ordered throughput buckets.
    pub points: Vec<TpsPoint>,
}

/// Persistent SQLite sink shared by the indexing task and dashboard queries.
///
/// Commits are idempotent by masterchain sequence number. The checkpoint is
/// intentionally stored in the same database, so a process restart resumes at
/// the first batch that has not been fully recorded.
#[derive(Clone)]
pub struct TpsStore {
    connection: Arc<Mutex<Connection>>,
    queue_size: Arc<AtomicU64>,
}

impl TpsStore {
    /// Opens the metrics database and creates its schema when necessary.
    ///
    /// # Errors
    ///
    /// Returns an error when SQLite cannot open or initialize the database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS indexer_checkpoint (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tps_batches (
                masterchain_seqno INTEGER PRIMARY KEY,
                block_time INTEGER NOT NULL,
                transactions INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS tps_batches_by_time
                ON tps_batches (block_time);",
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            queue_size: Arc::new(AtomicU64::new(UNKNOWN_QUEUE_SIZE)),
        })
    }

    /// Aggregates the most recent chain-time window into fixed-duration buckets.
    ///
    /// The window is anchored to the latest indexed block rather than the host
    /// clock because Localton networks may use accelerated or restored chain time.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be queried.
    pub fn recent_tps(
        &self,
        window_seconds: u64,
        bucket_seconds: u64,
    ) -> Result<TpsSeries, StoreError> {
        let bucket_seconds = bucket_seconds.max(1);
        let connection = self.connection()?;
        let (indexed_from, indexed_to): (Option<i64>, Option<i64>) = connection.query_row(
            "SELECT MIN(block_time), MAX(block_time) FROM tps_batches",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let (Some(indexed_from), Some(indexed_to)) = (indexed_from, indexed_to) else {
            return Ok(TpsSeries {
                bucket_seconds,
                indexed_from: None,
                indexed_to: None,
                queue_size: self.queue_size(),
                points: Vec::new(),
            });
        };

        let query_from = indexed_to
            .saturating_sub(i64::try_from(window_seconds).unwrap_or(i64::MAX))
            .max(indexed_from);
        let bucket = i64::try_from(bucket_seconds).unwrap_or(i64::MAX);
        let complete_until = indexed_to - indexed_to.rem_euclid(bucket);
        let first_complete_bucket = match query_from.rem_euclid(bucket) {
            0 => query_from,
            remainder => query_from.saturating_add(bucket - remainder),
        };

        // Exclude the bucket containing the latest block. Dividing an incomplete
        // bucket by its full duration would make its rates look too low. The oldest
        // partial bucket is excluded for the same reason.
        let mut statement = connection.prepare(
            "WITH samples AS (
                 SELECT block_time,
                        transactions,
                        (block_time - LAG(block_time) OVER (ORDER BY masterchain_seqno)) * 1000.0
                            AS block_time_ms
                 FROM tps_batches
             )
             SELECT (block_time / ?1) * ?1 AS bucket_start,
                    SUM(transactions),
                    COUNT(*),
                    AVG(block_time_ms)
             FROM samples
             WHERE block_time >= ?2 AND block_time < ?3
             GROUP BY bucket_start
             ORDER BY bucket_start",
        )?;
        let rows = statement.query_map(
            params![bucket, first_complete_bucket, complete_until],
            |row| {
                let timestamp: i64 = row.get(0)?;
                let transactions: i64 = row.get(1)?;
                let masterchain_blocks: i64 = row.get(2)?;
                let block_time_ms: Option<f64> = row.get(3)?;
                Ok((timestamp, transactions, masterchain_blocks, block_time_ms))
            },
        )?;
        let mut aggregates = rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .peekable();
        let mut timestamp = first_complete_bucket;
        let mut points = Vec::new();

        while timestamp < complete_until {
            let (transactions, masterchain_blocks, block_time_ms) = match aggregates.peek() {
                Some((bucket_start, _, _, _)) if *bucket_start == timestamp => {
                    let (_, transactions, masterchain_blocks, block_time_ms) = aggregates
                        .next()
                        .expect("the matching aggregate was inspected");
                    (transactions, masterchain_blocks, block_time_ms)
                }
                _ => (0, 0, None),
            };
            points.push(TpsPoint {
                timestamp: u64::try_from(timestamp).unwrap_or_default(),
                transactions: u64::try_from(transactions).unwrap_or_default(),
                tps: transactions as f64 / bucket_seconds as f64,
                masterchain_blocks: u64::try_from(masterchain_blocks).unwrap_or_default(),
                block_time_ms,
            });

            let next = timestamp.saturating_add(bucket);
            if next == timestamp {
                break;
            }
            timestamp = next;
        }

        Ok(TpsSeries {
            bucket_seconds,
            indexed_from: u64::try_from(first_complete_bucket).ok(),
            indexed_to: u64::try_from(indexed_to).ok(),
            queue_size: self.queue_size(),
            points,
        })
    }

    fn queue_size(&self) -> Option<u64> {
        match self.queue_size.load(Ordering::Relaxed) {
            UNKNOWN_QUEUE_SIZE => None,
            size => Some(size),
        }
    }

    fn record_queue_size(&self, size: Option<u64>) {
        self.queue_size
            .store(size.unwrap_or(UNKNOWN_QUEUE_SIZE), Ordering::Relaxed);
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.connection.lock().map_err(|_| StoreError::Poisoned)
    }

    fn record_batch(&self, batch: &Batch) -> Result<(), StoreError> {
        let masterchain_seqno = i64::from(batch.masterchain().id().seqno);
        let block_time = i64::from(batch.masterchain().info().gen_utime);

        // A canonical batch owns the masterchain block and every new shard block,
        // so summing the whole batch measures network TPS without double-counting.
        let transactions = batch
            .blocks()
            .map(|block| block.transactions().len() as u64)
            .sum::<u64>();
        self.record_sample(masterchain_seqno, block_time, transactions)
    }

    fn record_sample(
        &self,
        masterchain_seqno: i64,
        block_time: i64,
        transactions: u64,
    ) -> Result<(), StoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;

        transaction.execute(
            "INSERT INTO tps_batches (masterchain_seqno, block_time, transactions)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(masterchain_seqno) DO UPDATE SET
                block_time = excluded.block_time,
                transactions = excluded.transactions",
            params![
                masterchain_seqno,
                block_time,
                i64::try_from(transactions).unwrap_or(i64::MAX)
            ],
        )?;

        // Dashboard queries only use rolling windows. Pruning after one day keeps
        // storage bounded independently of how long the network remains running.
        transaction.execute(
            "DELETE FROM tps_batches WHERE block_time < ?1",
            [block_time.saturating_sub(i64::try_from(RETAIN_SECONDS).unwrap_or(i64::MAX))],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn reset(&self) -> Result<(), StoreError> {
        self.connection()?.execute_batch(
            "DELETE FROM indexer_checkpoint;
             DELETE FROM tps_batches;",
        )?;
        Ok(())
    }
}

#[async_trait]
impl Sink for TpsStore {
    async fn commit(&mut self, batch: &Batch) -> ton_indexer_core::Result<()> {
        self.record_batch(batch).map_err(IndexerError::sink)
    }
}

#[async_trait]
impl CheckpointStore for TpsStore {
    async fn load(&self) -> ton_indexer_core::Result<Option<BlockId>> {
        let connection = self.connection().map_err(IndexerError::checkpoint)?;
        let mut statement = connection
            .prepare("SELECT value FROM indexer_checkpoint WHERE id = 1")
            .map_err(IndexerError::checkpoint)?;
        let mut rows = statement.query([]).map_err(IndexerError::checkpoint)?;
        let Some(row) = rows.next().map_err(IndexerError::checkpoint)? else {
            return Ok(None);
        };
        let value: String = row.get(0).map_err(IndexerError::checkpoint)?;
        serde_json::from_str(&value)
            .map(Some)
            .map_err(IndexerError::checkpoint)
    }

    async fn save(&self, checkpoint: &BlockId) -> ton_indexer_core::Result<()> {
        let value = serde_json::to_string(checkpoint).map_err(IndexerError::checkpoint)?;
        self.connection()
            .map_err(IndexerError::checkpoint)?
            .execute(
                "INSERT INTO indexer_checkpoint (id, value) VALUES (1, ?1)
                 ON CONFLICT(id) DO UPDATE SET value = excluded.value",
                [value],
            )
            .map_err(IndexerError::checkpoint)?;
        Ok(())
    }
}

/// Runs the TPS indexer until the shared Localton shutdown signal is raised.
///
/// Transport failures end the current liteserver session and are retried after
/// a bounded delay. A checkpoint ahead of the current chain means the network
/// was recreated in the same state directory, so metrics are reset before the
/// new chain is indexed.
pub async fn run(
    global_config: PathBuf,
    node: String,
    store: TpsStore,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }

        let started_at = Instant::now();
        let outcome = run_session(&global_config, &node, &store, &mut shutdown, started_at).await;

        if *shutdown.borrow() {
            info!(
                operation = "index_tps",
                node,
                target = %global_config.display(),
                duration_ms = started_at.elapsed().as_millis(),
                outcome = "stopped",
                "TPS indexing stopped"
            );
            return;
        }

        if let Err(error) = outcome {
            warn!(
                operation = "index_tps",
                node,
                target = %global_config.display(),
                duration_ms = started_at.elapsed().as_millis(),
                outcome = "error",
                error = %error,
                "TPS indexing session failed"
            );
        }

        tokio::select! {
            _ = tokio::time::sleep(RETRY_DELAY) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

async fn run_session(
    global_config: &Path,
    node: &str,
    store: &TpsStore,
    shutdown: &mut watch::Receiver<bool>,
    started_at: Instant,
) -> Result<(), StoreError> {
    let mut client = TonutilsLiteClient::connect_path(global_config)
        .await
        .map_err(StoreError::source)?;
    let latest = client.latest().await.map_err(StoreError::source)?;

    let mut checkpoint = CheckpointStore::load(store)
        .await
        .map_err(StoreError::source)?;

    // Reusing a state directory after recreating the network can leave a
    // checkpoint ahead of the new chain. Those samples belong to the old chain.
    if checkpoint.is_some_and(|checkpoint| checkpoint.seqno > latest.seqno) {
        store.reset()?;
        checkpoint = None;
    }

    // A fresh metrics database only needs enough history to fill the rolling
    // dashboard window. CanonicalBlockSource also loads the predecessor of its
    // first block, so seqno two is the earliest valid start: seqno one is its
    // block predecessor and seqno zero remains the non-downloadable zerostate.
    let start_seqno = checkpoint.map_or_else(
        || latest.seqno.saturating_sub(INITIAL_LOOKBACK_BLOCKS).max(2),
        |checkpoint| checkpoint.seqno.saturating_add(1),
    );

    let source = CanonicalBlockSource::new(client, start_seqno);
    let mut pipeline = IndexPipeline::new(source, store.clone(), store.clone());
    let mut interval = tokio::time::interval(POLL_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut total_committed = 0usize;
    let mut initial_window_complete = false;
    let mut queue_error_reported = false;

    store.record_queue_size(None);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let queue_started_at = Instant::now();

                match pipeline
                    .source_mut()
                    .client_mut()
                    .out_msg_queue_size()
                    .await
                {
                    Ok(size) => {
                        store.record_queue_size(Some(size));
                        queue_error_reported = false;
                    }
                    Err(error) => {
                        store.record_queue_size(None);

                        if !queue_error_reported {
                            warn!(
                                operation = "index_queue_size",
                                node,
                                target = %global_config.display(),
                                duration_ms = queue_started_at.elapsed().as_millis(),
                                outcome = "unavailable",
                                error = %error,
                                "outbound message queue observation failed"
                            );
                            queue_error_reported = true;
                        }
                    }
                }

                let committed = pipeline
                    .run_until_idle(MAX_BATCHES_PER_TICK)
                    .await
                    .map_err(StoreError::source)?;

                total_committed = total_committed.saturating_add(committed);

                if committed == 0 && !initial_window_complete {
                    initial_window_complete = true;
                    info!(
                        operation = "index_tps",
                        node,
                        target = %global_config.display(),
                        duration_ms = started_at.elapsed().as_millis(),
                        outcome = "ready",
                        batches = total_committed,
                        "TPS index is ready"
                    );
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
        }
    }
}

/// Errors raised by the Localton metrics store or its indexing source.
#[derive(Debug, Error)]
pub enum StoreError {
    /// SQLite could not open, mutate, or query the metrics database.
    #[error("SQLite metrics operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The shared SQLite connection was poisoned by a panic.
    #[error("SQLite metrics connection is unavailable")]
    Poisoned,
    /// The TON block source or checkpoint pipeline failed.
    #[error("TON indexing failed: {0}")]
    Source(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl StoreError {
    fn source(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Source(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use ton_indexer_core::Hash256;

    use super::*;

    #[test]
    fn aggregates_transactions_into_fixed_time_buckets() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let store = TpsStore::open(directory.path().join("metrics.sqlite3")).unwrap();
        store.record_sample(10, 100, 5).unwrap();
        store.record_sample(11, 102, 7).unwrap();
        store.record_sample(12, 108, 4).unwrap();
        store.record_sample(13, 111, 9).unwrap();
        store.record_queue_size(Some(23));

        expect![[r#"
            TpsSeries {
                bucket_seconds: 5,
                indexed_from: Some(
                    100,
                ),
                indexed_to: Some(
                    111,
                ),
                queue_size: Some(
                    23,
                ),
                points: [
                    TpsPoint {
                        timestamp: 100,
                        transactions: 12,
                        tps: 2.4,
                        masterchain_blocks: 2,
                        block_time_ms: Some(
                            2000.0,
                        ),
                    },
                    TpsPoint {
                        timestamp: 105,
                        transactions: 4,
                        tps: 0.8,
                        masterchain_blocks: 1,
                        block_time_ms: Some(
                            6000.0,
                        ),
                    },
                ],
            }
        "#]]
        .assert_debug_eq(&store.recent_tps(60, 5).unwrap());
    }

    #[test]
    fn preserves_empty_buckets_as_block_production_gaps() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let store = TpsStore::open(directory.path().join("metrics.sqlite3")).unwrap();
        store.record_sample(10, 100, 3).unwrap();
        store.record_sample(11, 111, 5).unwrap();

        expect![[r#"
            [
                TpsPoint {
                    timestamp: 100,
                    transactions: 3,
                    tps: 0.6,
                    masterchain_blocks: 1,
                    block_time_ms: None,
                },
                TpsPoint {
                    timestamp: 105,
                    transactions: 0,
                    tps: 0.0,
                    masterchain_blocks: 0,
                    block_time_ms: None,
                },
            ]
        "#]]
        .assert_debug_eq(&store.recent_tps(60, 5).unwrap().points);
    }

    #[test]
    fn excludes_partial_buckets_at_both_window_edges() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let store = TpsStore::open(directory.path().join("metrics.sqlite3")).unwrap();
        store.record_sample(1, 100, 1).unwrap();
        store.record_sample(2, 101, 2).unwrap();
        store.record_sample(3, 105, 3).unwrap();
        store.record_sample(4, 106, 4).unwrap();
        store.record_sample(5, 110, 5).unwrap();
        store.record_sample(6, 111, 6).unwrap();

        expect![[r#"
            [
                TpsPoint {
                    timestamp: 105,
                    transactions: 7,
                    tps: 1.4,
                    masterchain_blocks: 2,
                    block_time_ms: Some(
                        2500.0,
                    ),
                },
            ]
        "#]]
        .assert_debug_eq(&store.recent_tps(10, 5).unwrap().points);
    }

    #[tokio::test]
    async fn persists_full_checkpoint_identity() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let store = TpsStore::open(directory.path().join("metrics.sqlite3")).unwrap();
        let checkpoint = BlockId {
            workchain: -1,
            shard: BlockId::FULL_SHARD,
            seqno: 42,
            root_hash: Hash256::new([0x11; 32]),
            file_hash: Hash256::new([0x22; 32]),
        };
        store.save(&checkpoint).await.unwrap();

        expect![[r#"
            Some(
                BlockId {
                    workchain: -1,
                    shard: 9223372036854775808,
                    seqno: 42,
                    root_hash: 1111111111111111111111111111111111111111111111111111111111111111,
                    file_hash: 2222222222222222222222222222222222222222222222222222222222222222,
                },
            )
        "#]]
        .assert_debug_eq(&store.load().await.unwrap());
    }
}
