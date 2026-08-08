use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const RUNTIME_SCHEMA_VERSION: u32 = 1;

/// Current launcher, node, and service state
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct RuntimeState {
    /// Version of this runtime state format
    pub schema_version: u32,
    /// Process ID of the Localton launcher
    pub launcher_pid: Option<u32>,
    /// Unix time when the launcher started
    pub started_at: Option<u64>,
    /// `true` when the network can process requests
    pub ready: bool,
    /// Latest masterchain block number that the launcher observed
    pub masterchain_seqno: Option<u32>,
    /// Unix time when the launcher observed the latest block
    pub last_block_at: Option<u64>,
    /// Runtime state for each configured node
    pub nodes: BTreeMap<String, NodeRuntime>,
    /// Runtime state for each HTTP service
    pub services: BTreeMap<String, ServiceRuntime>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            launcher_pid: None,
            started_at: None,
            ready: false,
            masterchain_seqno: None,
            last_block_at: None,
            nodes: BTreeMap::new(),
            services: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::new());
        }
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let state: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid runtime state {}", path.display()))?;
        ensure!(
            state.schema_version == RUNTIME_SCHEMA_VERSION,
            "unsupported runtime schema {}",
            state.schema_version
        );
        Ok(state)
    }

    pub fn save_atomic(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("runtime state path has no parent directory")?;
        fs::create_dir_all(parent)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
        Ok(())
    }

    pub fn update_atomic(
        path: &Path,
        update: impl FnOnce(&mut RuntimeState) -> Result<()>,
    ) -> Result<Self> {
        let parent = path
            .parent()
            .context("runtime state path has no parent directory")?;
        fs::create_dir_all(parent)?;
        let lock_path = path.with_extension("lock");
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("failed to open {}", lock_path.display()))?;
        lock.lock_exclusive()
            .with_context(|| format!("failed to lock {}", lock_path.display()))?;
        let result = (|| {
            let mut state = Self::load(path)?;
            update(&mut state)?;
            state.save_atomic(path)?;
            Ok(state)
        })();
        let unlock_result = FileExt::unlock(&lock)
            .with_context(|| format!("failed to unlock {}", lock_path.display()));
        match (result, unlock_result) {
            (Ok(state), Ok(())) => Ok(state),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    pub fn mark_launcher_started(&mut self) {
        self.launcher_pid = Some(std::process::id());
        self.started_at = Some(unix_time());
        self.ready = false;
    }

    pub fn mark_launcher_stopped(&mut self) {
        self.launcher_pid = None;
        self.ready = false;
        for node in self.nodes.values_mut() {
            node.running = false;
            node.pid = None;
            node.status = "stopped".to_owned();
        }
        for service in self.services.values_mut() {
            service.running = false;
            service.pid = None;
        }
    }
}

/// Current state of one Localton node
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct NodeRuntime {
    /// `true` when Localton initialized the node files
    pub initialized: bool,
    /// `true` when the node process is running
    pub running: bool,
    /// Process ID of the node
    pub pid: Option<u32>,
    /// Current node status
    pub status: String,
    /// Last node error
    pub last_error: Option<String>,
    /// Public key for the validator console
    pub console_public_key: Option<String>,
    /// Public key for the liteserver
    pub liteserver_public_key: Option<String>,
    /// Public validator key
    pub validator_public_key: Option<String>,
    /// Validator ADNL address
    pub validator_adnl: Option<String>,
    /// Current election identifier
    pub election_id: Option<u32>,
    /// Unix time when the current election ends
    pub election_end: Option<u32>,
    /// Path of the validator participation message
    #[schema(value_type = Option<String>)]
    pub participation_message: Option<PathBuf>,
    /// Total validator rewards in nanotons
    pub total_rewards_nano: String,
    /// Latest validator reward in nanotons
    pub last_reward_nano: String,
}

/// Current state of one Localton service
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ServiceRuntime {
    /// `true` when the service is running
    pub running: bool,
    /// Process ID of the service
    pub pid: Option<u32>,
    /// Public service URL
    pub endpoint: Option<String>,
    /// Last service error
    pub last_error: Option<String>,
}

pub fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_clears_every_runtime_process() {
        let mut state = RuntimeState::new();
        state.nodes.insert(
            "genesis".to_owned(),
            NodeRuntime {
                running: true,
                pid: Some(123),
                ..NodeRuntime::default()
            },
        );
        state.services.insert(
            "ton_http_api".to_owned(),
            ServiceRuntime {
                running: true,
                pid: Some(456),
                ..ServiceRuntime::default()
            },
        );
        state.mark_launcher_stopped();
        assert!(!state.nodes["genesis"].running);
        assert!(!state.services["ton_http_api"].running);
    }

    #[test]
    fn atomic_updates_preserve_independent_writers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("runtime.json");
        let mut threads = Vec::new();
        for index in 0..8 {
            let path = path.clone();
            threads.push(std::thread::spawn(move || {
                RuntimeState::update_atomic(&path, |state| {
                    state.nodes.insert(
                        format!("node-{index}"),
                        NodeRuntime {
                            initialized: true,
                            ..NodeRuntime::default()
                        },
                    );
                    Ok(())
                })
                .unwrap();
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(RuntimeState::load(&path).unwrap().nodes.len(), 8);
    }
}
