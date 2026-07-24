use std::{
    env, fmt, fs, io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Deserialize;
use thiserror::Error;

const DEFAULT_CONFIG_PATH: &str = "config.toml";
const CONFIG_PATH_ENV: &str = "VERIFIER_CONFIG";
const DEFAULT_LOG_LEVEL: &str = "info";
const MAINNET_TONCENTER_BASE_URL: &str = "https://toncenter.com";
const TESTNET_TONCENTER_BASE_URL: &str = "https://testnet.toncenter.com";
const LOCALNET_TONCENTER_BASE_URL: &str = "http://127.0.0.1:5411";
const DEFAULT_COMPILER_NODE_BIN: &str = "node";
const DEFAULT_COMPILER_WORKER_PATH: &str = "compiler-worker/compile.mjs";
const DEFAULT_COMPILER_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_SOURCE_REPOSITORY_REMOTE: &str = "origin";
const DEFAULT_SOURCE_REPOSITORY_COMMIT_ENABLED: bool = true;
const DEFAULT_SOURCE_REPOSITORY_PUSH_ENABLED: bool = true;
const DEFAULT_SOURCE_REPOSITORY_AUTHOR_NAME: &str = "ton-verifier";
const DEFAULT_SOURCE_REPOSITORY_AUTHOR_EMAIL: &str = "ton-verifier@example.invalid";
const DEFAULT_REGISTRY_INDEX_PATH: &str = "verifier-index.sqlite3";

#[derive(Clone, Debug)]
pub struct Config {
    bind_addr: SocketAddr,
    logging_level: String,
    network: TonNetwork,
    toncenter_base_url: Option<String>,
    toncenter_api_key: Option<String>,
    source_repository_path: Option<PathBuf>,
    source_repository_remote: String,
    source_repository_branch: Option<String>,
    source_repository_commit_enabled: bool,
    source_repository_push_enabled: bool,
    source_repository_author_name: String,
    source_repository_author_email: String,
    registry_index_path: PathBuf,
    compiler_node_bin: String,
    compiler_worker_path: PathBuf,
    compiler_timeout: Duration,
}

impl Config {
    /// Loads configuration from the `VERIFIER_CONFIG` path or `config.toml`.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed as TOML.
    pub fn load() -> Result<Self, ConfigError> {
        let path = env::var_os(CONFIG_PATH_ENV)
            .map_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH), PathBuf::from);

        Self::load_from_path(path)
    }

    /// Loads configuration from a specific TOML file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed as TOML.
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

        Ok(file.into_config())
    }

    #[must_use]
    pub const fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    #[must_use]
    pub fn logging_level(&self) -> &str {
        &self.logging_level
    }

    #[must_use]
    pub const fn network(&self) -> TonNetwork {
        self.network
    }

    #[must_use]
    pub fn toncenter_base_url(&self) -> &str {
        self.toncenter_base_url
            .as_deref()
            .unwrap_or_else(|| self.network.default_toncenter_base_url())
    }

    #[must_use]
    pub fn toncenter_api_key(&self) -> Option<&str> {
        self.toncenter_api_key.as_deref()
    }

    #[must_use]
    pub fn source_repository_path(&self) -> Option<&Path> {
        self.source_repository_path.as_deref()
    }

    #[must_use]
    pub fn source_repository_remote(&self) -> &str {
        &self.source_repository_remote
    }

    #[must_use]
    pub fn source_repository_branch(&self) -> Option<&str> {
        self.source_repository_branch.as_deref()
    }

    #[must_use]
    pub const fn source_repository_commit_enabled(&self) -> bool {
        self.source_repository_commit_enabled
    }

    #[must_use]
    pub const fn source_repository_push_enabled(&self) -> bool {
        self.source_repository_push_enabled
    }

    #[must_use]
    pub fn source_repository_author_name(&self) -> &str {
        &self.source_repository_author_name
    }

    #[must_use]
    pub fn source_repository_author_email(&self) -> &str {
        &self.source_repository_author_email
    }

    #[must_use]
    pub fn registry_index_path(&self) -> &Path {
        &self.registry_index_path
    }

    #[must_use]
    pub fn compiler_node_bin(&self) -> &str {
        &self.compiler_node_bin
    }

    #[must_use]
    pub fn compiler_worker_path(&self) -> &Path {
        &self.compiler_worker_path
    }

    #[must_use]
    pub const fn compiler_timeout(&self) -> Duration {
        self.compiler_timeout
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            logging_level: DEFAULT_LOG_LEVEL.to_owned(),
            network: TonNetwork::Mainnet,
            toncenter_base_url: None,
            toncenter_api_key: None,
            source_repository_path: None,
            source_repository_remote: DEFAULT_SOURCE_REPOSITORY_REMOTE.to_owned(),
            source_repository_branch: None,
            source_repository_commit_enabled: DEFAULT_SOURCE_REPOSITORY_COMMIT_ENABLED,
            source_repository_push_enabled: DEFAULT_SOURCE_REPOSITORY_PUSH_ENABLED,
            source_repository_author_name: DEFAULT_SOURCE_REPOSITORY_AUTHOR_NAME.to_owned(),
            source_repository_author_email: DEFAULT_SOURCE_REPOSITORY_AUTHOR_EMAIL.to_owned(),
            registry_index_path: PathBuf::from(DEFAULT_REGISTRY_INDEX_PATH),
            compiler_node_bin: DEFAULT_COMPILER_NODE_BIN.to_owned(),
            compiler_worker_path: PathBuf::from(DEFAULT_COMPILER_WORKER_PATH),
            compiler_timeout: Duration::from_millis(DEFAULT_COMPILER_TIMEOUT_MS),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TonNetwork {
    Mainnet,
    Testnet,
    Localnet,
}

impl TonNetwork {
    #[must_use]
    pub const fn uses_testnet_address_format(self) -> bool {
        matches!(self, Self::Testnet | Self::Localnet)
    }

    const fn default_toncenter_base_url(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_TONCENTER_BASE_URL,
            Self::Testnet => TESTNET_TONCENTER_BASE_URL,
            Self::Localnet => LOCALNET_TONCENTER_BASE_URL,
        }
    }
}

impl fmt::Display for TonNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mainnet => formatter.write_str("mainnet"),
            Self::Testnet => formatter.write_str("testnet"),
            Self::Localnet => formatter.write_str("localnet"),
        }
    }
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
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    logging: LoggingConfig,
    #[serde(default)]
    network: NetworkConfig,
    #[serde(default)]
    toncenter: ToncenterConfig,
    #[serde(default)]
    source_repository: SourceRepositoryConfig,
    #[serde(default)]
    registry_index: RegistryIndexConfig,
    #[serde(default)]
    compiler: CompilerConfig,
}

impl ConfigFile {
    fn into_config(self) -> Config {
        Config {
            bind_addr: self.server.bind_addr.unwrap_or_else(default_bind_addr),
            logging_level: self
                .logging
                .level
                .unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_owned()),
            network: self.network.name.unwrap_or(TonNetwork::Mainnet),
            toncenter_base_url: self.toncenter.base_url,
            toncenter_api_key: self.toncenter.api_key,
            source_repository_path: self.source_repository.path,
            source_repository_remote: self
                .source_repository
                .remote
                .unwrap_or_else(|| DEFAULT_SOURCE_REPOSITORY_REMOTE.to_owned()),
            source_repository_branch: self.source_repository.branch,
            source_repository_commit_enabled: self
                .source_repository
                .commit_enabled
                .unwrap_or(DEFAULT_SOURCE_REPOSITORY_COMMIT_ENABLED),
            source_repository_push_enabled: self
                .source_repository
                .push_enabled
                .unwrap_or(DEFAULT_SOURCE_REPOSITORY_PUSH_ENABLED),
            source_repository_author_name: self
                .source_repository
                .author_name
                .unwrap_or_else(|| DEFAULT_SOURCE_REPOSITORY_AUTHOR_NAME.to_owned()),
            source_repository_author_email: self
                .source_repository
                .author_email
                .unwrap_or_else(|| DEFAULT_SOURCE_REPOSITORY_AUTHOR_EMAIL.to_owned()),
            registry_index_path: self
                .registry_index
                .path
                .unwrap_or_else(|| PathBuf::from(DEFAULT_REGISTRY_INDEX_PATH)),
            compiler_node_bin: self
                .compiler
                .node_bin
                .unwrap_or_else(|| DEFAULT_COMPILER_NODE_BIN.to_owned()),
            compiler_worker_path: self
                .compiler
                .worker_path
                .unwrap_or_else(|| PathBuf::from(DEFAULT_COMPILER_WORKER_PATH)),
            compiler_timeout: Duration::from_millis(
                self.compiler
                    .timeout_ms
                    .unwrap_or(DEFAULT_COMPILER_TIMEOUT_MS),
            ),
        }
    }
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
struct NetworkConfig {
    name: Option<TonNetwork>,
}

#[derive(Debug, Default, Deserialize)]
struct ToncenterConfig {
    base_url: Option<String>,
    api_key: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SourceRepositoryConfig {
    path: Option<PathBuf>,
    remote: Option<String>,
    branch: Option<String>,
    commit_enabled: Option<bool>,
    push_enabled: Option<bool>,
    author_name: Option<String>,
    author_email: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryIndexConfig {
    path: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
struct CompilerConfig {
    node_bin: Option<String>,
    worker_path: Option<PathBuf>,
    timeout_ms: Option<u64>,
}

const fn default_bind_addr() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000))
}
