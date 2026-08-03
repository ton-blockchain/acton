use std::{
    env, fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use thiserror::Error;

const CONFIG_PATH_ENV: &str = "ACTONSCAN_CONFIG";
const DEFAULT_CONFIG_PATH: &str = "config.toml";
const DEFAULT_DATABASE_PATH: &str = "actonscan.sqlite3";
const DEFAULT_BACKFILL_BATCHES: u32 = 1_024;
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3008";
const DEFAULT_LITESERVER_PARALLELISM: usize = 4;
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_POLL_INTERVAL_MS: u64 = 100;

/// Process configuration loaded from TOML.
#[derive(Clone, Debug)]
pub struct Config {
    bind_addr: SocketAddr,
    logging_level: String,
    database_path: PathBuf,
    indexer: IndexerConfig,
}

/// Settings for the LiteServer-backed TPS indexer.
#[derive(Clone, Debug)]
pub struct IndexerConfig {
    pub(crate) global_config_path: PathBuf,
    pub(crate) parallelism: usize,
    pub(crate) backfill_batches: u32,
    pub(crate) poll_interval: Duration,
}

impl Config {
    /// Loads configuration from the `ACTONSCAN_CONFIG` path or `config.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be read, parsed, or validated.
    pub fn load() -> Result<Self, ConfigError> {
        let path = env::var_os(CONFIG_PATH_ENV)
            .map_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH), PathBuf::from);
        Self::load_from_path(path)
    }

    /// Loads configuration from a specific TOML file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be read, parsed, or validated.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let raw_config = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file =
            toml::from_str::<ConfigFile>(&raw_config).map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source,
            })?;

        let bind_addr = file.server.bind_addr.unwrap_or_else(default_bind_addr);
        let parallelism = file
            .indexer
            .liteserver_parallelism
            .unwrap_or(DEFAULT_LITESERVER_PARALLELISM);
        if parallelism == 0 {
            return Err(ConfigError::Invalid(
                "indexer.liteserver_parallelism must be greater than zero".to_owned(),
            ));
        }

        let backfill_batches = file
            .indexer
            .tps_backfill_batches
            .unwrap_or(DEFAULT_BACKFILL_BATCHES);
        if backfill_batches == 0 {
            return Err(ConfigError::Invalid(
                "indexer.tps_backfill_batches must be greater than zero".to_owned(),
            ));
        }

        let poll_interval_ms = file
            .indexer
            .poll_interval_ms
            .unwrap_or(DEFAULT_POLL_INTERVAL_MS);
        if poll_interval_ms == 0 {
            return Err(ConfigError::Invalid(
                "indexer.poll_interval_ms must be greater than zero".to_owned(),
            ));
        }

        let default_global_config = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json");
        let global_config_path = file
            .indexer
            .global_config_path
            .unwrap_or(default_global_config);

        Ok(Self {
            bind_addr,
            logging_level: file
                .logging
                .level
                .unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_owned()),
            database_path: file
                .storage
                .database_path
                .unwrap_or_else(|| PathBuf::from(DEFAULT_DATABASE_PATH)),
            indexer: IndexerConfig {
                global_config_path,
                parallelism,
                backfill_batches,
                poll_interval: Duration::from_millis(poll_interval_ms),
            },
        })
    }

    /// Returns the HTTP listen address.
    #[must_use]
    pub const fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    /// Returns the tracing filter configured for the service.
    #[must_use]
    pub fn logging_level(&self) -> &str {
        &self.logging_level
    }

    /// Returns the path to the `SQLite` database.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// Returns the indexer settings.
    #[must_use]
    pub const fn indexer(&self) -> &IndexerConfig {
        &self.indexer
    }
}

fn default_bind_addr() -> SocketAddr {
    DEFAULT_BIND_ADDR
        .parse()
        .expect("default Actonscan bind address must be valid")
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config {path}: {source}")]
    Read { path: PathBuf, source: io::Error },
    #[error("failed to parse config {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid Actonscan config: {0}")]
    Invalid(String),
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    logging: LoggingConfig,
    #[serde(default)]
    storage: StorageConfig,
    #[serde(default)]
    indexer: IndexerFileConfig,
}

#[derive(Debug, Default, Deserialize)]
struct ServerConfig {
    bind_addr: Option<SocketAddr>,
}

#[derive(Debug, Default, Deserialize)]
struct LoggingConfig {
    level: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StorageConfig {
    database_path: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
struct IndexerFileConfig {
    global_config_path: Option<PathBuf>,
    liteserver_parallelism: Option<usize>,
    tps_backfill_batches: Option<u32>,
    poll_interval_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_database_path() {
        let directory = tempfile::tempdir().unwrap();
        let config_path = directory.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
                [storage]
                database_path = "data/actonscan.sqlite3"
            "#,
        )
        .unwrap();

        let config = Config::load_from_path(config_path).unwrap();

        assert_eq!(config.database_path(), Path::new("data/actonscan.sqlite3"));
    }
}
