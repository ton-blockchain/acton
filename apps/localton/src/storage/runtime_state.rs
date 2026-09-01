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

use crate::ton::tools::types::{KeyId, TonPublicKey};

pub const RUNTIME_SCHEMA_VERSION: u32 = 3;
const MAX_RETAINED_VALIDATOR_KEYS: usize = 64;

/// Coarse validator-engine stage before its masterchain liteserver is queryable.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum InitialSyncStage {
    /// Engine startup has selected an init block but has not discovered state yet
    Starting,
    /// The engine is walking key blocks to select a persistent state
    DiscoveringKeyBlocks,
    /// The selected masterchain persistent state is downloading
    DownloadingMasterchainState,
    /// Required shard states are downloading after the masterchain state
    DownloadingShardStates,
    /// Download is complete and the engine is preparing the first local head
    Preparing,
}

/// Transfer metrics for the persistent state currently downloaded by validator-engine.
///
/// The native console rounds byte counts down to a binary B, KB, MB, or GB unit,
/// so these values are suitable for progress reporting rather than accounting.
/// Speed and ETA are short rolling estimates emitted by the engine itself.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct StateDownloadProgress {
    /// Bytes received for the currently selected persistent state
    pub downloaded_bytes: u64,
    /// Expected size of that persistent state
    pub total_bytes: u64,
    /// Engine-reported rolling transfer rate
    pub bytes_per_second: u64,
    /// Engine-reported estimated time until transfer completion
    pub remaining_seconds: u64,
}

/// Native initial-sync progress reported by validator-engine `getstats`.
///
/// TON downloads a recent persistent state before it exposes a local masterchain
/// head. The stage remains useful throughout that interval, while part counts are
/// present only when the selected state is split into downloadable parts and
/// transfer metrics are present only after validator-engine learns its total size.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct InitialSyncProgress {
    /// Current high-level validator-engine initialization stage
    pub stage: InitialSyncStage,
    /// Masterchain seqno of the state selected by the engine, when known
    pub masterchain_seqno: Option<u32>,
    /// Current persistent-state part, when the state is split
    pub current_part: Option<u32>,
    /// Total number of persistent-state parts, when known
    pub total_parts: Option<u32>,
    /// Byte-level transfer estimates, once the downloader exposes them
    pub state_download: Option<StateDownloadProgress>,
}

impl InitialSyncProgress {
    /// Distinguishes actual download advancement from changing speed and ETA estimates.
    ///
    /// This keeps `sync_progressed_at` useful for stall detection: a peer reporting
    /// a new throughput estimate without receiving more bytes is not progress.
    fn has_advanced_since(&self, previous: &Self) -> bool {
        if self.stage != previous.stage
            || self.masterchain_seqno != previous.masterchain_seqno
            || self.current_part != previous.current_part
        {
            return true;
        }

        match (&self.state_download, &previous.state_download) {
            (Some(current), Some(previous)) => current.downloaded_bytes > previous.downloaded_bytes,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

/// Current Localton instance, node, and service state
#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct RuntimeState {
    /// Version of this runtime state format
    pub schema_version: u32,
    /// Process ID of this Localton instance
    pub instance_pid: Option<u32>,
    /// Unix time when this Localton instance started
    pub started_at: Option<u64>,
    /// `true` when the network can process requests
    pub ready: bool,
    /// Latest masterchain block number that this instance observed
    pub masterchain_seqno: Option<u32>,
    /// Unix time when the observed masterchain seqno last advanced
    pub last_block_at: Option<u64>,
    /// Runtime state for the node owned by this state directory
    pub node: NodeRuntime,
    /// Runtime state for each HTTP service
    pub services: BTreeMap<String, ServiceRuntime>,
}

impl RuntimeState {
    pub fn new() -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            instance_pid: None,
            started_at: None,
            ready: false,
            masterchain_seqno: None,
            last_block_at: None,
            node: NodeRuntime::default(),
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

    pub fn mark_instance_started(&mut self) {
        self.instance_pid = Some(std::process::id());
        self.started_at = Some(unix_time());
        self.ready = false;
    }

    /// Publishes readiness together with the trusted masterchain head that proves it.
    pub fn mark_network_ready(&mut self, masterchain_seqno: u32) {
        self.ready = true;
        self.observe_masterchain_head(masterchain_seqno, unix_time());
    }

    pub fn mark_instance_stopped(&mut self) {
        self.instance_pid = None;
        self.ready = false;
        self.node.running = false;
        self.node.pid = None;
        self.node.status = "stopped".to_owned();
        for service in self.services.values_mut() {
            service.running = false;
            service.pid = None;
        }
    }

    /// Records a trusted masterchain head without hiding a production stall.
    ///
    /// Re-reading the same head proves that the liteserver still answers, but it
    /// is not a new block. Keeping the previous timestamp lets status distinguish
    /// a responsive yet stalled chain from one that is continuing to produce blocks.
    pub fn observe_masterchain_head(&mut self, seqno: u32, observed_at: u64) {
        if self.masterchain_seqno.is_none_or(|current| seqno > current) {
            self.last_block_at = Some(observed_at);
        }
        self.masterchain_seqno = Some(seqno);
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
    /// Latest masterchain block reported by this node's own liteserver
    pub head_seqno: Option<u32>,
    /// Masterchain block that the node was trying to reach at the same sample
    pub network_head_seqno: Option<u32>,
    /// First non-zero masterchain block time observed while this process synchronized
    pub sync_initial_masterchain_block_time: Option<u64>,
    /// Latest masterchain block time reported by validator-engine while synchronizing
    pub sync_masterchain_block_time: Option<u64>,
    /// Validator-engine wall-clock time used as the target for the time-based sample
    pub sync_target_time: Option<u64>,
    /// Native initial-sync stage used before block-time samples become available
    pub initial_sync_progress: Option<InitialSyncProgress>,
    /// Unix time when this node last made measurable synchronization progress
    pub sync_progressed_at: Option<u64>,
    /// Public key for the validator console
    pub console_public_key: Option<TonPublicKey>,
    /// Public key for the liteserver
    pub liteserver_public_key: Option<TonPublicKey>,
    /// Public validator key
    pub validator_public_key: Option<TonPublicKey>,
    /// Public validator keys used across election rounds
    #[serde(default)]
    pub validator_public_keys: Vec<TonPublicKey>,
    /// ADNL address advertised by the node's full-node role
    pub full_node_adnl: Option<KeyId>,
    /// Validator ADNL address
    pub validator_adnl: Option<KeyId>,
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

impl NodeRuntime {
    /// Clears an old synchronization sample before a newly started node is measured.
    ///
    /// A persisted head from the previous process would otherwise make a fresh join
    /// look synchronized until its local liteserver becomes queryable.
    pub fn begin_synchronization(&mut self) {
        self.status = "synchronizing".to_owned();
        self.head_seqno = None;
        self.network_head_seqno = None;
        self.sync_initial_masterchain_block_time = None;
        self.sync_masterchain_block_time = None;
        self.sync_target_time = None;
        self.initial_sync_progress = None;
        self.sync_progressed_at = None;
    }

    /// Records one comparable local and network masterchain sample.
    ///
    /// The target is normalized to at least the local head because two liteservers
    /// can be sampled across a block boundary. This preserves the invariant that
    /// progress never exceeds 100 percent without hiding the exact local head.
    pub fn observe_sync_progress(&mut self, local_head: u32, network_head: u32) {
        let progressed = self.head_seqno != Some(local_head);

        self.head_seqno = Some(local_head);
        self.network_head_seqno = Some(network_head.max(local_head));
        self.initial_sync_progress = None;

        if progressed || self.sync_progressed_at.is_none() {
            self.sync_progressed_at = Some(unix_time());
        }
    }

    /// Records the engine's own initial-sync stage before a block head exists.
    pub fn observe_initial_sync_progress(&mut self, progress: InitialSyncProgress) {
        let progressed = self
            .initial_sync_progress
            .as_ref()
            .is_none_or(|previous| progress.has_advanced_since(previous));

        self.sync_initial_masterchain_block_time = None;
        self.sync_masterchain_block_time = None;
        self.sync_target_time = None;
        self.initial_sync_progress = Some(progress);

        if progressed || self.sync_progressed_at.is_none() {
            self.sync_progressed_at = Some(unix_time());
        }
    }

    /// Records early synchronization progress before the node liteserver can answer.
    ///
    /// Validator-engine exposes the time of its latest masterchain block as soon as
    /// block download starts. Keeping the first sample lets the UI show progress over
    /// the remaining time range without pretending that this estimate is a block head.
    pub fn observe_sync_time_progress(&mut self, block_time: u64, target_time: u64) {
        if block_time == 0 {
            return;
        }

        let progressed = self.sync_masterchain_block_time != Some(block_time);

        self.sync_initial_masterchain_block_time
            .get_or_insert(block_time);
        self.sync_masterchain_block_time = Some(block_time);
        self.sync_target_time = Some(target_time.max(block_time));
        self.initial_sync_progress = None;

        if progressed || self.sync_progressed_at.is_none() {
            self.sync_progressed_at = Some(unix_time());
        }
    }

    /// Retains a validator identity so rewards from earlier election rounds stay attributable.
    pub fn remember_validator_public_key(&mut self, public_key: TonPublicKey) {
        if !self.validator_public_keys.contains(&public_key) {
            self.validator_public_keys.push(public_key);
            if self.validator_public_keys.len() > MAX_RETAINED_VALIDATOR_KEYS {
                self.validator_public_keys.remove(0);
            }
        }
    }

    /// Changes the active validator identity without discarding the previous round's key.
    pub fn set_validator_public_key(&mut self, public_key: TonPublicKey) {
        if let Some(current) = self.validator_public_key {
            self.remember_validator_public_key(current);
        }
        self.remember_validator_public_key(public_key);
        self.validator_public_key = Some(public_key);
    }
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
    fn readiness_is_published_with_its_masterchain_head() {
        let mut state = RuntimeState::new();
        state.mark_instance_started();
        state.mark_network_ready(42);

        expect_test::expect![[r#"
            (
                true,
                Some(
                    42,
                ),
                true,
            )
        "#]]
        .assert_debug_eq(&(
            state.ready,
            state.masterchain_seqno,
            state.last_block_at.is_some(),
        ));
    }

    #[test]
    fn stop_clears_every_runtime_process() {
        let mut state = RuntimeState::new();
        state.node.running = true;
        state.node.pid = Some(123);
        state.services.insert(
            "ton_http_api".to_owned(),
            ServiceRuntime {
                running: true,
                pid: Some(456),
                ..ServiceRuntime::default()
            },
        );
        state.mark_instance_stopped();
        assert!(!state.node.running);
        assert!(!state.services["ton_http_api"].running);
    }

    #[test]
    fn repeated_masterchain_head_does_not_look_like_new_production() {
        let mut state = RuntimeState::new();
        state.observe_masterchain_head(17, 100);
        state.observe_masterchain_head(17, 200);
        expect_test::expect![[r#"
            (
                Some(
                    17,
                ),
                Some(
                    100,
                ),
            )
        "#]]
        .assert_debug_eq(&(state.masterchain_seqno, state.last_block_at));

        state.observe_masterchain_head(18, 300);
        expect_test::expect![[r#"
            (
                Some(
                    18,
                ),
                Some(
                    300,
                ),
            )
        "#]]
        .assert_debug_eq(&(state.masterchain_seqno, state.last_block_at));
    }

    #[test]
    fn synchronization_sample_has_a_valid_target_and_can_be_reset() {
        let mut node = NodeRuntime::default();
        node.observe_initial_sync_progress(InitialSyncProgress {
            stage: InitialSyncStage::DownloadingMasterchainState,
            masterchain_seqno: Some(17),
            current_part: Some(2),
            total_parts: Some(8),
            state_download: None,
        });
        node.observe_sync_progress(18, 17);
        node.observe_sync_time_progress(90, 100);
        expect_test::expect![[r#"
            (
                Some(
                    18,
                ),
                Some(
                    18,
                ),
                Some(
                    90,
                ),
                Some(
                    90,
                ),
                Some(
                    100,
                ),
                None,
                true,
            )
        "#]]
        .assert_debug_eq(&(
            node.head_seqno,
            node.network_head_seqno,
            node.sync_initial_masterchain_block_time,
            node.sync_masterchain_block_time,
            node.sync_target_time,
            node.initial_sync_progress.as_ref(),
            node.sync_progressed_at.is_some(),
        ));

        node.begin_synchronization();
        expect_test::expect![[r#"
            (
                "synchronizing",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        "#]]
        .assert_debug_eq(&(
            node.status,
            node.head_seqno,
            node.network_head_seqno,
            node.sync_initial_masterchain_block_time,
            node.sync_masterchain_block_time,
            node.sync_target_time,
            node.initial_sync_progress.as_ref(),
            node.sync_progressed_at,
        ));
    }

    #[test]
    fn repeated_initial_sync_sample_does_not_look_like_progress() {
        let progress = InitialSyncProgress {
            stage: InitialSyncStage::Starting,
            masterchain_seqno: Some(46_894_135),
            current_part: None,
            total_parts: None,
            state_download: Some(StateDownloadProgress {
                downloaded_bytes: 1_024,
                total_bytes: 8_192,
                bytes_per_second: 512,
                remaining_seconds: 14,
            }),
        };
        let mut node = NodeRuntime::default();
        node.observe_initial_sync_progress(progress.clone());
        node.sync_progressed_at = Some(1);

        let mut same_bytes = progress;
        same_bytes.state_download = Some(StateDownloadProgress {
            downloaded_bytes: 1_024,
            total_bytes: 8_192,
            bytes_per_second: 256,
            remaining_seconds: 28,
        });
        node.observe_initial_sync_progress(same_bytes);

        expect_test::expect![[r#"
            (
                Some(
                    1,
                ),
                Some(
                    InitialSyncProgress {
                        stage: Starting,
                        masterchain_seqno: Some(
                            46894135,
                        ),
                        current_part: None,
                        total_parts: None,
                        state_download: Some(
                            StateDownloadProgress {
                                downloaded_bytes: 1024,
                                total_bytes: 8192,
                                bytes_per_second: 256,
                                remaining_seconds: 28,
                            },
                        ),
                    },
                ),
            )
        "#]]
        .assert_debug_eq(&(node.sync_progressed_at, node.initial_sync_progress.as_ref()));

        let mut advanced = node.initial_sync_progress.clone().unwrap();
        advanced.state_download.as_mut().unwrap().downloaded_bytes = 2_048;
        node.observe_initial_sync_progress(advanced);
        assert_ne!(node.sync_progressed_at, Some(1));
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
                    state.services.insert(
                        format!("service-{index}"),
                        ServiceRuntime {
                            running: true,
                            ..ServiceRuntime::default()
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
        assert_eq!(RuntimeState::load(&path).unwrap().services.len(), 8);
    }

    #[test]
    fn validator_keys_are_retained_across_rounds() {
        let mut node = NodeRuntime::default();
        node.remember_validator_public_key(TonPublicKey::from_bytes([1; 32]));
        node.set_validator_public_key(TonPublicKey::from_bytes([2; 32]));
        node.set_validator_public_key(TonPublicKey::from_bytes([3; 32]));

        expect_test::expect![[r#"
            {
              "current": "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM=",
              "history": [
                "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=",
                "AgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgI=",
                "AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwM="
              ]
            }"#]]
        .assert_eq(
            &serde_json::to_string_pretty(&serde_json::json!({
                "current": node.validator_public_key,
                "history": node.validator_public_keys,
            }))
            .unwrap(),
        );
    }
}
