use std::{
    fs,
    net::{Ipv4Addr, SocketAddrV4},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::storage::NodeSettings;

pub const SCHEMA_VERSION: u32 = 1;
pub const TON_RELEASE: &str = "v2026.06";

pub const VALIDATOR_CONSOLE_PORT: u16 = 4441;
pub const VALIDATOR_ADNL_PORT: u16 = 4442;
pub const LITESERVER_PORT: u16 = 18_004;
pub const DHT_PORT: u16 = 6302;
pub const OUT_PORT: u16 = 3272;

#[derive(Debug, Clone)]
pub struct Layout {
    pub root: PathBuf,
    pub cache: PathBuf,
    pub genesis: PathBuf,
    pub validator_db: PathBuf,
    pub validator_keyring: PathBuf,
    pub dht_db: PathBuf,
    pub certs: PathBuf,
    pub resources: PathBuf,
    pub smartcont: PathBuf,
    pub zerostate: PathBuf,
    pub global_config: PathBuf,
    pub manifest: PathBuf,
    pub settings: PathBuf,
    pub runtime: PathBuf,
    pub wallets: PathBuf,
    pub nodes: PathBuf,
    pub lock: PathBuf,
    pub logs: PathBuf,
}

impl Layout {
    pub fn new(root: PathBuf) -> Self {
        let genesis = root.join("genesis");
        let validator_db = genesis.join("db");
        let resources = genesis.join("resources");
        Self {
            cache: root.join("cache"),
            validator_keyring: validator_db.join("keyring"),
            dht_db: root.join("dht"),
            certs: genesis.join("certs"),
            smartcont: resources.join("smartcont"),
            zerostate: genesis.join("zerostate"),
            global_config: root.join("global.config.json"),
            manifest: root.join("manifest.json"),
            settings: root.join("settings.json"),
            runtime: root.join("runtime.json"),
            wallets: root.join("wallets"),
            nodes: root.join("nodes"),
            lock: root.join("launcher.lock"),
            logs: root.join("logs"),
            validator_db,
            resources,
            genesis,
            root,
        }
    }

    pub fn create_dirs(&self) -> Result<()> {
        for path in [
            &self.root,
            &self.cache,
            &self.genesis,
            &self.validator_db,
            &self.validator_keyring,
            &self.dht_db,
            &self.certs,
            &self.resources,
            &self.smartcont,
            &self.zerostate,
            &self.logs,
            &self.wallets,
            &self.nodes,
        ] {
            fs::create_dir_all(path)
                .with_context(|| format!("failed to create {}", path.display()))?;
        }
        Ok(())
    }

    pub fn node(&self, settings: &NodeSettings) -> NodeLayout {
        if settings.name == "genesis" {
            NodeLayout {
                root: self.genesis.clone(),
                db: self.validator_db.clone(),
                keyring: self.validator_keyring.clone(),
                certs: self.certs.clone(),
                logs: self.logs.clone(),
                global_config: self.global_config.clone(),
            }
        } else {
            let root = self.nodes.join(&settings.name);
            NodeLayout {
                db: root.join("db"),
                keyring: root.join("db/keyring"),
                certs: root.join("certs"),
                logs: self.logs.join(&settings.name),
                global_config: root.join("global.config.json"),
                root,
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeLayout {
    pub root: PathBuf,
    pub db: PathBuf,
    pub keyring: PathBuf,
    pub certs: PathBuf,
    pub logs: PathBuf,
    pub global_config: PathBuf,
}

impl NodeLayout {
    pub fn create_dirs(&self) -> Result<()> {
        for path in [&self.root, &self.db, &self.keyring, &self.certs, &self.logs] {
            fs::create_dir_all(path)
                .with_context(|| format!("failed to create {}", path.display()))?;
        }
        Ok(())
    }

    pub fn config_json(&self) -> PathBuf {
        self.db.join("config.json")
    }

    pub fn server_private_key(&self) -> PathBuf {
        self.certs.join("server")
    }

    pub fn server_public_key(&self) -> PathBuf {
        self.certs.join("server.pub")
    }

    pub fn client_private_key(&self) -> PathBuf {
        self.certs.join("client")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub ton_release: String,
    #[serde(default)]
    pub ton_bin_dir: Option<PathBuf>,
    pub validator_id_hex: String,
    pub validator_id_base64: String,
    pub liteserver_public_key: String,
    pub global_config: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imported_accounts: Vec<ImportedAccountDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImportedAccountDescriptor {
    /// Canonical raw TON address (`0:<account-id>`).
    pub address: String,
    /// Representation hash of the source ShardAccount cell.
    pub shard_account_hash: String,
    /// Native TON balance in nanotons.
    pub balance_nano: String,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read manifest {}", path.display()))?;
        let manifest: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid manifest {}", path.display()))?;
        if manifest.schema_version != SCHEMA_VERSION {
            bail!(
                "unsupported state schema {}, expected {}; use another --state-dir",
                manifest.schema_version,
                SCHEMA_VERSION
            );
        }
        if manifest.ton_release != TON_RELEASE {
            bail!(
                "state was created with TON {}, launcher expects {}; use another --state-dir",
                manifest.ton_release,
                TON_RELEASE
            );
        }
        Ok(manifest)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("manifest path has no parent directory")?;
        fs::create_dir_all(parent)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    }
}

pub fn global_config(
    zero_root_hash: &str,
    zero_file_hash: &str,
    dht_nodes: Vec<Value>,
    liteserver_public_key: &str,
) -> Value {
    let zero_state = json!({
        "@type": "ton.blockIdExt",
        "workchain": -1,
        "shard": -9223372036854775808_i64,
        "seqno": 0,
        "root_hash": zero_root_hash,
        "file_hash": zero_file_hash,
    });
    json!({
        "@type": "config.global",
        "dht": {
            "@type": "dht.config.global",
            "k": 3,
            "a": 3,
            "static_nodes": {
                "@type": "dht.nodes",
                "nodes": dht_nodes,
            },
        },
        "liteservers": [{
            "id": {
                "@type": "pub.ed25519",
                "key": liteserver_public_key,
            },
            "ip": ipv4_to_i32(Ipv4Addr::LOCALHOST),
            "port": LITESERVER_PORT,
        }],
        "validator": {
            "@type": "validator.config.global",
            "zero_state": zero_state.clone(),
            "init_block": zero_state,
        },
    })
}

pub fn endpoint() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::LOCALHOST, LITESERVER_PORT)
}

fn ipv4_to_i32(ip: Ipv4Addr) -> i32 {
    i32::from_be_bytes(ip.octets())
}

pub fn write_json_atomic(path: &Path, value: &Value) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_is_ton_signed_ip() {
        assert_eq!(ipv4_to_i32(Ipv4Addr::LOCALHOST), 2_130_706_433);
    }

    #[test]
    fn global_config_has_matching_init_block() {
        let config = global_config("root", "file", vec![], "pub");
        assert_eq!(
            config.pointer("/validator/zero_state"),
            config.pointer("/validator/init_block")
        );
        assert_eq!(
            config.pointer("/liteservers/0/ip"),
            Some(&json!(2_130_706_433))
        );
    }
}
