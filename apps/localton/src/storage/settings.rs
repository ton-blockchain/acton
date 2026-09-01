use std::{
    collections::BTreeMap,
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::storage::{
    DHT_PORT, LITESERVER_PORT, OUT_PORT, VALIDATOR_ADNL_PORT, VALIDATOR_CONSOLE_PORT,
};

pub const SETTINGS_SCHEMA_VERSION: u32 = 2;

/// Persistent settings for one Full localnet
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// Version of this settings format
    pub schema_version: u32,
    /// Validated TON installation used to initialize and operate a joined node
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<String>)]
    pub ton_bin_dir: Option<PathBuf>,
    /// TON network parameters
    pub network: NetworkSettings,
    /// TON node owned by this state directory
    pub node: NodeSettings,
    /// Localton HTTP services
    pub services: ServiceSettings,
    /// Validator automation settings
    pub validation: ValidationSettings,
    /// Runtime monitoring settings
    pub monitoring: MonitoringSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            ton_bin_dir: None,
            network: NetworkSettings::default(),
            node: NodeSettings::genesis(),
            services: ServiceSettings::default(),
            validation: ValidationSettings::default(),
            monitoring: MonitoringSettings::default(),
        }
    }
}

impl Settings {
    /// Creates settings for a state directory that owns one joined node.
    ///
    /// Join state does not own genesis or bootstrap HTTP services. Keeping those
    /// services disabled prevents commands from treating a remote bootstrap host
    /// as locally managed infrastructure.
    #[must_use]
    pub fn for_join(node: NodeSettings) -> Self {
        let mut settings = Self {
            node,
            ..Self::default()
        };

        settings.services.config_http.enabled = false;
        settings.services.admin_http.enabled = false;
        settings.services.ton_http_api.enabled = false;

        settings
    }

    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.is_file() {
            return Self::load(path);
        }
        let settings = Self::default();
        settings.save_atomic(path)?;
        Ok(settings)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read settings {}", path.display()))?;
        let settings: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid settings {}", path.display()))?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .context("settings path has no parent directory")?;
        fs::create_dir_all(parent)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == SETTINGS_SCHEMA_VERSION,
            "unsupported settings schema {}, expected {}",
            self.schema_version,
            SETTINGS_SCHEMA_VERSION
        );
        self.network.validate()?;
        self.services.validate()?;
        self.validation.validate()?;
        self.monitoring.validate()?;
        self.node.validate()?;

        let mut tcp_ports = BTreeMap::new();
        let mut udp_ports = BTreeMap::new();
        for (kind, port) in [
            ("console", self.node.console_port),
            ("liteserver", self.node.liteserver_port),
        ] {
            if tcp_ports
                .insert(port, format!("{} {kind}", self.node.name))
                .is_some()
            {
                bail!("duplicate TCP port {port}");
            }
        }
        for (kind, port) in [
            ("ADNL", self.node.adnl_port),
            ("out", self.node.out_port),
            ("DHT", self.node.dht_port),
        ] {
            if udp_ports
                .insert(port, format!("{} {kind}", self.node.name))
                .is_some()
            {
                bail!("duplicate UDP port {port}");
            }
        }
        let mut service_ports = Vec::new();
        if self.services.config_http.enabled {
            service_ports.push(("config HTTP", self.services.config_http.port));
        }
        if self.services.admin_http.enabled {
            service_ports.push(("admin HTTP", self.services.admin_http.port));
        }
        if self.services.ton_http_api.enabled {
            service_ports.extend([
                ("TON HTTP API public proxy", self.services.ton_http_api.port),
                (
                    "TON HTTP API backend",
                    self.services.ton_http_api.backend_port,
                ),
                (
                    "TON HTTP API monitor",
                    self.services.ton_http_api.monitor_port,
                ),
            ]);
        }
        for (kind, port) in service_ports {
            if tcp_ports.insert(port, kind.to_owned()).is_some() {
                bail!("duplicate TCP port {port}");
            }
        }
        Ok(())
    }
}

/// TON protocol parameters for the local network
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct NetworkSettings {
    /// Negative TON global network identifier
    pub global_id: i32,
    /// `true` when the base workchain is enabled
    pub workchain_enabled: bool,
    /// Initial network balance
    pub initial_balance: u64,
    /// Base gas price for the workchain
    pub gas_price: u64,
    /// Base gas price for the masterchain
    pub gas_price_masterchain: u64,
    /// Base cell storage price for the workchain
    pub cell_price: u64,
    /// Base cell storage price for the masterchain
    pub cell_price_masterchain: u64,
    /// Maximum number of validators
    pub max_validators: u32,
    /// Maximum number of masterchain validators
    pub max_masterchain_validators: u32,
    /// Minimum number of validators
    pub min_validators: u32,
    /// Minimum validator stake
    pub min_validator_stake: u64,
    /// Maximum validator stake
    pub max_validator_stake: u64,
    /// Minimum total validator stake
    pub min_total_validator_stake: u64,
    /// Maximum stake ratio for one validator
    pub max_stake_factor: u32,
    /// Validator-set lifetime in seconds
    pub elected_for_seconds: u32,
    /// Time before the validator-set change when elections start
    pub election_start_before_seconds: u32,
    /// Time before the validator-set change when elections end
    pub election_end_before_seconds: u32,
    /// Stake freeze time in seconds
    pub stakes_frozen_for_seconds: u32,
    /// Original validator-set lifetime in seconds
    pub original_validator_set_valid_for_seconds: u32,
    /// Target Simplex block interval stored as noncritical config 30 key 0
    pub simplex_target_rate_ms: u32,
    /// Number of slots in one Simplex leader window
    pub simplex_slots_per_leader_window: u32,
    /// Timeout for the first block in milliseconds
    pub simplex_first_block_timeout_ms: u32,
    /// Maximum number of future Simplex leader windows accepted from peers
    pub simplex_max_leader_window_desync: u32,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            global_id: -3,
            workchain_enabled: true,
            initial_balance: 4_999_990_000,
            gas_price: 26_214_400,
            gas_price_masterchain: 655_360_000,
            cell_price: 2_621_440_000,
            cell_price_masterchain: 65_536_000_000,
            max_validators: 1_000,
            max_masterchain_validators: 100,
            min_validators: 1,
            min_validator_stake: 10_000,
            max_validator_stake: 10_000_000,
            min_total_validator_stake: 10_000,
            max_stake_factor: 3,
            elected_for_seconds: 2 * 60,
            election_start_before_seconds: 90,
            election_end_before_seconds: 30,
            stakes_frozen_for_seconds: 30,
            original_validator_set_valid_for_seconds: 90,
            // Localton intentionally runs slower than the current 400ms TON
            // mainnet target so block-by-block debugging remains practical
            simplex_target_rate_ms: 1_000,
            // Keep the remaining consensus shape aligned with masterchain:
            // four slots per leader, a 700ms first-block timeout, and TON's
            // default allowance of 250 future leader windows
            simplex_slots_per_leader_window: 4,
            simplex_first_block_timeout_ms: 700,
            simplex_max_leader_window_desync: 250,
        }
    }
}

impl NetworkSettings {
    /// Configures a validator round and its dependent genesis timings
    ///
    /// TON config parameter 15 expresses the election window relative to the
    /// validator-set change. Localton opens the window after one quarter of the
    /// round has elapsed, closes it one quarter before the change, and uses the
    /// same quarter for stake freezing. The initial validator-set lifetime equals
    /// the opening offset, which lets the first election begin immediately
    pub fn set_election_time_seconds(&mut self, election_time_seconds: u32) -> Result<()> {
        ensure!(
            election_time_seconds >= 4,
            "election time must be at least 4 seconds"
        );

        let quarter = election_time_seconds / 4;
        let election_start_before = election_time_seconds - quarter;
        self.elected_for_seconds = election_time_seconds;
        self.election_start_before_seconds = election_start_before;
        self.election_end_before_seconds = quarter;
        self.stakes_frozen_for_seconds = quarter;
        self.original_validator_set_valid_for_seconds = election_start_before;
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.global_id < 0,
            "local network global_id must be negative"
        );
        ensure!(self.min_validators > 0, "min_validators must be positive");
        ensure!(
            self.min_validators <= self.max_masterchain_validators
                && self.max_masterchain_validators <= self.max_validators,
            "validator count limits are inconsistent"
        );
        ensure!(
            self.min_validator_stake <= self.max_validator_stake,
            "validator stake limits are inconsistent"
        );
        ensure!(
            self.election_end_before_seconds < self.election_start_before_seconds,
            "election_end_before_seconds must be below election_start_before_seconds"
        );
        ensure!(
            self.election_start_before_seconds < self.elected_for_seconds,
            "election_start_before_seconds must be below elected_for_seconds"
        );
        ensure!(
            self.simplex_target_rate_ms > 0
                && self.simplex_slots_per_leader_window > 0
                && self.simplex_first_block_timeout_ms > 0
                && self.simplex_max_leader_window_desync > 0,
            "simplex consensus values must be positive"
        );
        Ok(())
    }
}

/// Persistent settings for one TON node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct NodeSettings {
    /// Lifecycle role that determines whether this state owns network bootstrap artifacts
    pub role: NodeRole,
    /// Stable node name
    pub name: String,
    /// `true` when Localton starts the node
    pub enabled: bool,
    /// `true` when the node validates blocks
    pub validator: bool,
    /// `true` when the node provides a liteserver
    pub liteserver: bool,
    /// Public IPv4 address that Localton writes to the network config
    #[schema(value_type = String, format = "ipv4")]
    pub public_ip: Ipv4Addr,
    /// TCP port for the validator console
    pub console_port: u16,
    /// UDP port for validator ADNL
    pub adnl_port: u16,
    /// TCP port for the liteserver
    pub liteserver_port: u16,
    /// UDP port for outbound ADNL traffic
    pub out_port: u16,
    /// UDP port for DHT traffic
    pub dht_port: u16,
    /// Number of validator-engine threads
    pub threads: u16,
    /// Validator-engine log level
    pub verbosity: u8,
    /// Block time range that the node synchronizes before startup
    pub sync_before_seconds: u64,
    /// State lifetime in seconds
    pub state_ttl_seconds: u64,
    /// Block lifetime in seconds
    pub block_ttl_seconds: u64,
    /// Archive lifetime in seconds
    pub archive_ttl_seconds: u64,
    /// Validator key-proof lifetime in seconds
    pub key_proof_ttl_seconds: u64,
    /// Initial wallet balance in nanotons
    pub initial_wallet_amount_nano: u64,
    /// Validator stake in nanotons
    pub validator_stake_nano: u64,
    /// `true` when Localton enters this validator into elections
    pub participate_in_elections: bool,
}

/// Filesystem and initialization role of the node owned by one state directory.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    /// Validator that creates and anchors a new local network
    #[default]
    Genesis,
    /// Full node that joins an already existing network
    Joined,
}

/// Ports assigned together to one validator-engine process.
///
/// The host allocator deals in this complete type so initialization cannot
/// accidentally mix ports from different candidate ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodePorts {
    /// Authenticated validator-console TCP port
    pub console: u16,
    /// Public ADNL UDP port
    pub adnl: u16,
    /// Local liteserver TCP port
    pub liteserver: u16,
    /// Validator-engine outbound UDP port
    pub out: u16,
    /// DHT UDP port
    pub dht: u16,
}

impl Default for NodeSettings {
    fn default() -> Self {
        Self::genesis()
    }
}

impl NodeSettings {
    /// Creates the genesis validator owned by the bootstrap workflow.
    #[must_use]
    pub fn genesis() -> Self {
        Self {
            role: NodeRole::Genesis,
            name: "genesis".to_owned(),
            enabled: true,
            validator: true,
            liteserver: true,
            public_ip: Ipv4Addr::LOCALHOST,
            console_port: VALIDATOR_CONSOLE_PORT,
            adnl_port: VALIDATOR_ADNL_PORT,
            liteserver_port: LITESERVER_PORT,
            out_port: OUT_PORT,
            dht_port: DHT_PORT,
            threads: 4,
            verbosity: 2,
            sync_before_seconds: 3_600,
            state_ttl_seconds: 365 * 86_400,
            block_ttl_seconds: 365 * 86_400,
            archive_ttl_seconds: 365 * 86_400,
            key_proof_ttl_seconds: 10 * 365 * 86_400,
            initial_wallet_amount_nano: 50_005_000_000_000,
            validator_stake_nano: 10_001_000_000_000,
            participate_in_elections: true,
        }
    }

    /// Creates a joined node with one complete, already validated port set.
    ///
    /// Protocol retention and wallet defaults remain aligned with genesis, while
    /// validator participation is opt-in and configured by the join workflow.
    #[must_use]
    pub fn joined(name: String, public_ip: Ipv4Addr, ports: NodePorts) -> Self {
        Self {
            role: NodeRole::Joined,
            name,
            enabled: true,
            validator: false,
            public_ip,
            console_port: ports.console,
            adnl_port: ports.adnl,
            liteserver_port: ports.liteserver,
            out_port: ports.out,
            dht_port: ports.dht,
            verbosity: 1,
            participate_in_elections: false,
            ..Self::genesis()
        }
    }

    fn validate(&self) -> Result<()> {
        match self.role {
            NodeRole::Genesis => ensure!(
                self.name == "genesis",
                "genesis node must be named `genesis`"
            ),
            NodeRole::Joined => ensure!(
                self.name != "genesis",
                "joined node cannot be named `genesis`"
            ),
        }
        ensure!(
            !self.name.is_empty()
                && self
                    .name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'),
            "invalid node name {}",
            self.name
        );
        for (name, port) in [
            ("console", self.console_port),
            ("ADNL", self.adnl_port),
            ("liteserver", self.liteserver_port),
            ("out", self.out_port),
            ("DHT", self.dht_port),
        ] {
            ensure!(port > 0, "{} {name} port must be positive", self.name);
        }
        ensure!(self.threads > 0, "{} threads must be positive", self.name);
        ensure!(
            self.validator_stake_nano > 0,
            "{} validator stake must be positive",
            self.name
        );
        Ok(())
    }
}

/// Settings for Localton HTTP services
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct ServiceSettings {
    /// Config API settings
    pub config_http: HttpServiceSettings,
    /// Admin API settings
    pub admin_http: HttpServiceSettings,
    /// TON HTTP API v2 settings
    pub ton_http_api: TonHttpApiSettings,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            config_http: HttpServiceSettings {
                enabled: true,
                bind: Ipv4Addr::LOCALHOST,
                port: 18_000,
            },
            admin_http: HttpServiceSettings {
                enabled: true,
                bind: Ipv4Addr::LOCALHOST,
                port: 18_001,
            },
            ton_http_api: TonHttpApiSettings {
                enabled: false,
                port: 18_002,
                backend_port: 18_005,
                monitor_port: 18_006,
                command: None,
                static_config: None,
            },
        }
    }
}

impl ServiceSettings {
    fn validate(&self) -> Result<()> {
        self.config_http.validate("config HTTP")?;
        self.admin_http.validate("admin HTTP")?;
        self.ton_http_api.validate("TON HTTP API")?;
        Ok(())
    }
}

/// Bind settings for one HTTP service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct HttpServiceSettings {
    /// `true` when Localton starts the service
    pub enabled: bool,
    /// IPv4 address that accepts service connections
    #[schema(value_type = String, format = "ipv4")]
    pub bind: Ipv4Addr,
    /// TCP port for the service
    pub port: u16,
}

impl Default for HttpServiceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            bind: Ipv4Addr::LOCALHOST,
            port: 18_000,
        }
    }
}

impl HttpServiceSettings {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(self.bind), self.port)
    }

    fn validate(&self, name: &str) -> Result<()> {
        ensure!(self.port > 0, "{name} port must be positive");
        Ok(())
    }
}

/// Settings for the TON HTTP API v2 service
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct TonHttpApiSettings {
    /// `true` when Localton starts the TON HTTP API
    pub enabled: bool,
    /// Browser-facing port for the Rust CORS and PNA proxy
    pub port: u16,
    /// Loopback-only port for the TON HTTP API v2 backend
    pub backend_port: u16,
    /// Port for the TON HTTP API monitor
    pub monitor_port: u16,
    /// Path of the TON HTTP API executable
    #[schema(value_type = Option<String>)]
    pub command: Option<PathBuf>,
    /// Path of the static TON HTTP API config
    #[schema(value_type = Option<String>)]
    pub static_config: Option<PathBuf>,
}

impl Default for TonHttpApiSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 18_002,
            backend_port: 18_005,
            monitor_port: 18_006,
            command: None,
            static_config: None,
        }
    }
}

impl TonHttpApiSettings {
    fn validate(&self, name: &str) -> Result<()> {
        ensure!(self.port > 0, "{name} port must be positive");
        ensure!(
            self.backend_port > 0,
            "{name} backend port must be positive"
        );
        ensure!(
            self.port != self.backend_port,
            "{name} public and backend ports must differ"
        );
        ensure!(
            self.monitor_port > 0,
            "{name} monitor port must be positive"
        );
        ensure!(
            self.port != self.monitor_port && self.backend_port != self.monitor_port,
            "{name} public, backend, and monitor ports must differ"
        );
        Ok(())
    }
}

/// Settings for validator automation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct ValidationSettings {
    /// `true` when Localton enters enabled validators into elections
    pub auto_participate: bool,
    /// `true` when Localton withdraws available validator funds
    pub auto_reap: bool,
    /// Validator state poll interval in seconds
    pub poll_interval_seconds: u64,
    /// Maximum stake factor that Localton sends to elections
    pub max_factor: f64,
    /// Amount that Localton leaves in the elector contract, in nanotons
    pub reap_value_nano: u64,
}

impl Default for ValidationSettings {
    fn default() -> Self {
        Self {
            auto_participate: true,
            auto_reap: true,
            poll_interval_seconds: 5,
            max_factor: 3.0,
            reap_value_nano: 1_000_000_000,
        }
    }
}

impl ValidationSettings {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.poll_interval_seconds > 0,
            "validation poll interval must be positive"
        );
        ensure!(
            (1.0..=100.0).contains(&self.max_factor),
            "validation max_factor must be in 1..=100"
        );
        Ok(())
    }
}

/// Settings for runtime monitoring
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct MonitoringSettings {
    /// `true` when Localton monitors node state
    pub enabled: bool,
    /// Node-state poll interval in seconds
    pub poll_interval_seconds: u64,
}

impl Default for MonitoringSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_seconds: 2,
        }
    }
}

impl MonitoringSettings {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.poll_interval_seconds > 0,
            "monitor poll interval must be positive"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_defaults_contain_genesis() {
        let settings = Settings::default();
        settings.validate().unwrap();
        assert_eq!(settings.network.global_id, -3);
        assert_eq!(settings.node.name, "genesis");
        assert!(settings.node.participate_in_elections);
        assert_eq!(settings.services.config_http.port, 18_000);
        assert_eq!(settings.services.admin_http.port, 18_001);
        assert_eq!(settings.services.ton_http_api.port, 18_002);
        assert_eq!(settings.services.ton_http_api.backend_port, 18_005);
        assert_eq!(settings.services.ton_http_api.monitor_port, 18_006);
    }

    #[test]
    fn default_validator_round_is_two_minutes() {
        let settings = Settings::default();
        assert_eq!(settings.network.elected_for_seconds, 120);
        assert_eq!(settings.network.election_start_before_seconds, 90);
        assert_eq!(settings.network.election_end_before_seconds, 30);
        assert_eq!(
            settings.network.original_validator_set_valid_for_seconds,
            90
        );
        assert_eq!(settings.validation.poll_interval_seconds, 5);
    }

    #[test]
    fn election_time_configures_consistent_genesis_windows() {
        let mut network = NetworkSettings::default();
        network.set_election_time_seconds(240).unwrap();

        assert_eq!(network.elected_for_seconds, 240);
        assert_eq!(network.election_start_before_seconds, 180);
        assert_eq!(network.election_end_before_seconds, 60);
        assert_eq!(network.stakes_frozen_for_seconds, 60);
        assert_eq!(network.original_validator_set_valid_for_seconds, 180);
        assert!(network.set_election_time_seconds(3).is_err());
        network.validate().unwrap();
    }

    #[test]
    fn http_services_can_bind_to_all_container_interfaces() {
        let mut settings = Settings::default();
        settings.services.config_http.bind = Ipv4Addr::UNSPECIFIED;
        settings.services.admin_http.bind = Ipv4Addr::UNSPECIFIED;
        settings.validate().unwrap();
    }

    #[test]
    fn duplicate_ports_are_rejected() {
        let mut settings = Settings::default();
        settings.services.config_http.port = settings.node.console_port;
        assert!(settings.validate().is_err());
    }
}
