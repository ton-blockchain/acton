//! Typed TON global configuration used by bootstrap, join, and HTTP discovery.
//!
//! This module owns the JSON protocol shape instead of letting workflows assemble
//! arbitrary `serde_json::Value` trees. Constructor names, byte lengths, and required
//! network entry points are therefore checked before a config reaches official TON
//! binaries or is persisted as network identity.

use std::{
    fs,
    net::Ipv4Addr,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::storage::write_json_atomic;

use super::tools::types::{
    DhtNodeDescriptor, Ed25519PublicKey, TonBlockHash, TonPublicKey, ZeroStateId, is_public_ipv4,
};

const MASTERCHAIN_ID: i32 = -1;
const MASTERCHAIN_SHARD: i64 = i64::MIN;

/// An on-disk global config that was parsed and accepted for joining a TON node.
///
/// Official binaries consume a filename, not a Rust value. This type keeps that
/// filename while proving once, before any child starts, that the file matches
/// the complete typed schema and contains at least one DHT entry point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GlobalConfigFile {
    path: PathBuf,
}

impl GlobalConfigFile {
    /// Opens the final network config used by persistent TON processes.
    pub(crate) fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        GlobalConfig::load(&path)?.validate_for_node_join()?;
        Ok(Self { path })
    }

    /// Returns the already validated file expected by official TON binaries.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Complete `global.config.json` document consumed by TON nodes and clients.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GlobalConfig {
    #[serde(rename = "@type")]
    constructor: GlobalConfigConstructor,
    dht: DhtConfig,
    liteservers: Vec<LiteserverConfig>,
    validator: ValidatorConfig,
}

impl GlobalConfig {
    /// Reads one typed global config while preserving the source path in diagnostics.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read global config {}", path.display()))?;
        Self::from_json_bytes(&bytes)
            .with_context(|| format!("invalid global config {}", path.display()))
    }

    /// Atomically persists a typed config so readers never observe a partial JSON file.
    pub(crate) fn save_atomic(&self, path: &Path) -> Result<()> {
        write_json_atomic(path, self)
            .with_context(|| format!("failed to save global config {}", path.display()))
    }

    /// Returns authenticated liteserver endpoints in their configured priority order.
    pub(crate) fn liteserver_endpoints(
        &self,
    ) -> impl ExactSizeIterator<Item = (Ipv4Addr, u16, TonPublicKey)> + '_ {
        self.liteservers.iter().map(|liteserver| {
            (
                Ipv4Addr::from(liteserver.ip as u32),
                liteserver.port,
                liteserver.id.public_key(),
            )
        })
    }

    /// Builds the single-liteserver config that defines a fresh Localton network.
    ///
    /// The zerostate is also the initial block until validator-engine produces the
    /// first masterchain block. DHT nodes can be empty only for the preliminary file
    /// used to initialize the DHT database during genesis bootstrap.
    pub(crate) fn local(
        zero_state: ZeroStateId,
        dht_nodes: Vec<DhtNodeDescriptor>,
        liteserver_ip: Ipv4Addr,
        liteserver_port: u16,
        liteserver_public_key: TonPublicKey,
    ) -> Self {
        let zero_state = BlockIdExt::zerostate(zero_state);
        Self {
            constructor: GlobalConfigConstructor::Global,
            dht: DhtConfig::local(dht_nodes),
            liteservers: vec![LiteserverConfig::new(
                liteserver_ip,
                liteserver_port,
                liteserver_public_key,
            )],
            validator: ValidatorConfig {
                constructor: ValidatorConfigConstructor::Global,
                zero_state: zero_state.clone(),
                init_block: zero_state,
                hardforks: Vec::new(),
            },
        }
    }

    /// Parses and fully validates a downloaded global config.
    pub(crate) fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).context("global config does not match the TON JSON schema")
    }

    /// Checks the network entry points required by the complete join workflow.
    ///
    /// Validator-engine discovers peers through DHT, while Localton compares its
    /// node with an authenticated upstream liteserver before publishing readiness.
    pub(crate) fn validate_for_node_join(&self) -> Result<()> {
        ensure!(
            !self.dht.static_nodes.nodes.is_empty(),
            "global config has no DHT entry points"
        );
        ensure!(
            !self.liteservers.is_empty(),
            "global config has no liteserver endpoints"
        );
        Ok(())
    }

    /// Rejects a host address that public TON overlay peers cannot reach.
    ///
    /// DHT queries may succeed from behind NAT even when full-node replies are sent
    /// to an advertised loopback or private address. Failing before validator-engine
    /// starts avoids an indefinite `process.initial_sync` proof-download loop.
    pub(crate) fn validate_advertise_ip(&self, advertise_ip: Ipv4Addr) -> Result<()> {
        let public_network = self
            .dht
            .static_nodes
            .nodes
            .iter()
            .any(DhtNodeDescriptor::advertises_public_ipv4);

        if public_network {
            ensure!(
                is_public_ipv4(advertise_ip),
                "joining a public TON network requires a publicly reachable \
                 --advertise-ip, but {advertise_ip} is not public; forward the node's \
                 ADNL UDP port to a static public IPv4 address"
            );
        }

        Ok(())
    }

    /// Routes this host's chain operations through its own liteserver identity.
    ///
    /// DHT and validator sections remain untouched, so the node still joins the
    /// exact network described by the downloaded config.
    pub(crate) fn with_local_liteserver(mut self, port: u16, public_key: TonPublicKey) -> Self {
        self.liteservers = vec![LiteserverConfig::new(Ipv4Addr::LOCALHOST, port, public_key)];
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum GlobalConfigConstructor {
    #[serde(rename = "config.global")]
    Global,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct DhtConfig {
    #[serde(rename = "@type")]
    constructor: DhtConfigConstructor,
    k: i32,
    a: i32,
    static_nodes: DhtNodes,
}

impl DhtConfig {
    fn local(nodes: Vec<DhtNodeDescriptor>) -> Self {
        Self {
            constructor: DhtConfigConstructor::Global,
            k: 3,
            a: 3,
            static_nodes: DhtNodes {
                constructor: DhtNodesConstructor::Nodes,
                nodes,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum DhtConfigConstructor {
    #[serde(rename = "dht.config.global")]
    Global,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct DhtNodes {
    #[serde(rename = "@type")]
    constructor: DhtNodesConstructor,
    nodes: Vec<DhtNodeDescriptor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum DhtNodesConstructor {
    #[serde(rename = "dht.nodes")]
    Nodes,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct LiteserverConfig {
    id: Ed25519PublicKey,
    ip: i32,
    port: u16,
}

impl LiteserverConfig {
    fn new(ip: Ipv4Addr, port: u16, public_key: TonPublicKey) -> Self {
        Self {
            id: Ed25519PublicKey::new(public_key),
            ip: i32::from_be_bytes(ip.octets()),
            port,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct ValidatorConfig {
    #[serde(rename = "@type")]
    constructor: ValidatorConfigConstructor,
    zero_state: BlockIdExt,
    init_block: BlockIdExt,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hardforks: Vec<BlockIdExt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum ValidatorConfigConstructor {
    #[serde(rename = "validator.config.global")]
    Global,
}

/// Extended TON block identifier embedded into validator configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
struct BlockIdExt {
    #[serde(rename = "@type", default, skip_serializing_if = "Option::is_none")]
    constructor: Option<BlockIdExtConstructor>,
    workchain: i32,
    shard: i64,
    seqno: u32,
    root_hash: TonBlockHash,
    file_hash: TonBlockHash,
}

impl BlockIdExt {
    fn zerostate(state: ZeroStateId) -> Self {
        Self {
            constructor: Some(BlockIdExtConstructor::BlockIdExt),
            workchain: MASTERCHAIN_ID,
            shard: MASTERCHAIN_SHARD,
            seqno: 0,
            root_hash: state.root_hash(),
            file_hash: state.file_hash(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize, ToSchema)]
enum BlockIdExtConstructor {
    #[serde(rename = "ton.blockIdExt")]
    BlockIdExt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_config_round_trips_through_the_typed_schema() {
        let config = GlobalConfig::local(
            ZeroStateId::new(
                TonBlockHash::from_bytes([1; 32]),
                TonBlockHash::from_bytes([2; 32]),
            ),
            Vec::new(),
            Ipv4Addr::new(192, 168, 27, 4),
            18_004,
            TonPublicKey::from_bytes([3; 32]),
        );
        let json = serde_json::to_vec(&config).unwrap();
        let decoded = GlobalConfig::from_json_bytes(&json).unwrap();

        assert_eq!(decoded, config);
        decoded
            .validate_advertise_ip(Ipv4Addr::new(192, 168, 27, 4))
            .unwrap();
    }

    #[test]
    fn parses_the_repository_mainnet_fixture() {
        let config = GlobalConfig::from_json_bytes(include_bytes!(
            "../../../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json"
        ))
        .unwrap();

        config.validate_for_node_join().unwrap();
        assert!(config.validate_advertise_ip(Ipv4Addr::LOCALHOST).is_err());
        config
            .validate_advertise_ip(Ipv4Addr::new(203, 12, 34, 56))
            .unwrap();
    }

    #[test]
    fn join_requires_an_upstream_liteserver() {
        let mut config = GlobalConfig::from_json_bytes(include_bytes!(
            "../../../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json"
        ))
        .unwrap();
        config.liteservers.clear();

        assert_eq!(
            config.validate_for_node_join().unwrap_err().to_string(),
            "global config has no liteserver endpoints"
        );
    }
}
