use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{storage::NodeSettings, ton::tools::types::TonPublicKey};

pub const SCHEMA_VERSION: u32 = 3;
pub const TON_RELEASE: &str = "v2026.06";

pub const VALIDATOR_CONSOLE_PORT: u16 = 4441;
pub const VALIDATOR_ADNL_PORT: u16 = 4442;
pub const LITESERVER_PORT: u16 = 18_004;
pub const DHT_PORT: u16 = 6302;
pub const OUT_PORT: u16 = 3272;

#[derive(Debug, Clone)]
pub struct Layout {
    pub root: PathBuf,
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
            lock: root.join("instance.lock"),
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
                certs: self.certs.clone(),
                logs: self.logs.clone(),
                global_config: self.global_config.clone(),
            }
        } else {
            let root = self.nodes.join(&settings.name);
            NodeLayout {
                db: root.join("db"),
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
    pub certs: PathBuf,
    pub logs: PathBuf,
    pub global_config: PathBuf,
}

impl NodeLayout {
    pub fn config_json(&self) -> PathBuf {
        self.db.join("config.json")
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
    pub ton_bin_dir: PathBuf,
    pub validator_public_key: TonPublicKey,
    pub liteserver_public_key: TonPublicKey,
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
                "state was created with TON {}, instance expects {}; use another --state-dir",
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

pub fn write_json_atomic<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize + ?Sized,
{
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}
