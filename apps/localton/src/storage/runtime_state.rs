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

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct RuntimeState {
    pub schema_version: u32,
    pub launcher_pid: Option<u32>,
    pub started_at: Option<u64>,
    pub ready: bool,
    pub masterchain_seqno: Option<u32>,
    pub last_block_at: Option<u64>,
    pub nodes: BTreeMap<String, NodeRuntime>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct NodeRuntime {
    pub initialized: bool,
    pub running: bool,
    pub pid: Option<u32>,
    pub status: String,
    pub last_error: Option<String>,
    pub console_public_key: Option<String>,
    pub liteserver_public_key: Option<String>,
    pub validator_public_key: Option<String>,
    pub validator_adnl: Option<String>,
    pub election_id: Option<u32>,
    pub election_end: Option<u32>,
    #[schema(value_type = Option<String>)]
    pub participation_message: Option<PathBuf>,
    pub total_rewards_nano: String,
    pub last_reward_nano: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ServiceRuntime {
    pub running: bool,
    pub pid: Option<u32>,
    pub endpoint: Option<String>,
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
