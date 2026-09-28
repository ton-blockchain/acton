//! Reads the zerostate, starting block, and DHT entry points from a global config.

use std::{fs, path::Path};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use everscale_network::{crypto, overlay, proto};
use rston::{
    cell::HashBytes,
    models::{BlockId, ShardIdent},
};
use serde::{Deserialize, Deserializer, de::Error};

/// Network identity and DHT entry points from an operator-supplied global config.
///
/// Parsing validates block coordinates and descriptor fields. The transport
/// verifies DHT signatures when it adds these entry points.
#[derive(Deserialize)]
pub struct NetworkConfig {
    dht: DhtConfig,
    validator: ValidatorConfig,
}

impl NetworkConfig {
    /// Loads a global config. P2P-only starts do not require a `liteservers` field.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read global config {}", path.display()))?;

        Self::from_json(&bytes).with_context(|| format!("invalid global config {}", path.display()))
    }

    /// Parses the P2P subset while accepting unrelated standard config fields.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let config: Self = serde_json::from_slice(bytes)?;
        let zero_state = &config.validator.zero_state;

        ensure!(
            zero_state.workchain == -1 && zero_state.shard == i64::MIN && zero_state.seqno == 0,
            "global config must identify the masterchain zerostate"
        );
        ensure!(
            !config.dht.static_nodes.nodes.is_empty(),
            "global config has no DHT entry points"
        );

        if let Some(init_block) = &config.validator.init_block {
            ensure!(
                init_block.workchain == -1 && init_block.shard == i64::MIN,
                "global config init_block must belong to the masterchain"
            );
            ensure!(
                init_block.seqno != 0 || init_block.block_id() == zero_state.block_id(),
                "global config init_block at seqno zero must match the zerostate"
            );
        }

        Ok(config)
    }

    /// Derives the public masterchain overlay from the network's zerostate.
    #[must_use]
    pub fn masterchain_overlay(&self) -> overlay::IdShort {
        overlay::IdFull::for_workchain_overlay(-1, &self.validator.zero_state.file_hash)
            .compute_short_id()
    }

    /// Identifies the network in persistent storage, preventing cross-network resumes.
    #[must_use]
    pub const fn zero_state(&self) -> BlockId {
        self.validator.zero_state.block_id()
    }

    /// Returns the configured starting block, falling back to the zerostate.
    /// Its hashes are trusted; this client does not validate the history before it.
    #[must_use]
    pub fn initial_block(&self) -> BlockId {
        self.validator
            .init_block
            .as_ref()
            .unwrap_or(&self.validator.zero_state)
            .block_id()
    }

    /// Selects an operator-trusted starting block for a new download directory.
    /// Existing storage keeps its persisted anchor. The caller must establish
    /// the network identity; this only checks masterchain coordinates.
    pub fn set_initial_block(&mut self, block: BlockId) -> Result<()> {
        ensure!(
            block.shard == ShardIdent::MASTERCHAIN,
            "starting block must belong to the masterchain"
        );
        let block = ConfigBlockId {
            workchain: block.shard.workchain(),
            shard: block.shard.prefix() as i64,
            seqno: block.seqno,
            root_hash: block.root_hash.0,
            file_hash: block.file_hash.0,
        };
        ensure!(
            block.seqno != 0 || block.block_id() == self.zero_state(),
            "starting block at seqno zero must match the network zerostate"
        );

        self.validator.init_block = Some(block);
        Ok(())
    }

    pub(crate) fn dht_nodes(&self) -> impl Iterator<Item = Result<proto::dht::NodeOwned>> + '_ {
        self.dht.static_nodes.nodes.iter().map(DhtNode::to_wire)
    }
}

#[derive(Deserialize)]
struct DhtConfig {
    static_nodes: DhtNodes,
}

#[derive(Deserialize)]
struct DhtNodes {
    nodes: Vec<DhtNode>,
}

#[derive(Deserialize)]
struct ValidatorConfig {
    zero_state: ConfigBlockId,
    init_block: Option<ConfigBlockId>,
}

#[derive(Deserialize)]
struct ConfigBlockId {
    workchain: i32,
    shard: i64,
    seqno: u32,
    #[serde(deserialize_with = "hash_from_base64")]
    root_hash: [u8; 32],
    #[serde(deserialize_with = "hash_from_base64")]
    file_hash: [u8; 32],
}

impl ConfigBlockId {
    const fn block_id(&self) -> BlockId {
        BlockId {
            shard: ShardIdent::MASTERCHAIN,
            seqno: self.seqno,
            root_hash: HashBytes(self.root_hash),
            file_hash: HashBytes(self.file_hash),
        }
    }
}

#[derive(Deserialize)]
struct DhtNode {
    id: PublicKey,
    addr_list: AddressList,
    version: i32,
    #[serde(deserialize_with = "bytes_from_base64")]
    signature: Vec<u8>,
}

impl DhtNode {
    fn to_wire(&self) -> Result<proto::dht::NodeOwned> {
        // This transport represents one IPv4 address. Reject multiple addresses
        // rather than altering the signed descriptor before verification.
        ensure!(
            self.addr_list.addrs.len() == 1,
            "DHT bootstrap descriptors must contain exactly one UDP IPv4 address"
        );
        ensure!(
            self.addr_list.priority == 0,
            "nonzero DHT address priority is not supported"
        );
        let Address::Udp { ip, port } = self.addr_list.addrs[0];
        ensure!(port != 0, "DHT bootstrap address has a zero port");
        let PublicKey::Ed25519 { key } = self.id;

        Ok(proto::dht::NodeOwned {
            id: crypto::tl::PublicKeyOwned::Ed25519 { key },
            addr_list: proto::adnl::AddressList {
                address: Some(proto::adnl::Address {
                    ip: ip as u32,
                    port: u32::from(port),
                }),
                version: self.addr_list.version as u32,
                reinit_date: self.addr_list.reinit_date as u32,
                expire_at: self.addr_list.expire_at as u32,
            },
            // Static TON bootstrap descriptors use version = -1. Preserve the
            // signed TL int's bits when passing it to the transport's u32 model.
            version: self.version as u32,
            signature: self.signature.clone().into(),
        })
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(tag = "@type")]
enum PublicKey {
    #[serde(rename = "pub.ed25519")]
    Ed25519 {
        #[serde(deserialize_with = "hash_from_base64")]
        key: [u8; 32],
    },
}

#[derive(Deserialize)]
struct AddressList {
    addrs: Vec<Address>,
    version: i32,
    reinit_date: i32,
    priority: i32,
    expire_at: i32,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(tag = "@type")]
enum Address {
    #[serde(rename = "adnl.address.udp")]
    Udp { ip: i32, port: u16 },
}

fn bytes_from_base64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    let value = String::deserialize(deserializer)?;
    STANDARD.decode(value).map_err(D::Error::custom)
}

fn hash_from_base64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
    bytes_from_base64(deserializer)?
        .try_into()
        .map_err(|_| D::Error::custom("expected 32 base64-encoded bytes"))
}
