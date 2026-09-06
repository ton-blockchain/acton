//! Validator-engine session-log ingestion and chart-ready aggregation.
//!
//! Event correlation and metric names preserve the validator-engine session-log
//! schema while keeping aggregated data inside the node-owned observability database.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, Transaction, params};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;
use tokio::{sync::watch, time::MissedTickBehavior};
use tracing::{info, warn};

const IMPORT_INTERVAL: Duration = Duration::from_secs(60);
const IMPORT_LAG_SECONDS: u64 = 15;
const SOURCE_LABEL_FIELD: &str = "_session_stats_source";

/// One persisted metric bucket before a chart selects average, rate, or sum.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionStatBucket {
    /// Metric name used by the upstream Session Stats chart configuration.
    pub stat: String,
    /// TON workchain ID (`-1` for masterchain and `0` for base workchain).
    pub workchain: i32,
    /// Unix timestamp at the beginning of the returned aggregation window.
    pub timestamp: u64,
    /// Number of samples represented by this bucket.
    pub count: u64,
    /// Sum of all samples represented by this bucket.
    pub sum: f64,
    /// Minimum sample in this bucket.
    pub min: f64,
    /// Maximum sample in this bucket.
    pub max: f64,
}

/// Complete chart input for one requested wall-clock range.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessionStatsSnapshot {
    /// Effective duration of each bucket after the upstream 1,000-point cap.
    pub bucket_seconds: u64,
    /// Earliest persisted sample timestamp, independent of the requested range.
    pub indexed_from: Option<u64>,
    /// Latest persisted sample timestamp, independent of the requested range.
    pub indexed_to: Option<u64>,
    /// Node labels available for source-specific collate-time charts.
    pub sources: Vec<String>,
    /// Aggregated metric rows used to construct the upstream chart set.
    pub buckets: Vec<SessionStatBucket>,
}

/// Result of one idempotent import pass, used by structured progress logs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionStatsImport {
    /// Whether the validator session log existed during this import pass.
    pub file_found: bool,
    /// Number of complete supported events read from the active log.
    pub events: usize,
    /// Number of newly persisted collated blocks.
    pub collated: usize,
    /// Number of newly persisted validated blocks.
    pub validated: usize,
    /// Number of newly persisted applied blocks.
    pub applied: usize,
    /// Number of newly persisted externally applied blocks.
    pub externally_applied: usize,
}

/// SQLite-backed session metric store shared by the importer and HTTP queries.
///
/// Block identities and node identities form the idempotency boundary. The
/// importer can therefore reread the active log on every pass, matching the
/// upstream lookback model without double-counting committed events.
#[derive(Clone)]
pub struct SessionStatsStore {
    connection: Arc<Mutex<Connection>>,
    source: String,
}

impl SessionStatsStore {
    /// Opens the session statistics database and creates the upstream schema.
    ///
    /// # Errors
    ///
    /// Returns an error when SQLite cannot open or initialize the database.
    pub fn open(
        path: impl AsRef<Path>,
        source: impl Into<String>,
    ) -> Result<Self, SessionStatsError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(2))?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE IF NOT EXISTS processed_blocks_from (
                workchain INTEGER NOT NULL,
                shard INTEGER NOT NULL,
                seqno INTEGER NOT NULL,
                root_hash TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                self TEXT NOT NULL,
                PRIMARY KEY (workchain, shard, seqno, root_hash, file_hash, self)
            );
            CREATE TABLE IF NOT EXISTS processed_blocks_applied (
                workchain INTEGER NOT NULL,
                shard INTEGER NOT NULL,
                seqno INTEGER NOT NULL,
                root_hash TEXT NOT NULL,
                file_hash TEXT NOT NULL,
                PRIMARY KEY (workchain, shard, seqno, root_hash, file_hash)
            );
            CREATE TABLE IF NOT EXISTS data (
                stat_id INTEGER NOT NULL,
                workchain INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                v_count INTEGER NOT NULL DEFAULT 0,
                v_sum REAL NOT NULL DEFAULT 0,
                v_min REAL NOT NULL DEFAULT 1e18,
                v_max REAL NOT NULL DEFAULT -1e18,
                FOREIGN KEY (stat_id) REFERENCES stats(id),
                PRIMARY KEY (stat_id, workchain, timestamp)
            );
            CREATE INDEX IF NOT EXISTS session_data_by_timestamp
                ON data (stat_id, workchain, timestamp);",
        )?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            source: source.into(),
        })
    }

    /// Imports every complete JSON event older than `max_timestamp`.
    ///
    /// Invalid JSON lines are ignored because validator-engine may be writing the
    /// final line while the importer reads the file. Metric extraction and the
    /// accepted-block fallback preserve the validator-engine event relationships.
    ///
    /// # Errors
    ///
    /// Returns an error for filesystem failures other than a missing source file,
    /// malformed supported events, or SQLite failures.
    pub fn import_file(
        &self,
        path: &Path,
        max_timestamp: u64,
        private_network: bool,
    ) -> Result<SessionStatsImport, SessionStatsError> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(SessionStatsImport::default());
            }
            Err(source) => {
                return Err(SessionStatsError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let mut events = Vec::new();

        for raw_line in contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Ok(mut value) = serde_json::from_str::<Value>(raw_line) else {
                continue;
            };

            if event_type(&value) == Some("consensus.stats.events") {
                let session_id = value.get("id").cloned().unwrap_or(Value::Null);
                let Some(wrapped_events) = value.get_mut("events").and_then(Value::as_array_mut)
                else {
                    continue;
                };
                for wrapped in wrapped_events {
                    let Some(mut event) = wrapped.get("event").cloned() else {
                        continue;
                    };
                    let Some(object) = event.as_object_mut() else {
                        continue;
                    };
                    object.insert(
                        "timestamp".to_owned(),
                        wrapped.get("ts").cloned().unwrap_or(Value::Null),
                    );
                    object.insert("session_id".to_owned(), session_id.clone());
                    if event_timestamp(&event)
                        .is_some_and(|timestamp| timestamp < max_timestamp as f64)
                    {
                        events.push(event);
                    }
                }
            } else if event_timestamp(&value)
                .is_some_and(|timestamp| timestamp < max_timestamp as f64)
            {
                events.push(value);
            }
        }

        for event in &mut events {
            let Some(object) = event.as_object_mut() else {
                continue;
            };

            // One store owns one validator log, so the configured source is the
            // canonical node identity. This is the upstream `override_self`
            // mode and lets nested consensus events correlate with validator
            // events that carry a public-key-shaped `self` value.
            object.insert("self".to_owned(), Value::String(self.source.clone()));
            object.insert(
                SOURCE_LABEL_FIELD.to_owned(),
                Value::String(self.source.clone()),
            );
        }
        events.sort_by(|left, right| {
            event_timestamp(left)
                .unwrap_or_default()
                .total_cmp(&event_timestamp(right).unwrap_or_default())
        });

        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let mut importer = Importer::new(transaction, private_network);
        let mut summary = importer.process(&events)?;
        importer.commit()?;
        summary.file_found = true;
        summary.events = events.len();

        Ok(summary)
    }

    /// Reads chart aggregates for the requested range.
    ///
    /// The effective window uses the same formula as the upstream Flask API: a
    /// selected resolution expands only when the range would exceed roughly one
    /// thousand points.
    ///
    /// # Errors
    ///
    /// Returns an error when SQLite cannot read the persisted aggregates.
    pub fn snapshot(
        &self,
        start: u64,
        end: u64,
        requested_window_seconds: u64,
    ) -> Result<SessionStatsSnapshot, SessionStatsError> {
        let requested_window_seconds = requested_window_seconds.max(60);
        let range = end.saturating_sub(start);
        let bucket_seconds = (range / requested_window_seconds.saturating_mul(1_000) + 1)
            .saturating_mul(requested_window_seconds);
        let bucket = i64::try_from(bucket_seconds).unwrap_or(i64::MAX);
        let connection = self.connection()?;
        let (indexed_from, indexed_to): (Option<i64>, Option<i64>) = connection.query_row(
            "SELECT MIN(timestamp), MAX(timestamp) FROM data",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut statement = connection.prepare(
            "SELECT stats.name,
                    data.workchain,
                    (data.timestamp / ?1) * ?1 AS bucket_start,
                    SUM(data.v_count),
                    SUM(data.v_sum),
                    MIN(data.v_min),
                    MAX(data.v_max)
             FROM data
             JOIN stats ON stats.id = data.stat_id
             WHERE data.timestamp BETWEEN ?2 AND ?3
             GROUP BY stats.name, data.workchain, bucket_start
             ORDER BY bucket_start, stats.name, data.workchain",
        )?;
        let rows = statement.query_map(
            params![
                bucket,
                i64::try_from(start).unwrap_or(i64::MAX),
                i64::try_from(end).unwrap_or(i64::MAX)
            ],
            |row| {
                let timestamp: i64 = row.get(2)?;
                let count: i64 = row.get(3)?;
                Ok(SessionStatBucket {
                    stat: row.get(0)?,
                    workchain: row.get(1)?,
                    timestamp: u64::try_from(timestamp).unwrap_or_default(),
                    count: u64::try_from(count).unwrap_or_default(),
                    sum: row.get(4)?,
                    min: row.get(5)?,
                    max: row.get(6)?,
                })
            },
        )?;
        let buckets = rows.collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SessionStatsSnapshot {
            bucket_seconds,
            indexed_from: indexed_from.and_then(|value| u64::try_from(value).ok()),
            indexed_to: indexed_to.and_then(|value| u64::try_from(value).ok()),
            sources: vec![self.source.clone()],
            buckets,
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, SessionStatsError> {
        self.connection
            .lock()
            .map_err(|_| SessionStatsError::Poisoned)
    }
}

/// Runs the upstream 60-second importer lifecycle for one validator-engine log.
///
/// The task stops when the node-owned shutdown channel closes or becomes true.
/// Each pass leaves diagnostic context in structured logs without serializing
/// session payloads or validator keys.
pub async fn run_session_stats(
    log_path: PathBuf,
    node_name: String,
    private_network: bool,
    store: SessionStatsStore,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut interval = tokio::time::interval(IMPORT_INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    break;
                }
                continue;
            }
        }

        let started = Instant::now();
        let max_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(IMPORT_LAG_SECONDS);
        let worker_store = store.clone();
        let worker_path = log_path.clone();
        let result = tokio::task::spawn_blocking(move || {
            worker_store.import_file(&worker_path, max_timestamp, private_network)
        })
        .await;

        match result {
            Ok(Ok(summary)) if summary.file_found => info!(
                operation = "session_stats_import",
                node = node_name,
                target = %log_path.display(),
                duration_ms = started.elapsed().as_millis(),
                outcome = "success",
                events = summary.events,
                collated = summary.collated,
                validated = summary.validated,
                applied = summary.applied,
                externally_applied = summary.externally_applied,
                "validator session statistics imported"
            ),
            Ok(Ok(_)) => info!(
                operation = "session_stats_import",
                node = node_name,
                target = %log_path.display(),
                duration_ms = started.elapsed().as_millis(),
                outcome = "waiting",
                "validator session log is not available yet"
            ),
            Ok(Err(error)) => warn!(
                operation = "session_stats_import",
                node = node_name,
                target = %log_path.display(),
                duration_ms = started.elapsed().as_millis(),
                outcome = "failed",
                error = %error,
                "validator session statistics import failed"
            ),
            Err(error) => warn!(
                operation = "session_stats_import",
                node = node_name,
                target = %log_path.display(),
                duration_ms = started.elapsed().as_millis(),
                outcome = "failed",
                error = %error,
                "validator session statistics worker stopped unexpectedly"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct BlockId {
    workchain: i32,
    shard: i64,
    seqno: i64,
    root_hash: String,
    file_hash: String,
}

struct Importer<'connection> {
    transaction: Transaction<'connection>,
    stat_ids: HashMap<String, i64>,
    private_network: bool,
    summary: SessionStatsImport,
}

impl<'connection> Importer<'connection> {
    fn new(transaction: Transaction<'connection>, private_network: bool) -> Self {
        Self {
            transaction,
            stat_ids: HashMap::new(),
            private_network,
            summary: SessionStatsImport::default(),
        }
    }

    fn process(&mut self, events: &[Value]) -> Result<SessionStatsImport, SessionStatsError> {
        let mut hash_to_block_id = HashMap::<String, BlockId>::new();
        let mut collates = HashMap::<BlockId, Value>::new();
        let mut validates = HashMap::<(String, BlockId), Value>::new();
        let mut direct_collates = Vec::new();
        let mut direct_validates = Vec::new();

        for event in events {
            let Some(node) = string_field(event, "self") else {
                continue;
            };
            match event_type(event) {
                Some("validatorStats.appliedBlockStats") => self.process_external_applied(event)?,
                Some("consensus.stats.candidateReceived") => {
                    let Some(hash) = event.pointer("/id/hash").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(block) = event.pointer("/block/id") else {
                        continue;
                    };
                    hash_to_block_id.insert(hash.to_owned(), parse_block_id(block)?);
                }
                Some("validatorStats.collatedBlock") => {
                    let block_id = parse_block_id(required_field(event, "block_id")?)?;
                    collates.insert(block_id, event.clone());
                    direct_collates.push(event.clone());
                }
                Some("validatorStats.validatedBlock") => {
                    let block_id = parse_block_id(required_field(event, "block_id")?)?;
                    validates.insert((node.to_owned(), block_id), event.clone());
                    direct_validates.push(event.clone());
                }
                Some("validatorStats.collatorNodeResponse") => {
                    let new_block_id = parse_block_id(required_field(event, "block_id")?)?;
                    let old_block_id = parse_block_id(required_field(event, "original_block_id")?)?;
                    if let Some(collate) = collates.get(&old_block_id).cloned() {
                        collates.insert(new_block_id, collate);
                    }
                }
                _ => {}
            }
        }

        let mut new_applied_blocks = HashMap::<BlockId, (i64, f64)>::new();
        let mut accepted_ours = HashMap::<BlockId, Value>::new();
        let mut session_seqno_ours = HashMap::<(String, i64), BlockId>::new();
        let mut correlated_events = 0_u64;

        for event in events {
            if event_type(event) != Some("consensus.stats.blockAccepted") {
                continue;
            }
            let Some(node) = string_field(event, "self") else {
                continue;
            };
            let Some(hash) = event.pointer("/id/hash").and_then(Value::as_str) else {
                continue;
            };
            let Some(block_id) = hash_to_block_id.get(hash).cloned() else {
                continue;
            };
            let timestamp = numeric_field(event, "timestamp")?;
            let collate = collates
                .get(&block_id)
                .filter(|collate| string_field(collate, "self") == Some(node));
            let validate = validates.get(&(node.to_owned(), block_id.clone()));

            if collate.is_some() || validate.is_some() {
                correlated_events += 1;
            }

            if self.try_add_block_from(&block_id, node)? {
                if let Some(collate) = collate {
                    self.process_collate(timestamp, collate)?;
                } else if let Some(validate) = validate {
                    self.process_validate(timestamp, validate)?;
                }
            }

            if self.private_network
                && let Some(collate) = collate
            {
                if block_id.workchain == -1 {
                    new_applied_blocks.insert(block_id.clone(), (block_id.seqno, timestamp));
                    if let Some(shards) = collate
                        .pointer("/block_stats/shard_configuration")
                        .and_then(Value::as_array)
                    {
                        for shard in shards {
                            let shard_id = parse_block_id(shard)?;
                            let candidate = (block_id.seqno, timestamp);
                            new_applied_blocks
                                .entry(shard_id)
                                .and_modify(|current| {
                                    if candidate.0 < current.0
                                        || (candidate.0 == current.0 && candidate.1 < current.1)
                                    {
                                        *current = candidate;
                                    }
                                })
                                .or_insert(candidate);
                        }
                    }
                }
                if !accepted_ours.contains_key(&block_id) {
                    accepted_ours.insert(block_id.clone(), event.clone());
                    if let Some(session_id) = event.get("session_id") {
                        session_seqno_ours
                            .insert((value_key(session_id), block_id.seqno), block_id.clone());
                    }
                }
            }
        }

        let mut applied = new_applied_blocks.into_iter().collect::<Vec<_>>();
        applied.sort_by(|left, right| {
            left.1
                .0
                .cmp(&right.1.0)
                .then_with(|| left.1.1.total_cmp(&right.1.1))
        });
        let mut visited = HashSet::new();
        for (block_id, (_, master_accepted_at)) in applied {
            let Some(accepted) = accepted_ours.get(&block_id) else {
                continue;
            };
            let Some(session_id) = accepted.get("session_id").map(value_key) else {
                continue;
            };
            let mut lineage = Vec::new();
            let mut current = Some(block_id);
            while let Some(block_id) = current {
                if !visited.insert(block_id.clone()) {
                    break;
                }
                current = session_seqno_ours
                    .get(&(session_id.clone(), block_id.seqno - 1))
                    .cloned();
                lineage.push(block_id);
            }
            for block_id in lineage.into_iter().rev() {
                if let (Some(collate), Some(accepted)) =
                    (collates.get(&block_id), accepted_ours.get(&block_id))
                {
                    self.process_applied(
                        numeric_field(accepted, "timestamp")?,
                        collate,
                        master_accepted_at,
                    )?;
                }
            }
        }

        if correlated_events == 0 && (!direct_collates.is_empty() || !direct_validates.is_empty()) {
            for collate in direct_collates {
                let block_id = parse_block_id(required_field(&collate, "block_id")?)?;
                let marker = format!(
                    "{}#direct",
                    string_field(&collate, "self").unwrap_or_default()
                );
                if self.try_add_block_from(&block_id, &marker)? {
                    if let Some(node) = string_field(&collate, "self") {
                        self.try_add_block_from(&block_id, node)?;
                    }
                    self.process_collate(required_timestamp(&collate)?, &collate)?;
                }
            }
            for validate in direct_validates {
                let block_id = parse_block_id(required_field(&validate, "block_id")?)?;
                let marker = format!(
                    "{}#direct-validate",
                    string_field(&validate, "self").unwrap_or_default()
                );
                if self.try_add_block_from(&block_id, &marker)? {
                    if let Some(node) = string_field(&validate, "self") {
                        self.try_add_block_from(&block_id, node)?;
                    }
                    self.process_validate(required_timestamp(&validate)?, &validate)?;
                }
            }
        }

        Ok(self.summary.clone())
    }

    fn commit(self) -> Result<(), SessionStatsError> {
        self.transaction.commit()?;
        Ok(())
    }

    fn process_external_applied(&mut self, event: &Value) -> Result<(), SessionStatsError> {
        let block_id = parse_block_id(required_field(event, "block_id")?)?;
        let timestamp = numeric_field(event, "block_timestamp")?;
        if self.try_add_applied_block(&block_id)? {
            self.add_stat(
                "EXT_transactions",
                block_id.workchain,
                timestamp,
                numeric_field(event, "transactions")?,
                None,
            )?;
            self.add_stat(
                "EXT_ext_msgs",
                block_id.workchain,
                timestamp,
                numeric_field(event, "ext_msgs")?,
                None,
            )?;
            self.summary.externally_applied += 1;
        }
        Ok(())
    }

    fn process_collate(&mut self, timestamp: f64, event: &Value) -> Result<(), SessionStatsError> {
        let block_id = parse_block_id(required_field(event, "block_id")?)?;
        let workchain = block_id.workchain;
        let block_limits = required_field(event, "block_limits")?;
        self.add_stat(
            "BLOCK_size",
            workchain,
            timestamp,
            numeric_field(event, "bytes")?,
            None,
        )?;
        self.add_stat(
            "BLOCK_collated_data_size",
            workchain,
            timestamp,
            numeric_field(event, "collated_data_bytes")?,
            None,
        )?;
        self.add_stat(
            "BLOCK_size_est",
            workchain,
            timestamp,
            numeric_field(block_limits, "bytes")?,
            None,
        )?;
        self.add_stat(
            "BLOCK_collated_data_size_est",
            workchain,
            timestamp,
            numeric_field(block_limits, "collated_data_bytes")?,
            None,
        )?;

        if let Some(block_stats) = event.get("block_stats") {
            self.add_stat(
                "BLOCK_transactions",
                workchain,
                timestamp,
                numeric_field(block_stats, "transactions")?,
                None,
            )?;
            if block_stats.get("new_out_msg_queue_size").is_some() {
                self.add_stat(
                    "BLOCK_msg_queue_size",
                    workchain,
                    timestamp,
                    numeric_field(block_stats, "new_out_msg_queue_size")?,
                    None,
                )?;
                self.add_stat(
                    "BLOCK_msg_queue_cleaned",
                    workchain,
                    timestamp,
                    numeric_field(block_stats, "msg_queue_cleaned")?,
                    None,
                )?;
                let neighbors = block_stats
                    .get("neighbors")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                self.add_stat(
                    "BLOCK_queue_total_processed",
                    workchain,
                    timestamp,
                    neighbors
                        .iter()
                        .map(|neighbor| numeric_field(neighbor, "processed_msgs"))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .sum(),
                    None,
                )?;
                self.add_stat(
                    "BLOCK_queue_total_skipped",
                    workchain,
                    timestamp,
                    neighbors
                        .iter()
                        .map(|neighbor| numeric_field(neighbor, "skipped_msgs"))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .sum(),
                    None,
                )?;
                self.add_stat(
                    "BLOCK_queue_limit_reached",
                    workchain,
                    timestamp,
                    f64::from(
                        neighbors
                            .iter()
                            .any(|neighbor| bool_field(neighbor, "limit_reached").unwrap_or(false)),
                    ),
                    None,
                )?;
                if let Some(maximum) = neighbors
                    .iter()
                    .filter_map(|neighbor| numeric_field(neighbor, "processed_msgs").ok())
                    .max_by(f64::total_cmp)
                {
                    self.add_stat(
                        "BLOCK_max_neighbor_processed",
                        workchain,
                        timestamp,
                        maximum,
                        None,
                    )?;
                }
                for neighbor in neighbors {
                    let limit = numeric_field(neighbor, "msg_limit")?;
                    if limit >= 0.0 {
                        self.add_stat(
                            "BLOCK_neighbor_msg_limit",
                            workchain,
                            timestamp,
                            limit,
                            None,
                        )?;
                    }
                }
            }
            if let Some(shards) = block_stats
                .get("shard_configuration")
                .and_then(Value::as_array)
            {
                self.add_stat(
                    "shards_count",
                    workchain,
                    timestamp,
                    shards.len() as f64,
                    None,
                )?;
            }
        }

        if block_limits.get("load_fraction_queue_cleanup").is_some() {
            for name in [
                "queue_cleanup",
                "dispatch",
                "internals",
                "externals",
                "new_msgs",
            ] {
                self.add_stat(
                    &format!("BLOCK_load_fraction_{name}"),
                    workchain,
                    timestamp,
                    numeric_field(block_limits, &format!("load_fraction_{name}"))?,
                    None,
                )?;
            }
        }

        let source = event.get(SOURCE_LABEL_FIELD).and_then(Value::as_str);
        let wait_externals = optional_numeric_field(event, "wait_externals_time").unwrap_or(0.0);
        let other_wait = numeric_field(event, "total_time")?
            - numeric_field(event, "work_time")?
            - wait_externals;
        self.add_stat(
            "BLOCK_collate_time_wait_externals",
            workchain,
            timestamp,
            wait_externals,
            source,
        )?;
        self.add_stat(
            "BLOCK_collate_time_other_wait",
            workchain,
            timestamp,
            other_wait,
            source,
        )?;
        for kind in ["real", "cpu"] {
            let stats = parse_work_time_stats(
                event
                    .get(format!("work_time_{kind}_stats"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            let mut other = 0.0;
            for (name, value) in stats {
                if name == "total" {
                    other += value;
                } else {
                    if !name.starts_with('*') {
                        other -= value;
                    }
                    self.add_stat(
                        &format!("BLOCK_collate_work_time_{kind}_{name}"),
                        workchain,
                        timestamp,
                        value,
                        source,
                    )?;
                }
            }
            self.add_stat(
                &format!("BLOCK_collate_work_time_{kind}_other"),
                workchain,
                timestamp,
                other,
                source,
            )?;
        }

        if let Some(cache) = event.get("storage_stat_cache").and_then(Value::as_object) {
            for (name, value) in cache {
                if name != "@type" {
                    self.add_stat(
                        &format!("BLOCK_collate_storage_stat_cache_{name}"),
                        workchain,
                        timestamp,
                        numeric_value(value).ok_or_else(|| invalid_field(name))?,
                        None,
                    )?;
                }
            }
        }
        self.summary.collated += 1;
        Ok(())
    }

    fn process_validate(&mut self, timestamp: f64, event: &Value) -> Result<(), SessionStatsError> {
        if !bool_field(event, "valid")? {
            return Ok(());
        }
        let block_id = parse_block_id(required_field(event, "block_id")?)?;
        let workchain = block_id.workchain;
        let source = event.get(SOURCE_LABEL_FIELD).and_then(Value::as_str);
        let other_wait = numeric_field(event, "total_time")? - numeric_field(event, "work_time")?;
        self.add_stat(
            "BLOCK_validate_time_other_wait",
            workchain,
            timestamp,
            other_wait,
            source,
        )?;

        for kind in ["real", "cpu"] {
            let stats = parse_work_time_stats(
                event
                    .get(format!("work_time_{kind}_stats"))
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            let mut other = 0.0;
            for (name, value) in stats {
                if name == "total" {
                    other += value;
                } else {
                    if !name.starts_with('*') {
                        other -= value;
                    }
                    self.add_stat(
                        &format!("BLOCK_validate_work_time_{kind}_{name}"),
                        workchain,
                        timestamp,
                        value,
                        source,
                    )?;
                }
            }
            self.add_stat(
                &format!("BLOCK_validate_work_time_{kind}_other"),
                workchain,
                timestamp,
                other,
                source,
            )?;
        }

        let actual_work_time = optional_numeric_field(event, "actual_time")
            .unwrap_or(numeric_field(event, "work_time")?);
        self.add_stat(
            "BLOCK_validate_actual_work_time",
            workchain,
            timestamp,
            actual_work_time,
            None,
        )?;
        let mode = if optional_bool_field(event, "parallel_accounts_validation").unwrap_or(false) {
            "parallel"
        } else {
            "singlethread"
        };
        self.add_stat(
            &format!("BLOCK_validate_actual_work_time_{mode}"),
            workchain,
            timestamp,
            actual_work_time,
            None,
        )?;
        self.summary.validated += 1;
        Ok(())
    }

    fn process_applied(
        &mut self,
        timestamp: f64,
        collate: &Value,
        master_accepted_at: f64,
    ) -> Result<(), SessionStatsError> {
        let block_id = parse_block_id(required_field(collate, "block_id")?)?;
        if !self.try_add_applied_block(&block_id)? {
            return Ok(());
        }
        self.add_stat(
            "BLOCK_APPLIED_blocks",
            block_id.workchain,
            timestamp,
            1.0,
            None,
        )?;
        if let Some(block_stats) = collate.get("block_stats") {
            self.add_stat(
                "BLOCK_APPLIED_transactions",
                block_id.workchain,
                timestamp,
                numeric_field(block_stats, "transactions")?,
                None,
            )?;
        }
        if block_id.workchain == 0 && master_accepted_at > 0.0 {
            self.add_stat(
                "BLOCK_APPLIED_shard_latency_a2a",
                block_id.workchain,
                timestamp,
                master_accepted_at - timestamp,
                None,
            )?;
        }
        self.summary.applied += 1;
        Ok(())
    }

    fn try_add_block_from(
        &self,
        block_id: &BlockId,
        node: &str,
    ) -> Result<bool, SessionStatsError> {
        Ok(self.transaction.execute(
            "INSERT OR IGNORE INTO processed_blocks_from
             (workchain, shard, seqno, root_hash, file_hash, self)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                block_id.workchain,
                block_id.shard,
                block_id.seqno,
                block_id.root_hash,
                block_id.file_hash,
                node
            ],
        )? == 1)
    }

    fn try_add_applied_block(&self, block_id: &BlockId) -> Result<bool, SessionStatsError> {
        Ok(self.transaction.execute(
            "INSERT OR IGNORE INTO processed_blocks_applied
             (workchain, shard, seqno, root_hash, file_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                block_id.workchain,
                block_id.shard,
                block_id.seqno,
                block_id.root_hash,
                block_id.file_hash
            ],
        )? == 1)
    }

    fn add_stat(
        &mut self,
        name: &str,
        workchain: i32,
        timestamp: f64,
        value: f64,
        source: Option<&str>,
    ) -> Result<(), SessionStatsError> {
        if let Some(source) = source {
            self.add_stat(
                &format!("{name} src={source}"),
                workchain,
                timestamp,
                value,
                None,
            )?;
        }
        let stat_id = self.stat_id(name)?;
        let timestamp = timestamp as i64 / 60 * 60;
        self.transaction.execute(
            "INSERT INTO data (stat_id, workchain, timestamp, v_count, v_sum, v_min, v_max)
             VALUES (?1, ?2, ?3, 1, ?4, ?4, ?4)
             ON CONFLICT (stat_id, workchain, timestamp) DO UPDATE SET
                 v_count = v_count + excluded.v_count,
                 v_sum = v_sum + excluded.v_sum,
                 v_min = min(v_min, excluded.v_min),
                 v_max = max(v_max, excluded.v_max)",
            params![stat_id, workchain, timestamp, value],
        )?;
        Ok(())
    }

    fn stat_id(&mut self, name: &str) -> Result<i64, SessionStatsError> {
        if let Some(id) = self.stat_ids.get(name) {
            return Ok(*id);
        }
        self.transaction
            .execute("INSERT OR IGNORE INTO stats (name) VALUES (?1)", [name])?;
        let id =
            self.transaction
                .query_row("SELECT id FROM stats WHERE name = ?1", [name], |row| {
                    row.get(0)
                })?;
        self.stat_ids.insert(name.to_owned(), id);
        Ok(id)
    }
}

fn event_type(value: &Value) -> Option<&str> {
    value.get("@type").and_then(Value::as_str)
}

fn event_timestamp(value: &Value) -> Option<f64> {
    [
        "timestamp",
        "collated_at",
        "validated_at",
        "started_at",
        "block_timestamp",
    ]
    .into_iter()
    .find_map(|field| value.get(field).and_then(numeric_value))
}

fn required_timestamp(value: &Value) -> Result<f64, SessionStatsError> {
    event_timestamp(value).ok_or_else(|| invalid_field("event timestamp"))
}

fn parse_block_id(value: &Value) -> Result<BlockId, SessionStatsError> {
    Ok(BlockId {
        workchain: integer_field(value, "workchain")? as i32,
        shard: integer_field(value, "shard")?,
        seqno: integer_field(value, "seqno")?,
        root_hash: string_field(value, "root_hash")
            .ok_or_else(|| invalid_field("root_hash"))?
            .to_owned(),
        file_hash: string_field(value, "file_hash")
            .ok_or_else(|| invalid_field("file_hash"))?
            .to_owned(),
    })
}

fn parse_work_time_stats(value: &str) -> HashMap<String, f64> {
    value
        .trim_matches('"')
        .split_whitespace()
        .filter_map(|part| {
            let (name, value) = part.split_once('=')?;
            Some((name.to_owned(), value.parse().ok()?))
        })
        .collect()
}

fn required_field<'value>(
    value: &'value Value,
    field: &str,
) -> Result<&'value Value, SessionStatsError> {
    value.get(field).ok_or_else(|| invalid_field(field))
}

fn string_field<'value>(value: &'value Value, field: &str) -> Option<&'value str> {
    value.get(field).and_then(Value::as_str)
}

fn numeric_field(value: &Value, field: &str) -> Result<f64, SessionStatsError> {
    value
        .get(field)
        .and_then(numeric_value)
        .ok_or_else(|| invalid_field(field))
}

fn optional_numeric_field(value: &Value, field: &str) -> Option<f64> {
    value.get(field).and_then(numeric_value)
}

fn numeric_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn integer_field(value: &Value, field: &str) -> Result<i64, SessionStatsError> {
    let value = required_field(value, field)?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        .ok_or_else(|| invalid_field(field))
}

fn bool_field(value: &Value, field: &str) -> Result<bool, SessionStatsError> {
    optional_bool_field(value, field).ok_or_else(|| invalid_field(field))
}

fn optional_bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

fn value_key(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

fn invalid_field(field: &str) -> SessionStatsError {
    SessionStatsError::InvalidEvent(format!("missing or invalid `{field}`"))
}

/// Failures that preserve the source boundary for session-stat diagnostics.
#[derive(Debug, Error)]
pub enum SessionStatsError {
    #[error("failed to read session log {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid validator session event: {0}")]
    InvalidEvent(String),
    #[error("session statistics database lock was poisoned")]
    Poisoned,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use serde_json::json;

    use super::*;

    fn block_id(seqno: i64) -> Value {
        block_id_for(0, seqno)
    }

    fn block_id_for(workchain: i32, seqno: i64) -> Value {
        json!({
            "workchain": workchain,
            "shard": "-9223372036854775808",
            "seqno": seqno,
            "root_hash": format!("root-{seqno}"),
            "file_hash": format!("file-{seqno}")
        })
    }

    #[test]
    fn direct_fallback_matches_upstream_metric_names_and_is_idempotent() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let database = directory.path().join("stats.sqlite3");
        let log = directory.path().join("log.session-stats");
        let collate = json!({
            "@type": "validatorStats.collatedBlock",
            "block_id": block_id(7),
            "collated_at": 1_700_000_001.0,
            "self": "validator-key",
            "bytes": 900,
            "collated_data_bytes": 40,
            "total_time": 0.7,
            "work_time": 0.5,
            "wait_externals_time": 0.1,
            "work_time_real_stats": "total=0.5 preinit=0.2 create_block=0.1",
            "work_time_cpu_stats": "total=0.4 preinit=0.2 create_block=0.1",
            "block_limits": {
                "bytes": 1000,
                "collated_data_bytes": 50,
                "load_fraction_queue_cleanup": 0.1,
                "load_fraction_dispatch": 0.2,
                "load_fraction_internals": 0.3,
                "load_fraction_externals": 0.4,
                "load_fraction_new_msgs": 0.5
            },
            "block_stats": {
                "transactions": 3,
                "new_out_msg_queue_size": "5",
                "msg_queue_cleaned": 2,
                "neighbors": [{
                    "processed_msgs": 4,
                    "skipped_msgs": 1,
                    "limit_reached": true,
                    "msg_limit": 10
                }],
                "shard_configuration": []
            },
            "storage_stat_cache": {"@type": "cache", "hit_cnt": "2", "hit_cells": "3"}
        });
        fs::write(&log, format!("{collate}\n")).unwrap();
        let store = SessionStatsStore::open(&database, "validator-1").unwrap();

        let first = store.import_file(&log, u64::MAX, true).unwrap();
        let second = store.import_file(&log, u64::MAX, true).unwrap();
        let snapshot = store.snapshot(1_699_999_000, 1_700_001_000, 60).unwrap();
        let metric_names = snapshot
            .buckets
            .iter()
            .map(|bucket| bucket.stat.as_str())
            .collect::<Vec<_>>();

        expect![[r#"
            first collated=1 validated=0
            second collated=0 validated=0
            BLOCK_collate_storage_stat_cache_hit_cells
            BLOCK_collate_storage_stat_cache_hit_cnt
            BLOCK_collate_time_other_wait
            BLOCK_collate_time_other_wait src=validator-1
            BLOCK_collate_time_wait_externals
            BLOCK_collate_time_wait_externals src=validator-1
            BLOCK_collate_work_time_cpu_create_block
            BLOCK_collate_work_time_cpu_create_block src=validator-1
            BLOCK_collate_work_time_cpu_other
            BLOCK_collate_work_time_cpu_other src=validator-1
            BLOCK_collate_work_time_cpu_preinit
            BLOCK_collate_work_time_cpu_preinit src=validator-1
            BLOCK_collate_work_time_real_create_block
            BLOCK_collate_work_time_real_create_block src=validator-1
            BLOCK_collate_work_time_real_other
            BLOCK_collate_work_time_real_other src=validator-1
            BLOCK_collate_work_time_real_preinit
            BLOCK_collate_work_time_real_preinit src=validator-1
            BLOCK_collated_data_size
            BLOCK_collated_data_size_est
            BLOCK_load_fraction_dispatch
            BLOCK_load_fraction_externals
            BLOCK_load_fraction_internals
            BLOCK_load_fraction_new_msgs
            BLOCK_load_fraction_queue_cleanup
            BLOCK_max_neighbor_processed
            BLOCK_msg_queue_cleaned
            BLOCK_msg_queue_size
            BLOCK_neighbor_msg_limit
            BLOCK_queue_limit_reached
            BLOCK_queue_total_processed
            BLOCK_queue_total_skipped
            BLOCK_size
            BLOCK_size_est
            BLOCK_transactions
            shards_count
        "#]]
        .assert_eq(&format!(
            "first collated={} validated={}\nsecond collated={} validated={}\n{}\n",
            first.collated,
            first.validated,
            second.collated,
            second.validated,
            metric_names.join("\n")
        ));
    }

    #[test]
    fn consensus_correlation_produces_applied_rate_metrics() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let database = directory.path().join("stats.sqlite3");
        let log = directory.path().join("log.session-stats");
        let master_id = block_id_for(-1, 100);
        let shard_id = block_id_for(0, 200);
        let collate = |block_id: Value,
                       collated_at: f64,
                       transactions: u64,
                       shard_configuration: Vec<Value>| {
            json!({
                "@type": "validatorStats.collatedBlock",
                "block_id": block_id,
                "collated_at": collated_at,
                "self": "validator-public-key",
                "bytes": 900,
                "collated_data_bytes": 40,
                "total_time": 0.7,
                "work_time": 0.5,
                "block_limits": {"bytes": 1000, "collated_data_bytes": 50},
                "block_stats": {
                    "transactions": transactions,
                    "shard_configuration": shard_configuration
                }
            })
        };
        let session = |id: &str, hash: &str, block_id: Value, timestamp: f64| {
            json!({
                "@type": "consensus.stats.events",
                "id": id,
                "events": [
                    {
                        "@type": "consensus.stats.timestampedEvent",
                        "ts": timestamp - 0.1,
                        "event": {
                            "@type": "consensus.stats.candidateReceived",
                            "id": {"hash": hash},
                            "block": {"id": block_id}
                        }
                    },
                    {
                        "@type": "consensus.stats.timestampedEvent",
                        "ts": timestamp,
                        "event": {
                            "@type": "consensus.stats.blockAccepted",
                            "id": {"hash": hash}
                        }
                    }
                ]
            })
        };
        let events = [
            collate(shard_id.clone(), 1_700_000_099.0, 3, Vec::new()),
            collate(
                master_id.clone(),
                1_700_000_100.0,
                5,
                vec![shard_id.clone()],
            ),
            session("shard-session", "shard-hash", shard_id, 1_700_000_101.0),
            session("master-session", "master-hash", master_id, 1_700_000_102.0),
        ];
        let contents = events
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&log, format!("{contents}\n")).unwrap();
        let store = SessionStatsStore::open(&database, "validator-1").unwrap();

        let first = store.import_file(&log, u64::MAX, true).unwrap();
        let second = store.import_file(&log, u64::MAX, true).unwrap();
        let snapshot = store.snapshot(1_699_999_000, 1_700_001_000, 60).unwrap();
        let applied = snapshot
            .buckets
            .iter()
            .filter(|bucket| bucket.stat.starts_with("BLOCK_APPLIED_"))
            .map(|bucket| {
                format!(
                    "{} wc={} count={} sum={}",
                    bucket.stat, bucket.workchain, bucket.count, bucket.sum
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        expect![[r#"
            first collated=2 applied=2
            second collated=0 applied=0
            BLOCK_APPLIED_blocks wc=-1 count=1 sum=1
            BLOCK_APPLIED_blocks wc=0 count=1 sum=1
            BLOCK_APPLIED_shard_latency_a2a wc=0 count=1 sum=1
            BLOCK_APPLIED_transactions wc=-1 count=1 sum=5
            BLOCK_APPLIED_transactions wc=0 count=1 sum=3
        "#]]
        .assert_eq(&format!(
            "first collated={} applied={}\nsecond collated={} applied={}\n{}\n",
            first.collated, first.applied, second.collated, second.applied, applied
        ));
    }
}
