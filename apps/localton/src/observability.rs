//! Signed host telemetry combined with independently read TON network state.
//!
//! A stable observer key authenticates process health reported by one Localton
//! state directory. Chain heads, elections, validator membership, and production
//! never enter that signed payload; every dashboard derives them through its own
//! liteserver connection.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;

use crate::storage::InitialSyncProgress;

/// Wire format accepted by the signed telemetry collector endpoint.
pub const PROTOCOL_VERSION: u16 = 2;
/// Maximum live observer reports retained for one network view.
pub const MAX_OBSERVERS: usize = 1_024;
/// Maximum head difference that still counts as synchronized.
pub const SYNC_LAG_TOLERANCE_BLOCKS: u32 = 2;
const MAX_CLOCK_SKEW_SECONDS: u64 = 30;
const MAX_OBSERVATION_TTL_SECONDS: u64 = 5 * 60;

/// Masterchain reference independently verified by an observer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ChainHead {
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
    pub gen_utime: u32,
    pub observed_at: u64,
    pub shard_count: usize,
}

/// Latest block observed for one shard at the reported masterchain head.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ShardHead {
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
    pub gen_utime: u32,
    pub before_split: bool,
    pub before_merge: bool,
    pub want_split: bool,
    pub want_merge: bool,
}

/// One validator set and the exact interval in which it secures the network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ValidatorSetObservation {
    pub round_id: u32,
    pub validation_started_at: u32,
    pub validation_ended_at: u32,
    pub validators: usize,
    pub main_validators: u16,
    /// Exact decimal representation avoids losing `u64` weight precision in JSON clients.
    pub total_weight: String,
    pub members: Vec<ValidatorObservation>,
}

/// Public validator identity and weight for one election cycle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ValidatorObservation {
    /// Canonical lowercase Ed25519 public key.
    pub public_key: String,
    /// Canonical lowercase ADNL address when config provides one.
    pub adnl_address: Option<String>,
    /// Exact decimal representation of the validator weight.
    pub weight: String,
}

/// Election timing and adjacent validator sets decoded from on-chain configuration.
///
/// TON keeps the previous, current, and elected next sets in configuration
/// parameters 32, 34, and 36. The next set is absent until its election completes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ElectionObservation {
    pub stage: ElectionStage,
    pub elections_open_at: u32,
    pub elections_close_at: u32,
    pub validators_elected_for: u32,
    pub stake_held_for: u32,
    pub previous: Option<ValidatorSetObservation>,
    pub current: ValidatorSetObservation,
    pub next: Option<ValidatorSetObservation>,
}

/// Election phase derived from on-chain timing and next-set availability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ElectionStage {
    Validation,
    AcceptingEntries,
    Finalizing,
    NextSetReady,
    Retrying,
    ActivationOverdue,
}

/// TON state independently read by the dashboard through a liteserver.
///
/// Validator keys are kept beside the chain snapshot so membership and block
/// production can be derived locally instead of trusted from host telemetry.
#[derive(Debug, Clone)]
pub(crate) struct VerifiedNetworkState {
    pub head: ChainHead,
    pub masterchain_history: Vec<MasterchainBlock>,
    pub shards: Vec<ShardHead>,
    pub election: Option<ElectionObservation>,
    pub production: Vec<ProductionView>,
    pub current_validator_keys: Option<BTreeSet<String>>,
    pub next_validator_keys: Option<BTreeSet<String>>,
}

impl VerifiedNetworkState {
    /// Returns the last masterchain head that existed when a node head was sampled.
    ///
    /// Collector delivery and network reads run independently. Matching by block time
    /// prevents delivery latency from appearing as node synchronization lag.
    fn head_at(&self, observed_at: u64) -> Option<u32> {
        self.masterchain_history
            .iter()
            .filter(|block| u64::from(block.gen_utime) <= observed_at)
            .map(|block| block.seqno)
            .max()
    }
}

/// Masterchain position retained for time-aligned synchronization comparisons.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MasterchainBlock {
    pub seqno: u32,
    pub gen_utime: u32,
}

/// Network capability configured for one validator-engine process.
///
/// Liveness and active validator membership are reported separately; these values
/// describe what the process is configured to provide when it is online.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeCapability {
    FullNode,
    Validator,
    Liteserver,
}

/// Process and synchronization state self-reported by one owned node.
///
/// This payload contains only facts owned by the host. Network-wide facts are
/// joined later from [`VerifiedNetworkState`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NodeTelemetry {
    pub software: String,
    pub observability_endpoint: String,
    pub instance_started_at: Option<u64>,
    pub name: String,
    pub public_ip: String,
    pub roles: Vec<NodeCapability>,
    pub running: bool,
    pub process_id: Option<u32>,
    pub status: String,
    pub last_error: Option<String>,
    /// Latest masterchain block reported by the node's own liteserver
    pub head_seqno: Option<u32>,
    /// Unix time when the node's own liteserver returned `head_seqno`
    pub head_observed_at: Option<u64>,
    /// First masterchain block time observed during the current synchronization
    pub sync_initial_masterchain_block_time: Option<u64>,
    /// Latest masterchain block time reported directly by validator-engine
    pub sync_masterchain_block_time: Option<u64>,
    /// Validator-engine wall-clock time used as the target for the time-based sample
    pub sync_target_time: Option<u64>,
    /// Native initial-sync stage before block-time samples become available
    pub initial_sync_progress: Option<InitialSyncProgress>,
    /// Unix time when this node last made measurable synchronization progress
    pub sync_progressed_at: Option<u64>,
    pub participate_in_elections: bool,
    pub validator_public_key: Option<String>,
    pub validator_public_keys: Vec<String>,
    pub validator_adnl: Option<String>,
}

/// Versioned observation authenticated by the state directory's stable Ed25519 key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SignedObservation {
    pub protocol_version: u16,
    pub network_id: String,
    pub observer_id: String,
    pub public_key: String,
    pub sequence: u64,
    pub generated_at: u64,
    pub expires_at: u64,
    pub payload: NodeTelemetry,
    pub signature: String,
}

#[derive(Serialize)]
struct SignableObservation<'a> {
    protocol_version: u16,
    network_id: &'a str,
    observer_id: &'a str,
    public_key: &'a str,
    sequence: u64,
    generated_at: u64,
    expires_at: u64,
    payload: &'a NodeTelemetry,
}

impl SignedObservation {
    fn signing_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&SignableObservation {
            protocol_version: self.protocol_version,
            network_id: &self.network_id,
            observer_id: &self.observer_id,
            public_key: &self.public_key,
            sequence: self.sequence,
            generated_at: self.generated_at,
            expires_at: self.expires_at,
            payload: &self.payload,
        })?)
    }
}

/// Public liveness summary for one observer identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ObserverView {
    pub observer_id: String,
    pub endpoint: String,
    pub software: String,
    pub generated_at: u64,
    pub expires_at: u64,
    pub online: bool,
}

/// Aggregated node state after signed reports and verified chain data are combined.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NodeView {
    pub observer_id: String,
    pub generated_at: u64,
    pub expires_at: u64,
    pub online: bool,
    pub sync_status: SyncStatus,
    pub active_validator: bool,
    pub validator_status: ValidatorStatus,
    pub produced_masterchain_blocks: u64,
    pub produced_shard_blocks: u64,
    pub network_head_seqno: Option<u32>,
    pub sync_lag_blocks: Option<u32>,
    pub current_validator: Option<bool>,
    pub next_validator: Option<bool>,
    /// Approximate placement derived locally from the node's advertised address.
    pub location: NodeLocation,
    #[serde(flatten)]
    pub telemetry: NodeTelemetry,
}

/// Country-level placement derived from an offline IP allocation database.
///
/// IP geolocation cannot identify a physical host location. The dashboard uses
/// this value only for a coarse network distribution view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeLocation {
    /// Country-level result for a globally routable address.
    Country {
        /// ISO 3166-1 alpha-2 country code.
        country_code: String,
        country: String,
    },
    /// Loopback, private, link-local, carrier-grade NAT, or reserved address.
    Private,
    /// A public address for which the local database has no usable record.
    Unavailable,
}

/// Synchronization classification derived from node and network head samples.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Synced,
    CatchingUp,
    Unknown,
    Offline,
}

/// Election lifecycle state derived from configuration, intent, and set membership.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidatorStatus {
    NotConfigured,
    Validating,
    Leaving,
    Joining,
    Waiting,
    Inactive,
    Unknown,
}

/// Blocks attributed to one validator in the active retention window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ProductionView {
    pub creator: String,
    pub masterchain_blocks: u64,
    pub shard_blocks: u64,
    pub last_block_at: u32,
}

/// Counts used by the dashboard to summarize network health.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NetworkTotals {
    pub observers: usize,
    pub online_observers: usize,
    pub nodes: usize,
    pub online_nodes: usize,
    pub synchronized_nodes: usize,
    pub catching_up_nodes: usize,
    pub configured_validators: usize,
    pub active_validators: usize,
    pub full_nodes: usize,
    pub masterchain_blocks: usize,
    pub shard_blocks: usize,
}

/// Aggregate returned by combining signed host telemetry with local TON reads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NetworkView {
    pub protocol_version: u16,
    pub network_id: String,
    pub generated_at: u64,
    pub chain: Option<ChainHead>,
    pub shards: Vec<ShardHead>,
    pub election: Option<ElectionObservation>,
    pub totals: NetworkTotals,
    pub observers: Vec<ObserverView>,
    pub nodes: Vec<NodeView>,
    pub production: Vec<ProductionView>,
}

/// Stable signing identity owned by one Localton state directory.
#[derive(Clone)]
pub struct ObserverIdentity {
    signing_key: SigningKey,
    observer_id: String,
    public_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct IdentityFile {
    version: u8,
    secret_key: String,
}

impl ObserverIdentity {
    /// Loads an observer key or atomically creates it before publication starts.
    ///
    /// The private key never leaves this type. A partially written key is not
    /// accepted, which keeps the observer ID stable across process restarts.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        let secret = if path.is_file() {
            let bytes = fs::read(path)
                .with_context(|| format!("failed to read observer identity {}", path.display()))?;
            let identity: IdentityFile = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid observer identity {}", path.display()))?;
            ensure!(
                identity.version == 1,
                "unsupported observer identity version"
            );
            let decoded = STANDARD
                .decode(identity.secret_key)
                .context("observer secret key is not valid base64")?;
            <[u8; 32]>::try_from(decoded.as_slice())
                .map_err(|_| anyhow::anyhow!("observer secret key must contain 32 bytes"))?
        } else {
            let mut secret = [0_u8; 32];
            OsRng.fill_bytes(&mut secret);
            let parent = path
                .parent()
                .context("observer identity path has no parent")?;
            fs::create_dir_all(parent)?;
            let temporary = path.with_extension("json.tmp");
            fs::write(
                &temporary,
                serde_json::to_vec_pretty(&IdentityFile {
                    version: 1,
                    secret_key: STANDARD.encode(secret),
                })?,
            )
            .with_context(|| format!("failed to write {}", temporary.display()))?;
            set_private_permissions(&temporary)?;
            fs::rename(&temporary, path)
                .with_context(|| format!("failed to replace {}", path.display()))?;
            secret
        };
        Ok(Self::from_secret(secret))
    }

    fn from_secret(secret: [u8; 32]) -> Self {
        Self::from_signing_key(SigningKey::from_bytes(&secret))
    }

    fn from_signing_key(signing_key: SigningKey) -> Self {
        let verifying_key = signing_key.verifying_key();
        let public_bytes = verifying_key.to_bytes();
        Self {
            signing_key,
            observer_id: hex::encode(Sha256::digest(public_bytes)),
            public_key: STANDARD.encode(public_bytes),
        }
    }

    /// Returns the stable identifier derived from the signing public key.
    pub fn observer_id(&self) -> &str {
        &self.observer_id
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

/// In-memory anti-replay store for locally published and collected telemetry.
///
/// The store accepts only current signatures for one network ID and retains the
/// newest sequence per observer. Chain state is supplied separately at read time.
pub struct ObservationStore {
    network_id: String,
    identity: ObserverIdentity,
    next_sequence: u64,
    retention_seconds: u64,
    observations: BTreeMap<String, SignedObservation>,
}

impl ObservationStore {
    /// Creates an empty store for one network and takes ownership of the local signer.
    pub fn new(network_id: String, identity: ObserverIdentity, retention_seconds: u64) -> Self {
        Self {
            network_id,
            identity,
            next_sequence: unix_time_millis().saturating_mul(1_000),
            retention_seconds,
            observations: BTreeMap::new(),
        }
    }

    /// Signs and installs the next local observation sequence.
    pub fn publish(
        &mut self,
        payload: NodeTelemetry,
        now: u64,
        ttl_seconds: u64,
    ) -> Result<SignedObservation> {
        self.next_sequence = self.next_sequence.saturating_add(1);
        let mut observation = SignedObservation {
            protocol_version: PROTOCOL_VERSION,
            network_id: self.network_id.clone(),
            observer_id: self.identity.observer_id.clone(),
            public_key: self.identity.public_key.clone(),
            sequence: self.next_sequence,
            generated_at: now,
            expires_at: now.saturating_add(ttl_seconds),
            payload,
            signature: String::new(),
        };
        observation.signature = STANDARD.encode(
            self.identity
                .signing_key
                .sign(&observation.signing_bytes()?)
                .to_bytes(),
        );
        self.observations
            .insert(observation.observer_id.clone(), observation.clone());
        self.prune(now);
        Ok(observation)
    }

    /// Verifies one pushed heartbeat and retains it when its sequence is newer.
    pub fn ingest(&mut self, observation: SignedObservation, now: u64) -> Result<bool> {
        verify_observation(&observation, &self.network_id, now)?;
        ensure!(
            self.observations.contains_key(&observation.observer_id)
                || self.observations.len() < MAX_OBSERVERS,
            "observer store capacity exceeded"
        );

        let replace = self
            .observations
            .get(&observation.observer_id)
            .is_none_or(|current| observation.sequence > current.sequence);
        if replace {
            self.observations
                .insert(observation.observer_id.clone(), observation);
        }

        self.prune(now);
        Ok(replace)
    }

    /// Returns the most recent locally signed report, if publication has started.
    pub fn local(&self) -> Option<SignedObservation> {
        self.observations.get(self.identity.observer_id()).cloned()
    }

    /// Builds a dashboard view from host reports and one locally read network state.
    ///
    /// When the local node is also the source of the network snapshot, its row uses
    /// that canonical head instead of comparing two independent liteserver requests.
    pub fn aggregate(
        &mut self,
        now: u64,
        network: Option<&VerifiedNetworkState>,
        local_node_is_network_source: bool,
    ) -> NetworkView {
        self.prune(now);
        let mut production = network
            .map(|network| network.production.clone())
            .unwrap_or_default();
        production.sort_by(|left, right| {
            right
                .masterchain_blocks
                .cmp(&left.masterchain_blocks)
                .then_with(|| right.shard_blocks.cmp(&left.shard_blocks))
                .then_with(|| left.creator.cmp(&right.creator))
        });

        let production_by_creator = production
            .iter()
            .map(|entry| (entry.creator.clone(), entry))
            .collect::<BTreeMap<_, _>>();
        let mut observers = Vec::new();
        let mut nodes = Vec::new();
        for observation in self.observations.values() {
            let observer_online = observation.expires_at > now;
            observers.push(ObserverView {
                observer_id: observation.observer_id.clone(),
                endpoint: observation.payload.observability_endpoint.clone(),
                software: observation.payload.software.clone(),
                generated_at: observation.generated_at,
                expires_at: observation.expires_at,
                online: observer_online,
            });

            let local_network = network.filter(|_| {
                local_node_is_network_source
                    && observation.observer_id == self.identity.observer_id()
            });
            let mut telemetry = observation.payload.clone();
            if let Some(network) = local_network {
                if telemetry.head_seqno != Some(network.head.seqno) {
                    telemetry.sync_progressed_at = Some(network.head.observed_at);
                }
                telemetry.head_seqno = Some(network.head.seqno);
                telemetry.head_observed_at = Some(network.head.observed_at);
            }
            let telemetry = &telemetry;
            let online = observer_online && telemetry.running;
            let validator_keys = telemetry
                .validator_public_keys
                .iter()
                .chain(telemetry.validator_public_key.iter())
                .filter_map(|key| public_key_hex(key))
                .collect::<BTreeSet<_>>();
            let masterchain = validator_keys.iter().fold(0_u64, |total, key| {
                total.saturating_add(
                    production_by_creator
                        .get(key)
                        .map_or(0, |counts| counts.masterchain_blocks),
                )
            });
            let shard = validator_keys.iter().fold(0_u64, |total, key| {
                total.saturating_add(
                    production_by_creator
                        .get(key)
                        .map_or(0, |counts| counts.shard_blocks),
                )
            });
            let membership = |set: Option<&BTreeSet<String>>| {
                if validator_keys.is_empty() {
                    None
                } else {
                    set.map(|set| validator_keys.iter().any(|key| set.contains(key)))
                }
            };
            let current_membership =
                membership(network.and_then(|network| network.current_validator_keys.as_ref()));
            let next_membership =
                membership(network.and_then(|network| network.next_validator_keys.as_ref()));
            let active_validator = current_membership.unwrap_or(masterchain > 0 || shard > 0);
            let network_head = local_network.map(|network| network.head.seqno).or_else(|| {
                telemetry.head_observed_at.and_then(|observed_at| {
                    network.and_then(|network| network.head_at(observed_at))
                })
            });
            let sync_lag_blocks = network_head
                .zip(telemetry.head_seqno)
                .map(|(network, node)| network.saturating_sub(node));
            let sync_status = sync_status(online, sync_lag_blocks, &telemetry.status);
            let validator_status = validator_status(
                telemetry.roles.contains(&NodeCapability::Validator),
                telemetry.participate_in_elections,
                current_membership,
                next_membership,
            );
            nodes.push(NodeView {
                observer_id: observation.observer_id.clone(),
                generated_at: observation.generated_at,
                expires_at: observation.expires_at,
                online,
                sync_status,
                active_validator,
                validator_status,
                produced_masterchain_blocks: masterchain,
                produced_shard_blocks: shard,
                network_head_seqno: network_head,
                sync_lag_blocks,
                current_validator: current_membership,
                next_validator: next_membership,
                location: NodeLocation::Unavailable,
                telemetry: telemetry.clone(),
            });
        }
        observers.sort_by(|left, right| left.observer_id.cmp(&right.observer_id));
        nodes.sort_by(|left, right| {
            left.telemetry
                .name
                .cmp(&right.telemetry.name)
                .then_with(|| left.observer_id.cmp(&right.observer_id))
        });

        let chain = network.map(|network| network.head.clone());
        let shards = network
            .map(|network| network.shards.clone())
            .unwrap_or_default();
        let election = network.and_then(|network| network.election.clone());
        let totals = NetworkTotals {
            observers: observers.len(),
            online_observers: observers.iter().filter(|observer| observer.online).count(),
            nodes: nodes.len(),
            online_nodes: nodes.iter().filter(|node| node.online).count(),
            synchronized_nodes: nodes
                .iter()
                .filter(|node| node.sync_status == SyncStatus::Synced)
                .count(),
            catching_up_nodes: nodes
                .iter()
                .filter(|node| node.sync_status == SyncStatus::CatchingUp)
                .count(),
            configured_validators: nodes
                .iter()
                .filter(|node| node.telemetry.roles.contains(&NodeCapability::Validator))
                .count(),
            active_validators: nodes.iter().filter(|node| node.active_validator).count(),
            full_nodes: nodes
                .iter()
                .filter(|node| node.telemetry.roles.contains(&NodeCapability::FullNode))
                .count(),
            masterchain_blocks: production.iter().fold(0, |total, entry| {
                total
                    .saturating_add(usize::try_from(entry.masterchain_blocks).unwrap_or(usize::MAX))
            }),
            shard_blocks: production.iter().fold(0, |total, entry| {
                total.saturating_add(usize::try_from(entry.shard_blocks).unwrap_or(usize::MAX))
            }),
        };
        NetworkView {
            protocol_version: PROTOCOL_VERSION,
            network_id: self.network_id.clone(),
            generated_at: now,
            chain,
            shards,
            election,
            totals,
            observers,
            nodes,
            production,
        }
    }

    fn prune(&mut self, now: u64) {
        let retention = self.retention_seconds;
        self.observations
            .retain(|_, observation| observation.generated_at.saturating_add(retention) > now);
    }
}

/// Derives the observation network ID from the immutable TON genesis reference.
///
/// Reports from another chain are rejected before entering the collector store.
pub fn network_id(global_config: &Path) -> Result<String> {
    let bytes = fs::read(global_config)
        .with_context(|| format!("failed to read global config {}", global_config.display()))?;
    let config: serde_json::Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid global config {}", global_config.display()))?;
    let zero_state = config
        .pointer("/validator/zero_state")
        .context("global config has no validator zerostate")?;
    let canonical = serde_json::to_vec(zero_state)?;
    Ok(hex::encode(Sha256::digest(canonical)))
}

fn verify_observation(
    observation: &SignedObservation,
    expected_network_id: &str,
    now: u64,
) -> Result<()> {
    ensure!(
        observation.protocol_version == PROTOCOL_VERSION,
        "unsupported observation protocol version"
    );
    ensure!(
        observation.network_id == expected_network_id,
        "observation belongs to a different network"
    );
    ensure!(
        observation.generated_at <= now.saturating_add(MAX_CLOCK_SKEW_SECONDS),
        "observation timestamp is too far in the future"
    );
    ensure!(observation.expires_at > now, "observation has expired");
    ensure!(
        observation.expires_at > observation.generated_at,
        "observation expiry precedes its timestamp"
    );
    ensure!(
        observation
            .expires_at
            .saturating_sub(observation.generated_at)
            <= MAX_OBSERVATION_TTL_SECONDS,
        "observation expiry is too far in the future"
    );
    let public_key = STANDARD
        .decode(&observation.public_key)
        .context("observation public key is not valid base64")?;
    let public_key = <[u8; 32]>::try_from(public_key.as_slice())
        .map_err(|_| anyhow::anyhow!("observation public key must contain 32 bytes"))?;
    ensure!(
        observation.observer_id == hex::encode(Sha256::digest(public_key)),
        "observer ID does not match its public key"
    );
    let signature = STANDARD
        .decode(&observation.signature)
        .context("observation signature is not valid base64")?;
    let signature = Signature::from_slice(&signature).context("invalid observation signature")?;
    VerifyingKey::from_bytes(&public_key)
        .context("invalid observation public key")?
        .verify(&observation.signing_bytes()?, &signature)
        .context("observation signature verification failed")?;
    Ok(())
}

fn sync_status(online: bool, lag: Option<u32>, runtime_status: &str) -> SyncStatus {
    if !online {
        return SyncStatus::Offline;
    }
    if runtime_status == "synchronizing" {
        return SyncStatus::CatchingUp;
    }
    match lag {
        Some(lag) if lag <= SYNC_LAG_TOLERANCE_BLOCKS => SyncStatus::Synced,
        Some(_) => SyncStatus::CatchingUp,
        None => SyncStatus::Unknown,
    }
}

fn validator_status(
    configured: bool,
    participate_in_elections: bool,
    current_membership: Option<bool>,
    next_membership: Option<bool>,
) -> ValidatorStatus {
    if !configured {
        return ValidatorStatus::NotConfigured;
    }
    match (
        current_membership,
        participate_in_elections,
        next_membership,
    ) {
        (Some(true), false, _) | (Some(true), true, Some(false)) => ValidatorStatus::Leaving,
        (Some(true), true, _) => ValidatorStatus::Validating,
        (Some(false), _, Some(true)) => ValidatorStatus::Joining,
        (Some(false), true, _) => ValidatorStatus::Waiting,
        (Some(false), false, _) | (None, false, _) => ValidatorStatus::Inactive,
        (None, true, _) => ValidatorStatus::Unknown,
    }
}

pub(crate) fn public_key_hex(value: &str) -> Option<String> {
    let bytes = hex::decode(value).ok()?;
    (bytes.len() == 32).then(|| hex::encode(bytes))
}

fn unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn telemetry(validator_key: Option<String>) -> NodeTelemetry {
        NodeTelemetry {
            software: "localton/test".to_owned(),
            observability_endpoint: "http://127.0.0.1:18007".to_owned(),
            instance_started_at: Some(10),
            name: "node".to_owned(),
            public_ip: "127.0.0.1".to_owned(),
            roles: vec![NodeCapability::FullNode, NodeCapability::Validator],
            running: true,
            process_id: Some(1),
            status: "running".to_owned(),
            last_error: None,
            head_seqno: Some(7),
            head_observed_at: Some(100),
            sync_initial_masterchain_block_time: None,
            sync_masterchain_block_time: None,
            sync_target_time: None,
            initial_sync_progress: None,
            sync_progressed_at: Some(100),
            participate_in_elections: true,
            validator_public_key: validator_key,
            validator_public_keys: Vec::new(),
            validator_adnl: None,
        }
    }

    fn network_state(seqno: u32, election: Option<ElectionObservation>) -> VerifiedNetworkState {
        VerifiedNetworkState {
            head: ChainHead {
                seqno,
                root_hash: format!("root-{seqno}"),
                file_hash: format!("file-{seqno}"),
                gen_utime: 99,
                observed_at: 100,
                shard_count: 1,
            },
            masterchain_history: vec![MasterchainBlock {
                seqno,
                gen_utime: 99,
            }],
            shards: Vec::new(),
            election,
            production: Vec::new(),
            current_validator_keys: None,
            next_validator_keys: None,
        }
    }

    #[test]
    fn signed_observation_rejects_tampering() {
        let identity = ObserverIdentity::from_secret([7; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut observation = store.publish(telemetry(None), 100, 20).unwrap();
        observation.payload.software = "tampered".to_owned();

        let collector_identity = ObserverIdentity::from_secret([8; 32]);
        let mut collector = ObservationStore::new("network".to_owned(), collector_identity, 600);
        assert!(collector.ingest(observation, 101).is_err());
    }

    #[test]
    fn synchronizing_runtime_is_catching_up_before_the_first_head() {
        assert_eq!(
            sync_status(true, None, "synchronizing"),
            SyncStatus::CatchingUp
        );
    }

    #[test]
    fn host_telemetry_is_combined_with_the_local_network_head() {
        let identity = ObserverIdentity::from_secret([6; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = telemetry(None);
        report.status = "synchronizing".to_owned();
        report.head_seqno = Some(40);
        store.publish(report, 100, 20).unwrap();

        let network = network_state(100, None);
        let view = store.aggregate(101, Some(&network), false);
        expect_test::expect![[r#"
            (
                Some(
                    40,
                ),
                Some(
                    100,
                ),
                Some(
                    60,
                ),
                CatchingUp,
            )
        "#]]
        .assert_debug_eq(&(
            view.nodes[0].telemetry.head_seqno,
            view.nodes[0].network_head_seqno,
            view.nodes[0].sync_lag_blocks,
            view.nodes[0].sync_status,
        ));
    }

    #[test]
    fn synchronization_uses_the_network_head_from_the_node_sample_time() {
        let identity = ObserverIdentity::from_secret([5; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = telemetry(None);
        report.head_seqno = Some(100);
        report.head_observed_at = Some(100);
        store.publish(report, 105, 20).unwrap();

        let mut network = network_state(110, None);
        network.masterchain_history = vec![
            MasterchainBlock {
                seqno: 100,
                gen_utime: 100,
            },
            MasterchainBlock {
                seqno: 110,
                gen_utime: 110,
            },
        ];

        let view = store.aggregate(105, Some(&network), false);
        expect_test::expect![[r#"
            (
                Some(
                    100,
                ),
                Some(
                    0,
                ),
                Synced,
            )
        "#]]
        .assert_debug_eq(&(
            view.nodes[0].network_head_seqno,
            view.nodes[0].sync_lag_blocks,
            view.nodes[0].sync_status,
        ));
    }

    #[test]
    fn local_network_source_uses_one_canonical_head_sample() {
        let identity = ObserverIdentity::from_secret([15; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = telemetry(None);
        report.head_seqno = Some(55);
        report.head_observed_at = Some(100);
        store.publish(report, 100, 20).unwrap();

        let network = network_state(56, None);
        let view = store.aggregate(101, Some(&network), true);
        expect_test::expect![[r#"
            (
                Some(
                    55,
                ),
                Some(
                    56,
                ),
                Some(
                    56,
                ),
                Some(
                    0,
                ),
                Synced,
            )
        "#]]
        .assert_debug_eq(&(
            store.local().unwrap().payload.head_seqno,
            view.nodes[0].telemetry.head_seqno,
            view.nodes[0].network_head_seqno,
            view.nodes[0].sync_lag_blocks,
            view.nodes[0].sync_status,
        ));
    }

    #[test]
    fn historical_validator_key_matches_raw_block_creator() {
        let identity = ObserverIdentity::from_secret([9; 32]);
        let producing_key = hex::encode([3; 32]);
        let next_round_key = hex::encode([4; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = telemetry(Some(next_round_key));
        report.validator_public_keys = vec![producing_key];
        store.publish(report, 100, 20).unwrap();

        let mut network = network_state(7, None);
        network.production = vec![ProductionView {
            creator: hex::encode([3; 32]),
            masterchain_blocks: 1,
            shard_blocks: 0,
            last_block_at: 99,
        }];

        let view = store.aggregate(101, Some(&network), false);
        assert!(view.nodes[0].active_validator);
        assert_eq!(view.nodes[0].produced_masterchain_blocks, 1);
    }

    #[test]
    fn collector_combines_remote_telemetry_with_its_own_network_state() {
        let local = ObserverIdentity::from_secret([1; 32]);
        let mut store = ObservationStore::new("network".to_owned(), local, 600);
        let mut remote = ObservationStore::new(
            "network".to_owned(),
            ObserverIdentity::from_secret([2; 32]),
            600,
        );
        let mut report = telemetry(None);
        report.name = "remote".to_owned();
        report.head_seqno = Some(5);
        let signed = remote.publish(report, 101, 20).unwrap();
        assert!(store.ingest(signed, 102).unwrap());

        let network = network_state(7, None);
        let view = store.aggregate(102, Some(&network), false);
        let remote = &view.nodes[0];
        assert_eq!(view.chain.unwrap().seqno, 7);
        assert_eq!(remote.network_head_seqno, Some(7));
        assert_eq!(remote.sync_lag_blocks, Some(2));
        assert_eq!(remote.sync_status, SyncStatus::Synced);
        assert_eq!(view.totals.synchronized_nodes, 1);
        assert_eq!(view.totals.catching_up_nodes, 0);
    }

    #[test]
    fn disabled_active_validator_is_leaving_after_the_round() {
        let identity = ObserverIdentity::from_secret([13; 32]);
        let validator_key = hex::encode([5; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = telemetry(Some(validator_key));
        report.participate_in_elections = false;
        store.publish(report, 100, 20).unwrap();

        let mut network = network_state(
            7,
            Some(ElectionObservation {
                stage: ElectionStage::Validation,
                elections_open_at: 30,
                elections_close_at: 90,
                validators_elected_for: 120,
                stake_held_for: 30,
                previous: None,
                current: ValidatorSetObservation {
                    round_id: 0,
                    validation_started_at: 0,
                    validation_ended_at: 120,
                    validators: 1,
                    main_validators: 1,
                    total_weight: "1".to_owned(),
                    members: vec![ValidatorObservation {
                        public_key: hex::encode([5; 32]),
                        adnl_address: None,
                        weight: "1".to_owned(),
                    }],
                },
                next: None,
            }),
        );
        network.current_validator_keys = Some(BTreeSet::from([hex::encode([5; 32])]));

        let view = store.aggregate(101, Some(&network), false);
        assert!(view.nodes[0].active_validator);
        assert_eq!(view.nodes[0].validator_status, ValidatorStatus::Leaving);
        assert_eq!(view.totals.active_validators, 1);
    }
}
