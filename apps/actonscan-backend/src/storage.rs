use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use ton_indexer_core::{BlockId, CheckpointStore, Error as IndexerError, Hash256};

use crate::{
    opcodes::{OpcodeAggregate, OpcodeBatchStats, OpcodeStats},
    stats::{SAMPLE_RETENTION_SECONDS, TpsSample, TpsStats},
};

const SCHEMA_VERSION: i64 = 2;

/// `SQLite` storage for the indexer checkpoint and network statistics.
#[derive(Clone)]
pub struct SqliteStorage {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Opens the database and creates its schema.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or database cannot be initialized.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| StorageError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        initialize_schema(&connection)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Loads the persisted TPS samples into a new in-memory accumulator.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be read.
    pub fn load_tps_stats(&self) -> Result<TpsStats, StorageError> {
        Ok(TpsStats::from_samples(self.load_tps_samples()?))
    }

    /// Loads the persisted all-time opcode statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be read.
    pub fn load_opcode_stats(&self) -> Result<OpcodeStats, StorageError> {
        let connection = self.connection()?;
        let state = connection
            .query_row(
                "select first_masterchain_seqno, latest_masterchain_seqno,
                        total_messages, messages_with_opcode
                 from opcode_indexer_state where id = 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u64>(2)?,
                        row.get::<_, u64>(3)?,
                    ))
                },
            )
            .optional()?;
        let mut statement = connection.prepare(
            "select opcode, messages, first_transaction_hash, second_transaction_hash
             from opcode_stats order by opcode asc",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
                row.get::<_, Option<Vec<u8>>>(3)?,
            ))
        })?;
        let mut counts = Vec::new();
        for row in rows {
            let (opcode, messages, first_hash, second_hash) = row?;
            let mut example_transactions = Vec::new();
            for hash in [first_hash, second_hash].into_iter().flatten() {
                example_transactions.push(decode_transaction_hash(hash)?);
            }
            counts.push((
                opcode,
                OpcodeAggregate {
                    messages,
                    example_transactions,
                },
            ));
        }
        drop(statement);
        drop(connection);

        let (first, latest, total_messages, messages_with_opcode) = state
            .map_or((None, None, 0, 0), |(first, latest, total, with_opcode)| {
                (Some(first), Some(latest), total, with_opcode)
            });
        Ok(OpcodeStats::from_persisted(
            first,
            latest,
            total_messages,
            messages_with_opcode,
            counts,
        ))
    }

    pub(crate) fn record_batch_stats(
        &self,
        tps_sample: TpsSample,
        opcode_batch: &OpcodeBatchStats,
    ) -> Result<(), StorageError> {
        debug_assert_eq!(tps_sample.masterchain_seqno, opcode_batch.masterchain_seqno);
        let cutoff = tps_sample
            .timestamp
            .saturating_sub(SAMPLE_RETENTION_SECONDS);

        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "insert into tps_samples (masterchain_seqno, block_time, transactions)
             values (?1, ?2, ?3)
             on conflict (masterchain_seqno) do update set
               block_time = excluded.block_time,
               transactions = excluded.transactions",
            params![
                tps_sample.masterchain_seqno,
                tps_sample.timestamp,
                tps_sample.transactions
            ],
        )?;
        transaction.execute(
            "delete from tps_samples where block_time < ?1",
            params![cutoff],
        )?;

        let latest_opcode_seqno = transaction
            .query_row(
                "select latest_masterchain_seqno from opcode_indexer_state where id = 1",
                [],
                |row| row.get::<_, u32>(0),
            )
            .optional()?;
        if latest_opcode_seqno.is_none_or(|latest| opcode_batch.masterchain_seqno > latest) {
            for (&opcode, batch_entry) in &opcode_batch.counts {
                let existing = transaction
                    .query_row(
                        "select messages, first_transaction_hash, second_transaction_hash
                         from opcode_stats where opcode = ?1",
                        params![opcode],
                        |row| {
                            Ok((
                                row.get::<_, u64>(0)?,
                                row.get::<_, Option<Vec<u8>>>(1)?,
                                row.get::<_, Option<Vec<u8>>>(2)?,
                            ))
                        },
                    )
                    .optional()?;
                let (current_messages, first_hash, second_hash) =
                    existing.unwrap_or((0, None, None));
                let mut aggregate = OpcodeAggregate {
                    messages: current_messages,
                    example_transactions: Vec::new(),
                };
                for hash in [first_hash, second_hash].into_iter().flatten() {
                    aggregate
                        .example_transactions
                        .push(decode_transaction_hash(hash)?);
                }
                aggregate.merge(batch_entry);
                let first_hash = aggregate
                    .example_transactions
                    .first()
                    .map(|hash| hash.as_bytes().as_slice());
                let second_hash = aggregate
                    .example_transactions
                    .get(1)
                    .map(|hash| hash.as_bytes().as_slice());
                transaction.execute(
                    "insert into opcode_stats (
                       opcode, messages, first_transaction_hash, second_transaction_hash
                     ) values (?1, ?2, ?3, ?4)
                     on conflict (opcode) do update set
                       messages = excluded.messages,
                       first_transaction_hash = excluded.first_transaction_hash,
                       second_transaction_hash = excluded.second_transaction_hash",
                    params![opcode, aggregate.messages, first_hash, second_hash,],
                )?;
            }
            transaction.execute(
                "insert into opcode_indexer_state (
                   id, first_masterchain_seqno, latest_masterchain_seqno,
                   total_messages, messages_with_opcode
                 ) values (1, ?1, ?1, ?2, ?3)
                 on conflict (id) do update set
                   latest_masterchain_seqno = excluded.latest_masterchain_seqno,
                   total_messages = opcode_indexer_state.total_messages + excluded.total_messages,
                   messages_with_opcode = opcode_indexer_state.messages_with_opcode + excluded.messages_with_opcode",
                params![
                    opcode_batch.masterchain_seqno,
                    opcode_batch.total_messages,
                    opcode_batch.messages_with_opcode(),
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        Ok(())
    }

    fn load_tps_samples(&self) -> Result<Vec<TpsSample>, StorageError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "select masterchain_seqno, block_time, transactions
             from tps_samples
             order by masterchain_seqno asc",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(TpsSample {
                masterchain_seqno: row.get(0)?,
                timestamp: row.get(1)?,
                transactions: row.get(2)?,
            })
        })?;

        let mut samples = Vec::new();
        for row in rows {
            samples.push(row?);
        }
        drop(statement);
        drop(connection);
        Ok(samples)
    }

    fn load_checkpoint(&self) -> Result<Option<BlockId>, StorageError> {
        let connection = self.connection()?;
        let value = connection
            .query_row(
                "select value from indexer_checkpoint where id = 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        drop(connection);

        value
            .map(|value| serde_json::from_str(&value).map_err(StorageError::Json))
            .transpose()
    }

    fn save_checkpoint(&self, checkpoint: &BlockId) -> Result<(), StorageError> {
        let value = serde_json::to_string(checkpoint)?;
        let connection = self.connection()?;
        connection.execute(
            "insert into indexer_checkpoint (id, value) values (1, ?1)
             on conflict (id) do update set
               value = excluded.value",
            params![value],
        )?;
        drop(connection);
        Ok(())
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, StorageError> {
        self.connection.lock().map_err(|_| StorageError::Lock)
    }
}

#[async_trait]
impl CheckpointStore for SqliteStorage {
    async fn load(&self) -> ton_indexer_core::Result<Option<BlockId>> {
        self.load_checkpoint().map_err(IndexerError::checkpoint)
    }

    async fn save(&self, checkpoint: &BlockId) -> ton_indexer_core::Result<()> {
        self.save_checkpoint(checkpoint)
            .map_err(IndexerError::checkpoint)
    }
}

fn initialize_schema(connection: &Connection) -> Result<(), StorageError> {
    let version = connection.query_row("pragma user_version", [], |row| row.get::<_, i64>(0))?;
    match version {
        // SQLite uses version 0 for a new database that has no schema.
        0 => connection.execute_batch(
            "create table if not exists indexer_checkpoint (
               id integer primary key check (id = 1),
               value text not null
             );
             create table if not exists tps_samples (
               masterchain_seqno integer primary key,
               block_time integer not null,
               transactions integer not null
             );",
        )?,
        1 => {}
        SCHEMA_VERSION => return Ok(()),
        version => return Err(StorageError::UnsupportedSchemaVersion(version)),
    }

    connection.execute_batch(
        "create table if not exists opcode_stats (
           opcode integer primary key,
           messages integer not null,
           first_transaction_hash blob
             check (first_transaction_hash is null or length(first_transaction_hash) = 32),
           second_transaction_hash blob
             check (second_transaction_hash is null or length(second_transaction_hash) = 32)
         );
         create table if not exists opcode_indexer_state (
           id integer primary key check (id = 1),
           first_masterchain_seqno integer not null,
           latest_masterchain_seqno integer not null,
           total_messages integer not null,
           messages_with_opcode integer not null
         );
         pragma user_version = 2;",
    )?;
    Ok(())
}

fn decode_transaction_hash(value: Vec<u8>) -> Result<Hash256, StorageError> {
    let length = value.len();
    let bytes = value
        .try_into()
        .map_err(|_| StorageError::InvalidTransactionHashLength(length))?;
    Ok(Hash256::new(bytes))
}

/// Errors produced by the Actonscan `SQLite` storage.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to create Actonscan database directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Actonscan database mutex is poisoned")]
    Lock,
    #[error("unsupported Actonscan database schema version {0}")]
    UnsupportedSchemaVersion(i64),
    #[error("stored transaction hash has {0} bytes instead of 32")]
    InvalidTransactionHashLength(usize),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn database_reopen_restores_checkpoint_and_statistics() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("actonscan.sqlite3");
        let checkpoint = BlockId {
            workchain: BlockId::MASTERCHAIN_WORKCHAIN,
            shard: BlockId::FULL_SHARD,
            seqno: 42,
            root_hash: Hash256::new([1; 32]),
            file_hash: Hash256::new([2; 32]),
        };
        let sample = TpsSample {
            masterchain_seqno: 42,
            timestamp: 1_000,
            transactions: 120,
        };

        let storage = SqliteStorage::open(&path).unwrap();
        let transaction_hashes = [Hash256::new([3; 32]), Hash256::new([4; 32])];
        let opcode_batch = OpcodeBatchStats {
            masterchain_seqno: 42,
            total_messages: 3,
            counts: [(
                0x1234_5678,
                OpcodeAggregate {
                    messages: 2,
                    example_transactions: transaction_hashes.to_vec(),
                },
            )]
            .into(),
        };
        storage.record_batch_stats(sample, &opcode_batch).unwrap();
        storage.record_batch_stats(sample, &opcode_batch).unwrap();
        storage.save(&checkpoint).await.unwrap();
        drop(storage);

        let storage = SqliteStorage::open(path).unwrap();
        assert_eq!(storage.load().await.unwrap(), Some(checkpoint));
        assert_eq!(storage.load_tps_samples().unwrap(), vec![sample]);
        assert_eq!(
            storage
                .load_opcode_stats()
                .unwrap()
                .snapshot(usize::MAX, 1)
                .await,
            crate::opcodes::OpcodeSnapshot {
                first_masterchain_seqno: Some(42),
                latest_masterchain_seqno: Some(42),
                total_messages: 3,
                messages_with_opcode: 2,
                total_opcodes: 1,
                matching_opcodes: 1,
                opcodes: vec![crate::opcodes::OpcodeCount {
                    opcode: 0x1234_5678,
                    messages: 2,
                    example_transaction_hashes: transaction_hashes
                        .into_iter()
                        .map(|hash| hash.to_string())
                        .collect(),
                }],
            }
        );
    }

    #[tokio::test]
    async fn database_upgrades_v1_schema_for_opcode_statistics() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("actonscan.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "create table indexer_checkpoint (
                   id integer primary key check (id = 1),
                   value text not null
                 );
                 create table tps_samples (
                   masterchain_seqno integer primary key,
                   block_time integer not null,
                   transactions integer not null
                 );
                 pragma user_version = 1;",
            )
            .unwrap();
        drop(connection);

        let storage = SqliteStorage::open(path).unwrap();

        assert_eq!(
            storage
                .load_opcode_stats()
                .unwrap()
                .snapshot(usize::MAX, 1)
                .await,
            crate::opcodes::OpcodeSnapshot {
                first_masterchain_seqno: None,
                latest_masterchain_seqno: None,
                total_messages: 0,
                messages_with_opcode: 0,
                total_opcodes: 0,
                matching_opcodes: 0,
                opcodes: Vec::new(),
            }
        );
        assert_eq!(
            storage
                .connection()
                .unwrap()
                .query_row("pragma user_version", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            SCHEMA_VERSION
        );
    }

    #[tokio::test]
    async fn database_does_not_store_transaction_hashes_for_singletons() {
        let directory = tempfile::tempdir().unwrap();
        let storage = SqliteStorage::open(directory.path().join("actonscan.sqlite3")).unwrap();
        let sample = TpsSample {
            masterchain_seqno: 1,
            timestamp: 1_000,
            transactions: 1,
        };
        let opcode_batch = OpcodeBatchStats {
            masterchain_seqno: 1,
            total_messages: 1,
            counts: [(
                0x1234_5678,
                OpcodeAggregate {
                    messages: 1,
                    example_transactions: vec![Hash256::new([1; 32])],
                },
            )]
            .into(),
        };

        storage.record_batch_stats(sample, &opcode_batch).unwrap();

        let hashes = storage
            .connection()
            .unwrap()
            .query_row(
                "select first_transaction_hash, second_transaction_hash
                 from opcode_stats where opcode = ?1",
                params![0x1234_5678_u32],
                |row| {
                    Ok((
                        row.get::<_, Option<Vec<u8>>>(0)?,
                        row.get::<_, Option<Vec<u8>>>(1)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(hashes, (None, None));

        let snapshot = storage
            .load_opcode_stats()
            .unwrap()
            .snapshot(usize::MAX, 1)
            .await;
        assert!(snapshot.opcodes[0].example_transaction_hashes.is_empty());
    }

    #[test]
    fn database_prunes_expired_tps_samples() {
        let directory = tempfile::tempdir().unwrap();
        let storage = SqliteStorage::open(directory.path().join("actonscan.sqlite3")).unwrap();
        let old = TpsSample {
            masterchain_seqno: 1,
            timestamp: 1_000,
            transactions: 10,
        };
        let current = TpsSample {
            masterchain_seqno: 2,
            timestamp: 1_000 + SAMPLE_RETENTION_SECONDS + 1,
            transactions: 20,
        };

        storage
            .record_batch_stats(
                old,
                &OpcodeBatchStats {
                    masterchain_seqno: old.masterchain_seqno,
                    ..OpcodeBatchStats::default()
                },
            )
            .unwrap();
        storage
            .record_batch_stats(
                current,
                &OpcodeBatchStats {
                    masterchain_seqno: current.masterchain_seqno,
                    ..OpcodeBatchStats::default()
                },
            )
            .unwrap();

        assert_eq!(storage.load_tps_samples().unwrap(), vec![current]);
    }
}
