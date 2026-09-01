//! Typed TON global configuration used by bootstrap and HTTP discovery.
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

use super::tools::types::{
    DhtNodeDescriptor, Ed25519PublicKey, TonBlockHash, TonPublicKey, ZeroStateId,
};

const MASTERCHAIN_ID: i32 = -1;
const MASTERCHAIN_SHARD: i64 = i64::MIN;

/// An on-disk global config validated before it is passed to TON processes.
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
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read global config {}", path.display()))?;
        GlobalConfig::from_json_bytes(&bytes)?.validate_network_entry_points()?;
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

    /// Parses and validates a global config before Localton persists or serves it.
    pub(crate) fn from_json_bytes(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).context("global config does not match the TON JSON schema")
    }

    /// Checks that the final config contains discovery data required by TON clients.
    pub(crate) fn validate_network_entry_points(&self) -> Result<()> {
        ensure!(
            !self.dht.static_nodes.nodes.is_empty(),
            "global config has no DHT entry points"
        );
        Ok(())
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
    }
}
