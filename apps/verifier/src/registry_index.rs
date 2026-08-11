use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Instant, SystemTime, SystemTimeError, UNIX_EPOCH},
};

use async_trait::async_trait;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use thiserror::Error;

use crate::{
    bundle_validation::{StoredBundleValidationError, validate_stored_bundle},
    source_storage::{
        CompilerMetadata, SourceBundleManifest, SourceMapData, SourceStorage, SourceStorageError,
        StoredSourceBundle, StoredSourceFile,
    },
};

const INDEX_SCHEMA_VERSION: i64 = 9;
const UNKNOWN_REVISION: &str = "unknown";

#[async_trait]
pub trait VerificationIndex: Send + Sync + 'static {
    async fn ensure_current(
        &self,
        source_storage: &dyn SourceStorage,
    ) -> Result<(), VerificationIndexError>;

    async fn upsert_bundle(
        &self,
        bundle: &StoredSourceBundle,
        indexed_revision: Option<&str>,
    ) -> Result<(), VerificationIndexError>;

    async fn status(
        &self,
        code_hash: &str,
    ) -> Result<IndexedVerificationStatus, VerificationIndexError>;

    async fn bundle(
        &self,
        code_hash: &str,
    ) -> Result<Option<StoredSourceBundle>, VerificationIndexError>;

    async fn last_verified(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<IndexedLastVerifiedPage, VerificationIndexError>;

    async fn statistics(&self) -> Result<IndexedVerificationStatistics, VerificationIndexError>;

    async fn statistics_history(
        &self,
    ) -> Result<Vec<IndexedVerificationStatisticsHistoryItem>, VerificationIndexError>;

    async fn abi_contracts(
        &self,
        query: IndexedAbiContractsQuery,
    ) -> Result<IndexedAbiContractsPage, VerificationIndexError>;
}

pub type SharedVerificationIndex = Arc<dyn VerificationIndex>;

#[derive(Clone, Debug)]
pub struct IndexedVerificationStatus {
    pub verified: bool,
}

#[derive(Clone, Debug)]
pub struct IndexedLastVerifiedPage {
    pub items: Vec<IndexedVerifiedBundleSummary>,
    pub total: usize,
}

#[derive(Clone, Debug)]
pub struct IndexedVerifiedBundleSummary {
    pub code_hash: String,
    pub source_bundle_hash: String,
    pub verified_at: u64,
    pub storage_revision: String,
    pub entrypoint: String,
    pub compiler: CompilerMetadata,
    pub file_count: usize,
    pub has_tolk_abi: bool,
    pub abi_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct IndexedVerificationStatistics {
    pub total: usize,
    pub languages: Vec<IndexedLanguageStatistics>,
}

#[derive(Clone, Debug)]
pub struct IndexedLanguageStatistics {
    pub language: String,
    pub total: usize,
    pub versions: Vec<IndexedCompilerVersionStatistics>,
}

#[derive(Clone, Debug)]
pub struct IndexedCompilerVersionStatistics {
    pub version: String,
    pub total: usize,
}

#[derive(Clone, Debug)]
pub struct IndexedVerificationStatisticsHistoryItem {
    pub timestamp: u64,
    pub compiler: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct IndexedAbiContractsQuery {
    pub code_hash: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

#[derive(Clone, Debug)]
pub struct IndexedAbiContractsPage {
    pub items: Vec<IndexedAbiContract>,
}

#[derive(Clone, Debug)]
pub struct IndexedAbiContract {
    pub code_hash: String,
    pub abi: Value,
}

pub struct SqliteVerificationIndex {
    connection: Mutex<Connection>,
}

impl SqliteVerificationIndex {
    /// Opens a `SQLite` verification index at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory cannot be created, the
    /// database cannot be opened, or schema initialization fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VerificationIndexError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| VerificationIndexError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        Self::from_connection(Connection::open(path)?)
    }

    /// Opens an in-memory `SQLite` verification index.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or initialized.
    pub fn in_memory() -> Result<Self, VerificationIndexError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, VerificationIndexError> {
        initialize_schema(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>, VerificationIndexError> {
        self.connection
            .lock()
            .map_err(|_| VerificationIndexError::Lock)
    }

    fn indexed_revision(&self) -> Result<Option<String>, VerificationIndexError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "select indexed_revision from registry_index_state where id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(VerificationIndexError::Sqlite)
    }

    fn replace_all(
        &self,
        indexed_revision: &str,
        sources: &[StoredSourceBundle],
    ) -> Result<(), VerificationIndexError> {
        let indexed_at = now_unix_seconds()?;
        {
            let mut connection = self.connection()?;
            let transaction = connection.transaction()?;

            transaction.execute("delete from bundle_abis", [])?;
            transaction.execute("delete from bundle_files", [])?;
            transaction.execute("delete from verified_bundles", [])?;
            transaction.execute("delete from registry_index_state", [])?;

            for bundle in sources {
                insert_bundle(&transaction, bundle, indexed_at)?;
            }
            set_index_state(&transaction, indexed_revision, indexed_at)?;

            transaction.commit()?;
            drop(connection);
        }
        Ok(())
    }
}

#[async_trait]
impl VerificationIndex for SqliteVerificationIndex {
    async fn ensure_current(
        &self,
        source_storage: &dyn SourceStorage,
    ) -> Result<(), VerificationIndexError> {
        let indexed_revision = revision_or_unknown(source_storage.current_revision().await?);
        let previous_revision = self.indexed_revision()?;
        if previous_revision.as_deref() == Some(indexed_revision.as_str()) {
            tracing::debug!(revision = %indexed_revision, "registry index is current");
            return Ok(());
        }

        let started_at = Instant::now();
        tracing::info!(
            previous_revision = previous_revision.as_deref().unwrap_or("<none>"),
            storage_revision = %indexed_revision,
            "registry index is stale; rebuilding"
        );

        let mut sources = Vec::new();
        let code_hashes = source_storage.list_code_hashes().await?;
        let code_hash_count = code_hashes.len();
        tracing::info!(
            completed_code_hashes = 0,
            total_code_hashes = code_hash_count,
            verified_code_hash_count = 0,
            "registry index rebuild progress"
        );

        for (index, code_hash) in code_hashes.into_iter().enumerate() {
            if let Some(bundle) = source_storage.load_bundle(&code_hash).await? {
                validate_stored_bundle(&bundle, &code_hash)?;
                sources.push(bundle);
            }

            let completed = index + 1;
            if completed.is_multiple_of(100) || completed == code_hash_count {
                tracing::info!(
                    completed_code_hashes = completed,
                    total_code_hashes = code_hash_count,
                    verified_code_hash_count = sources.len(),
                    "registry index rebuild progress"
                );
            }
        }

        self.replace_all(&indexed_revision, &sources)?;
        tracing::info!(
            revision = %indexed_revision,
            verified_code_hash_count = sources.len(),
            elapsed_ms = started_at.elapsed().as_secs_f64() * 1_000.0,
            "registry index rebuilt"
        );
        Ok(())
    }

    async fn upsert_bundle(
        &self,
        bundle: &StoredSourceBundle,
        indexed_revision: Option<&str>,
    ) -> Result<(), VerificationIndexError> {
        validate_stored_bundle(bundle, &bundle.manifest.code_hash)?;

        let indexed_at = now_unix_seconds()?;
        {
            let mut connection = self.connection()?;
            let transaction = connection.transaction()?;

            delete_bundle_rows(&transaction, &bundle.manifest.code_hash)?;
            insert_bundle(&transaction, bundle, indexed_at)?;
            if let Some(indexed_revision) = indexed_revision {
                set_index_state(&transaction, indexed_revision, indexed_at)?;
            }

            transaction.commit()?;
            drop(connection);
        }
        Ok(())
    }

    async fn status(
        &self,
        code_hash: &str,
    ) -> Result<IndexedVerificationStatus, VerificationIndexError> {
        let verified = {
            let connection = self.connection()?;
            connection.query_row(
                "select exists(select 1 from verified_bundles where code_hash = ?1)",
                params![code_hash],
                |row| row.get::<_, bool>(0),
            )?
        };

        Ok(IndexedVerificationStatus { verified })
    }

    async fn bundle(
        &self,
        code_hash: &str,
    ) -> Result<Option<StoredSourceBundle>, VerificationIndexError> {
        let connection = self.connection()?;
        let row = connection
            .query_row(
                r"
                select
                  source_bundle_hash,
                  payment_tx_hash,
                  verified_at,
                  compiler_json,
                  storage_revision,
                  source_map_json
                from verified_bundles
                where code_hash = ?1
                ",
                params![code_hash],
                |row| {
                    Ok(IndexedBundleRow {
                        source_bundle_hash: row.get(0)?,
                        payment_tx_hash: row.get(1)?,
                        verified_at: row.get(2)?,
                        compiler_json: row.get(3)?,
                        storage_revision: row.get(4)?,
                        source_map_json: row.get(5)?,
                    })
                },
            )
            .optional()?;
        let Some(row) = row else {
            return Ok(None);
        };
        let bundle = bundle_from_row(&connection, code_hash, row)?;
        drop(connection);
        validate_stored_bundle(&bundle, code_hash)?;
        Ok(Some(bundle))
    }

    async fn last_verified(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<IndexedLastVerifiedPage, VerificationIndexError> {
        let limit_i64 = usize_to_i64("limit", limit)?;
        let offset_i64 = usize_to_i64("offset", offset)?;
        let connection = self.connection()?;
        let total = connection.query_row("select count(*) from verified_bundles", [], |row| {
            row.get::<_, i64>(0)
        })?;
        let total = i64_to_usize("total", total)?;
        let mut statement = connection.prepare(
            r"
            select
              verified_bundles.code_hash,
              verified_bundles.source_bundle_hash,
              verified_bundles.verified_at,
              verified_bundles.compiler_json,
              verified_bundles.storage_revision,
              count(bundle_files.path) as file_count,
              exists (
                select 1
                from bundle_abis
                where bundle_abis.code_hash = verified_bundles.code_hash
              ) as has_tolk_abi,
              (
                select bundle_abis.abi_json
                from bundle_abis
                where bundle_abis.code_hash = verified_bundles.code_hash
                order by bundle_abis.path asc
                limit 1
              ) as abi_json
            from verified_bundles
            left join bundle_files
              on bundle_files.code_hash = verified_bundles.code_hash
            group by
              verified_bundles.code_hash,
              verified_bundles.source_bundle_hash,
              verified_bundles.verified_at,
              verified_bundles.compiler_json,
              verified_bundles.storage_revision
            order by verified_bundles.verified_at desc, verified_bundles.code_hash desc
            limit ?1 offset ?2
            ",
        )?;
        let rows = statement.query_map(params![limit_i64, offset_i64], |row| {
            Ok(IndexedVerifiedBundleSummaryRow {
                code_hash: row.get(0)?,
                source_bundle_hash: row.get(1)?,
                verified_at: row.get(2)?,
                compiler_json: row.get(3)?,
                storage_revision: row.get(4)?,
                file_count: row.get(5)?,
                has_tolk_abi: row.get::<_, i64>(6)? != 0,
                abi_json: row.get(7)?,
            })
        })?;

        let mut items = Vec::new();
        for row in rows {
            items.push(summary_from_row(row?)?);
        }
        drop(statement);
        drop(connection);

        Ok(IndexedLastVerifiedPage { items, total })
    }

    async fn statistics(&self) -> Result<IndexedVerificationStatistics, VerificationIndexError> {
        let connection = self.connection()?;
        let total = connection.query_row("select count(*) from verified_bundles", [], |row| {
            row.get::<_, i64>(0)
        })?;
        let total = i64_to_usize("total", total)?;
        let mut statement = connection.prepare(
            r"
            select
              json_extract(compiler_json, '$.language') as language,
              json_extract(compiler_json, '$.version') as version,
              count(*) as total
            from verified_bundles
            group by
              json_extract(compiler_json, '$.language'),
              json_extract(compiler_json, '$.version')
            order by language asc, version asc
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;

        let mut languages = BTreeMap::<String, IndexedLanguageStatistics>::new();
        for row in rows {
            let (language, version, version_total) = row?;
            let version_total = i64_to_usize("version_total", version_total)?;
            let language = languages.entry(language).or_insert_with_key(|language| {
                IndexedLanguageStatistics {
                    language: language.clone(),
                    total: 0,
                    versions: Vec::new(),
                }
            });
            language.total += version_total;
            language.versions.push(IndexedCompilerVersionStatistics {
                version,
                total: version_total,
            });
        }
        drop(statement);
        drop(connection);

        Ok(IndexedVerificationStatistics {
            total,
            languages: languages.into_values().collect(),
        })
    }

    async fn statistics_history(
        &self,
    ) -> Result<Vec<IndexedVerificationStatisticsHistoryItem>, VerificationIndexError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            r"
            select
              verified_at,
              json_extract(compiler_json, '$.language') as compiler,
              json_extract(compiler_json, '$.version') as version
            from verified_bundles
            order by verified_at asc, compiler asc, version asc, code_hash asc
            ",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut items = Vec::new();
        for row in rows {
            let (timestamp, compiler, version) = row?;
            items.push(IndexedVerificationStatisticsHistoryItem {
                timestamp: i64_to_u64("timestamp", timestamp)?,
                compiler,
                version,
            });
        }
        drop(statement);
        drop(connection);

        Ok(items)
    }

    async fn abi_contracts(
        &self,
        query: IndexedAbiContractsQuery,
    ) -> Result<IndexedAbiContractsPage, VerificationIndexError> {
        let limit_i64 = usize_to_i64("limit", query.limit)?;
        let offset_i64 = usize_to_i64("offset", query.offset)?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            r"
            select
              bundle_abis.code_hash,
              bundle_abis.abi_json
            from bundle_abis
            join verified_bundles
              on verified_bundles.code_hash = bundle_abis.code_hash
            where (?1 is null or bundle_abis.code_hash = ?1)
            group by bundle_abis.code_hash, bundle_abis.abi_json
            order by max(verified_bundles.verified_at) desc, bundle_abis.code_hash desc
            limit ?2 offset ?3
            ",
        )?;
        let rows = statement.query_map(
            params![query.code_hash.as_deref(), limit_i64, offset_i64],
            |row| {
                Ok(IndexedAbiContractRow {
                    code_hash: row.get(0)?,
                    abi_json: row.get(1)?,
                })
            },
        )?;

        let mut items = Vec::new();
        for row in rows {
            items.push(abi_contract_from_row(row?)?);
        }
        drop(statement);
        drop(connection);

        Ok(IndexedAbiContractsPage { items })
    }
}

#[derive(Debug, Error)]
pub enum VerificationIndexError {
    #[error("failed to create registry index directory {path}: {source}", path = path.display())]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("registry index mutex is poisoned")]
    Lock,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Time(#[from] SystemTimeError),
    #[error("timestamp is too large for SQLite integer: {0}")]
    TimestampOutOfRange(u64),
    #[error("registry index integer field {field} has invalid value: {value}")]
    InvalidInteger { field: &'static str, value: i64 },
    #[error("registry index pagination field {field} is too large: {value}")]
    PaginationOutOfRange { field: &'static str, value: usize },
    #[error(transparent)]
    SourceStorage(#[from] SourceStorageError),
    #[error(transparent)]
    BundleValidation(#[from] StoredBundleValidationError),
}

struct IndexedBundleRow {
    source_bundle_hash: String,
    payment_tx_hash: Option<String>,
    verified_at: i64,
    compiler_json: String,
    storage_revision: String,
    source_map_json: Option<String>,
}

struct IndexedVerifiedBundleSummaryRow {
    code_hash: String,
    source_bundle_hash: String,
    verified_at: i64,
    compiler_json: String,
    storage_revision: String,
    file_count: i64,
    has_tolk_abi: bool,
    abi_json: Option<String>,
}

struct IndexedAbiContractRow {
    code_hash: String,
    abi_json: String,
}

fn initialize_schema(connection: &Connection) -> Result<(), VerificationIndexError> {
    let user_version =
        connection.query_row("pragma user_version", [], |row| row.get::<_, i64>(0))?;
    if user_version != INDEX_SCHEMA_VERSION {
        connection.execute_batch(
            r"
            drop table if exists bundle_abis;
            drop table if exists bundle_sources;
            drop table if exists bundle_files;
            drop table if exists verified_code_hashes;
            drop table if exists verified_bundles;
            drop table if exists registry_index_state;
            ",
        )?;
    }

    connection.execute_batch(
        r"
        pragma foreign_keys = on;

        create table if not exists registry_index_state (
          id integer primary key check (id = 1),
          indexed_revision text not null,
          indexed_at integer not null
        );

        create table if not exists verified_bundles (
          code_hash text primary key,
          source_bundle_hash text not null,
          payment_tx_hash text,
          verified_at integer not null,
          compiler_json text not null,
          storage_revision text not null,
          source_map_json text,
          indexed_at integer not null
        );

        create table if not exists bundle_files (
          code_hash text not null,
          path text not null,
          content_hash text not null,
          content text not null,
          include_in_command integer,
          is_stdlib integer,
          has_include_directives integer,
          primary key (code_hash, path)
        );

        create table if not exists bundle_abis (
          code_hash text not null,
          path text not null,
          content_hash text not null,
          abi_json text not null,
          indexed_at integer not null,
          primary key (code_hash, path)
        );

        create index if not exists bundle_files_by_bundle
          on bundle_files (code_hash);

        create index if not exists bundle_abis_by_code_hash
          on bundle_abis (code_hash);
        ",
    )?;
    connection.pragma_update(None, "user_version", INDEX_SCHEMA_VERSION)?;

    Ok(())
}

fn insert_bundle(
    transaction: &Transaction<'_>,
    bundle: &StoredSourceBundle,
    indexed_at: i64,
) -> Result<(), VerificationIndexError> {
    let manifest = &bundle.manifest;
    transaction.execute(
        r"
        insert into verified_bundles (
          code_hash,
          source_bundle_hash,
          payment_tx_hash,
          verified_at,
          compiler_json,
          storage_revision,
          source_map_json,
          indexed_at
        ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        on conflict (code_hash) do update set
          source_bundle_hash = excluded.source_bundle_hash,
          payment_tx_hash = excluded.payment_tx_hash,
          verified_at = excluded.verified_at,
          compiler_json = excluded.compiler_json,
          storage_revision = excluded.storage_revision,
          source_map_json = excluded.source_map_json,
          indexed_at = excluded.indexed_at
        ",
        params![
            &manifest.code_hash,
            &manifest.source_bundle_hash,
            &manifest.payment_tx_hash,
            i64::try_from(manifest.verified_at).map_err(|_| {
                VerificationIndexError::TimestampOutOfRange(manifest.verified_at)
            })?,
            serde_json::to_string(&manifest.compiler)?,
            &bundle.storage_revision,
            manifest
                .source_map
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
            indexed_at,
        ],
    )?;

    for file in &bundle.files {
        transaction.execute(
            r"
            insert into bundle_files (
              code_hash,
              path,
              content_hash,
              content,
              include_in_command,
              is_stdlib,
              has_include_directives
            ) values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                &manifest.code_hash,
                &file.path,
                &file.content_hash,
                &file.content,
                option_bool_to_i64(file.include_in_command),
                option_bool_to_i64(file.is_stdlib),
                option_bool_to_i64(file.has_include_directives),
            ],
        )?;

        if let Some(abi_json) = compiler_abi_json(manifest, file) {
            transaction.execute(
                r"
                insert into bundle_abis (
                  code_hash,
                  path,
                  content_hash,
                  abi_json,
                  indexed_at
                ) values (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    &manifest.code_hash,
                    &file.path,
                    &file.content_hash,
                    abi_json.to_string(),
                    indexed_at,
                ],
            )?;
        }
    }

    Ok(())
}

fn delete_bundle_rows(
    transaction: &Transaction<'_>,
    code_hash: &str,
) -> Result<(), VerificationIndexError> {
    transaction.execute(
        "delete from bundle_abis where code_hash = ?1",
        params![code_hash],
    )?;
    transaction.execute(
        "delete from bundle_files where code_hash = ?1",
        params![code_hash],
    )?;
    transaction.execute(
        "delete from verified_bundles where code_hash = ?1",
        params![code_hash],
    )?;
    Ok(())
}

fn set_index_state(
    transaction: &Transaction<'_>,
    indexed_revision: &str,
    indexed_at: i64,
) -> Result<(), VerificationIndexError> {
    transaction.execute(
        r"
        insert into registry_index_state (
          id,
          indexed_revision,
          indexed_at
        ) values (1, ?1, ?2)
        on conflict (id) do update set
          indexed_revision = excluded.indexed_revision,
          indexed_at = excluded.indexed_at
        ",
        params![indexed_revision, indexed_at],
    )?;
    Ok(())
}

fn bundle_from_row(
    connection: &Connection,
    code_hash: &str,
    row: IndexedBundleRow,
) -> Result<StoredSourceBundle, VerificationIndexError> {
    let files = bundle_files(connection, code_hash)?;
    let compiler = serde_json::from_str::<CompilerMetadata>(&row.compiler_json)?;
    let source_map = row
        .source_map_json
        .as_deref()
        .map(serde_json::from_str::<SourceMapData>)
        .transpose()?;

    Ok(StoredSourceBundle {
        storage_revision: row.storage_revision,
        manifest: SourceBundleManifest {
            code_hash: code_hash.to_owned(),
            source_bundle_hash: row.source_bundle_hash,
            payment_tx_hash: row.payment_tx_hash,
            verified_at: i64_to_u64("verified_at", row.verified_at)?,
            compiler,
            source_map,
        },
        files,
    })
}

fn bundle_files(
    connection: &Connection,
    code_hash: &str,
) -> Result<Vec<StoredSourceFile>, VerificationIndexError> {
    let mut statement = connection.prepare(
        r"
        select
          path,
          content_hash,
          content,
          include_in_command,
          is_stdlib,
          has_include_directives
        from bundle_files
        where code_hash = ?1
        order by path
        ",
    )?;
    let rows = statement.query_map(params![code_hash], |row| {
        Ok(StoredSourceFile {
            path: row.get(0)?,
            content_hash: row.get(1)?,
            content: row.get(2)?,
            include_in_command: option_i64_to_bool(row.get(3)?),
            is_stdlib: option_i64_to_bool(row.get(4)?),
            has_include_directives: option_i64_to_bool(row.get(5)?),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(VerificationIndexError::Sqlite)
}

fn summary_from_row(
    row: IndexedVerifiedBundleSummaryRow,
) -> Result<IndexedVerifiedBundleSummary, VerificationIndexError> {
    let compiler = serde_json::from_str::<CompilerMetadata>(&row.compiler_json)?;
    let entrypoint = compiler.entrypoint.clone();
    let abi_name = abi_contract_name(row.abi_json.as_deref())?;

    Ok(IndexedVerifiedBundleSummary {
        code_hash: row.code_hash,
        source_bundle_hash: row.source_bundle_hash,
        verified_at: i64_to_u64("verified_at", row.verified_at)?,
        storage_revision: row.storage_revision,
        entrypoint,
        compiler,
        file_count: i64_to_usize("file_count", row.file_count)?,
        has_tolk_abi: row.has_tolk_abi,
        abi_name,
    })
}

fn abi_contract_name(abi_json: Option<&str>) -> Result<Option<String>, VerificationIndexError> {
    let Some(abi_json) = abi_json else {
        return Ok(None);
    };
    let abi = serde_json::from_str::<Value>(abi_json)?;

    Ok(abi
        .get("contract_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned))
}

fn abi_contract_from_row(
    row: IndexedAbiContractRow,
) -> Result<IndexedAbiContract, VerificationIndexError> {
    let abi = serde_json::from_str::<Value>(&row.abi_json)?;

    Ok(IndexedAbiContract {
        code_hash: row.code_hash,
        abi,
    })
}

fn compiler_abi_json(manifest: &SourceBundleManifest, file: &StoredSourceFile) -> Option<Value> {
    let language = manifest.compiler.language.as_str();
    if !(language.eq_ignore_ascii_case("tolk") || language.eq_ignore_ascii_case("tact"))
        || !file.path.ends_with(".abi.json")
    {
        return None;
    }

    serde_json::from_str::<Value>(&file.content).ok()
}

fn now_unix_seconds() -> Result<i64, VerificationIndexError> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    i64::try_from(seconds).map_err(|_| VerificationIndexError::TimestampOutOfRange(seconds))
}

fn i64_to_u64(field: &'static str, value: i64) -> Result<u64, VerificationIndexError> {
    u64::try_from(value).map_err(|_| VerificationIndexError::InvalidInteger { field, value })
}

fn i64_to_usize(field: &'static str, value: i64) -> Result<usize, VerificationIndexError> {
    usize::try_from(value).map_err(|_| VerificationIndexError::InvalidInteger { field, value })
}

fn usize_to_i64(field: &'static str, value: usize) -> Result<i64, VerificationIndexError> {
    i64::try_from(value).map_err(|_| VerificationIndexError::PaginationOutOfRange { field, value })
}

fn revision_or_unknown(revision: Option<String>) -> String {
    revision.unwrap_or_else(|| UNKNOWN_REVISION.to_owned())
}

const fn option_bool_to_i64(value: Option<bool>) -> Option<i64> {
    match value {
        Some(value) => Some(if value { 1 } else { 0 }),
        None => None,
    }
}

const fn option_i64_to_bool(value: Option<i64>) -> Option<bool> {
    match value {
        Some(value) => Some(value != 0),
        None => None,
    }
}
