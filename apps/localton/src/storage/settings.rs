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

pub const SETTINGS_SCHEMA_VERSION: u32 = 1;
pub const MAX_LOCAL_NODES: usize = 7;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub network: NetworkSettings,
    pub nodes: Vec<NodeSettings>,
    pub services: ServiceSettings,
    pub validation: ValidationSettings,
    pub monitoring: MonitoringSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            network: NetworkSettings::default(),
            nodes: default_nodes(),
            services: ServiceSettings::default(),
            validation: ValidationSettings::default(),
            monitoring: MonitoringSettings::default(),
        }
    }
}

impl Settings {
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
        ensure!(
            !self.nodes.is_empty(),
            "settings must contain a genesis node"
        );
        ensure!(
            self.nodes.len() <= MAX_LOCAL_NODES,
            "at most {MAX_LOCAL_NODES} local nodes are supported"
        );
        ensure!(
            self.nodes[0].name == "genesis",
            "the first node must be named genesis"
        );
        ensure!(
            self.nodes[0].enabled && self.nodes[0].validator,
            "genesis must be an enabled validator"
        );
        self.network.validate()?;
        self.services.validate()?;
        self.validation.validate()?;
        self.monitoring.validate()?;

        let mut names = BTreeMap::new();
        let mut tcp_ports = BTreeMap::new();
        let mut udp_ports = BTreeMap::new();
        for node in &self.nodes {
            node.validate()?;
            if names.insert(node.name.clone(), ()).is_some() {
                bail!("duplicate node name {}", node.name);
            }
            for (kind, port) in [
                ("console", node.console_port),
                ("liteserver", node.liteserver_port),
            ] {
                if tcp_ports
                    .insert(port, format!("{} {kind}", node.name))
                    .is_some()
                {
                    bail!("duplicate TCP port {port}");
                }
            }
            for (kind, port) in [("ADNL", node.adnl_port), ("DHT", node.dht_port)] {
                if udp_ports
                    .insert(port, format!("{} {kind}", node.name))
                    .is_some()
                {
                    bail!("duplicate UDP port {port}");
                }
            }
        }
        let service_ports = vec![
            ("config HTTP", self.services.config_http.port),
            ("admin HTTP", self.services.admin_http.port),
            ("TON HTTP API public proxy", self.services.ton_http_api.port),
            (
                "TON HTTP API backend",
                self.services.ton_http_api.backend_port,
            ),
            (
                "TON HTTP API monitor",
                self.services.ton_http_api.monitor_port,
            ),
        ];
        for (kind, port) in service_ports {
            if tcp_ports.insert(port, kind.to_owned()).is_some() {
                bail!("duplicate TCP port {port}");
            }
        }
        Ok(())
    }

    pub fn node(&self, name: &str) -> Result<&NodeSettings> {
        self.nodes
            .iter()
            .find(|node| node.name == name)
            .with_context(|| format!("unknown node {name}"))
    }

    pub fn node_mut(&mut self, name: &str) -> Result<&mut NodeSettings> {
        self.nodes
            .iter_mut()
            .find(|node| node.name == name)
            .with_context(|| format!("unknown node {name}"))
    }

    pub fn enable_validator_count(&mut self, count: usize) -> Result<()> {
        ensure!(
            (1..=MAX_LOCAL_NODES).contains(&count),
            "validator count must be in 1..={MAX_LOCAL_NODES}"
        );
        for (index, node) in self.nodes.iter_mut().enumerate() {
            node.enabled = index < count;
            node.validator = index < count;
            node.participate_in_elections = index < count;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct NetworkSettings {
    pub global_id: i32,
    pub workchain_enabled: bool,
    pub initial_balance: u64,
    pub gas_price: u64,
    pub gas_price_masterchain: u64,
    pub cell_price: u64,
    pub cell_price_masterchain: u64,
    pub max_validators: u32,
    pub max_masterchain_validators: u32,
    pub min_validators: u32,
    pub min_validator_stake: u64,
    pub max_validator_stake: u64,
    pub min_total_validator_stake: u64,
    pub max_stake_factor: u32,
    pub elected_for_seconds: u32,
    pub election_start_before_seconds: u32,
    pub election_end_before_seconds: u32,
    pub stakes_frozen_for_seconds: u32,
    pub original_validator_set_valid_for_seconds: u32,
    pub simplex_target_rate_ms: u32,
    pub simplex_slots_per_leader_window: u32,
    pub simplex_first_block_timeout_ms: u32,
    pub simplex_max_leader_window_desync_ms: u32,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            global_id: -239,
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
            elected_for_seconds: 30 * 60,
            election_start_before_seconds: 25 * 60,
            election_end_before_seconds: 10 * 60,
            stakes_frozen_for_seconds: 5 * 60,
            original_validator_set_valid_for_seconds: 25 * 60,
            simplex_target_rate_ms: 300,
            simplex_slots_per_leader_window: 4,
            simplex_first_block_timeout_ms: 400,
            simplex_max_leader_window_desync_ms: 700,
        }
    }
}

impl NetworkSettings {
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
                && self.simplex_first_block_timeout_ms > 0,
            "simplex timing values must be positive"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct NodeSettings {
    pub name: String,
    pub enabled: bool,
    pub validator: bool,
    pub liteserver: bool,
    #[schema(value_type = String, format = "ipv4")]
    pub public_ip: Ipv4Addr,
    pub console_port: u16,
    pub adnl_port: u16,
    pub liteserver_port: u16,
    pub out_port: u16,
    pub dht_port: u16,
    pub threads: u16,
    pub verbosity: u8,
    pub sync_before_seconds: u64,
    pub state_ttl_seconds: u64,
    pub block_ttl_seconds: u64,
    pub archive_ttl_seconds: u64,
    pub key_proof_ttl_seconds: u64,
    pub initial_wallet_amount_nano: u64,
    pub validator_stake_nano: u64,
    pub participate_in_elections: bool,
}

impl Default for NodeSettings {
    fn default() -> Self {
        Self::for_index(0)
    }
}

impl NodeSettings {
    pub fn for_index(index: usize) -> Self {
        let index_u16 = u16::try_from(index).expect("local node index fits u16");
        let is_genesis = index == 0;
        Self {
            name: if is_genesis {
                "genesis".to_owned()
            } else {
                format!("node{}", index + 1)
            },
            enabled: is_genesis,
            validator: is_genesis,
            liteserver: true,
            public_ip: Ipv4Addr::LOCALHOST,
            console_port: VALIDATOR_CONSOLE_PORT + index_u16 * 3,
            adnl_port: VALIDATOR_ADNL_PORT + index_u16 * 3,
            liteserver_port: LITESERVER_PORT + index_u16 * 3,
            out_port: OUT_PORT + index_u16,
            dht_port: DHT_PORT + index_u16,
            threads: 4,
            verbosity: if is_genesis { 2 } else { 1 },
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

    fn validate(&self) -> Result<()> {
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

fn default_nodes() -> Vec<NodeSettings> {
    (0..MAX_LOCAL_NODES).map(NodeSettings::for_index).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct ServiceSettings {
    pub config_http: HttpServiceSettings,
    pub admin_http: HttpServiceSettings,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct HttpServiceSettings {
    pub enabled: bool,
    #[schema(value_type = String, format = "ipv4")]
    pub bind: Ipv4Addr,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct TonHttpApiSettings {
    pub enabled: bool,
    /// Browser-facing endpoint served by the Rust CORS/PNA proxy.
    pub port: u16,
    /// Loopback-only port used by the TON HTTP API V2 backend.
    pub backend_port: u16,
    pub monitor_port: u16,
    #[schema(value_type = Option<String>)]
    pub command: Option<PathBuf>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(default)]
pub struct ValidationSettings {
    pub auto_participate: bool,
    pub auto_reap: bool,
    pub poll_interval_seconds: u64,
    pub max_factor: f64,
    pub reap_value_nano: u64,
}

impl Default for ValidationSettings {
    fn default() -> Self {
        Self {
            auto_participate: true,
            auto_reap: true,
            poll_interval_seconds: 15,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(default)]
pub struct MonitoringSettings {
    pub enabled: bool,
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
    fn defaults_cover_all_local_node_slots() {
        let settings = Settings::default();
        settings.validate().unwrap();
        assert_eq!(settings.nodes.len(), 7);
        assert!(settings.nodes[0].participate_in_elections);
        assert_eq!(settings.nodes[6].name, "node7");
        assert_eq!(settings.services.config_http.port, 18_000);
        assert_eq!(settings.services.admin_http.port, 18_001);
        assert_eq!(settings.services.ton_http_api.port, 18_002);
        assert_eq!(settings.services.ton_http_api.backend_port, 18_005);
        assert_eq!(settings.services.ton_http_api.monitor_port, 18_006);
    }

    #[test]
    fn http_services_can_bind_to_all_container_interfaces() {
        let mut settings = Settings::default();
        settings.services.config_http.bind = Ipv4Addr::UNSPECIFIED;
        settings.services.admin_http.bind = Ipv4Addr::UNSPECIFIED;
        settings.validate().unwrap();
    }

    #[test]
    fn enabling_validator_count_updates_topology() {
        let mut settings = Settings::default();
        settings.enable_validator_count(3).unwrap();
        assert!(settings.nodes[0].enabled);
        assert!(settings.nodes[0].participate_in_elections);
        assert!(settings.nodes[1].validator);
        assert!(settings.nodes[2].participate_in_elections);
        assert!(!settings.nodes[3].enabled);
    }

    #[test]
    fn duplicate_ports_are_rejected() {
        let mut settings = Settings::default();
        settings.nodes[1].enabled = true;
        settings.nodes[1].console_port = settings.nodes[0].console_port;
        assert!(settings.validate().is_err());
    }
}
