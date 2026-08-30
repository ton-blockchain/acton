//! Signed, peer-replicated observations for a Localton network.
//!
//! Process health is a self-report authenticated by a stable observer key.
//! Block production is kept separate: every observer derives block creators
//! from downloaded block data, and the aggregate prefers its own verified view
//! over a signed peer attestation.

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

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_OBSERVERS: usize = 1_024;
pub const MAX_EXCHANGE_OBSERVATIONS: usize = 128;
pub const SYNC_LAG_TOLERANCE_BLOCKS: u32 = 2;
const MAX_CLOCK_SKEW_SECONDS: u64 = 30;
const MAX_OBSERVATION_TTL_SECONDS: u64 = 5 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ChainHead {
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
    pub gen_utime: u32,
    pub observed_at: u64,
    pub shard_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct BlockObservation {
    pub id: String,
    pub workchain: i32,
    pub shard: String,
    pub seqno: u32,
    pub root_hash: String,
    pub file_hash: String,
    pub gen_utime: u32,
    pub creator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ChainObservation {
    pub head: ChainHead,
    pub window_started_at: u64,
    pub shards: Vec<ShardHead>,
    pub election: Option<ElectionObservation>,
    pub production: Vec<ProductionView>,
    pub blocks: Vec<BlockObservation>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ElectionObservation {
    pub round_id: u32,
    pub stage: String,
    pub validation_started_at: u32,
    pub elections_open_at: u32,
    pub elections_close_at: u32,
    pub next_set_activation_at: u32,
    pub validators_elected_for: u32,
    pub stake_held_for: u32,
    pub current_validators: u16,
    pub current_main_validators: u16,
    pub next_validators: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NodeObservation {
    pub name: String,
    pub public_ip: String,
    pub roles: Vec<String>,
    pub running: bool,
    pub process_id: Option<u32>,
    pub status: String,
    pub last_error: Option<String>,
    pub head_seqno: Option<u32>,
    pub sync_lag_blocks: Option<u32>,
    pub participate_in_elections: bool,
    pub current_validator: Option<bool>,
    pub next_validator: Option<bool>,
    pub validator_public_key: Option<String>,
    #[serde(default)]
    pub validator_public_keys: Vec<String>,
    pub validator_adnl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ObservationPayload {
    pub endpoint: String,
    pub software: String,
    pub launcher_started_at: Option<u64>,
    pub nodes: Vec<NodeObservation>,
    pub chain: Option<ChainObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct SignedObservation {
    pub protocol_version: u16,
    pub network_id: String,
    pub observer_id: String,
    pub public_key: String,
    pub sequence: u64,
    pub generated_at: u64,
    pub expires_at: u64,
    pub payload: ObservationPayload,
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
    payload: &'a ObservationPayload,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ExchangeRequest {
    pub known: BTreeMap<String, u64>,
    pub observations: Vec<SignedObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ExchangeResponse {
    pub known: BTreeMap<String, u64>,
    pub observations: Vec<SignedObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ObserverView {
    pub observer_id: String,
    pub endpoint: String,
    pub generated_at: u64,
    pub expires_at: u64,
    pub online: bool,
    pub node_count: usize,
}

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
    #[serde(flatten)]
    pub node: NodeObservation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Synced,
    CatchingUp,
    Unknown,
    Offline,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct ProductionView {
    pub creator: String,
    pub masterchain_blocks: u64,
    pub shard_blocks: u64,
    pub last_block_at: u32,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct NetworkView {
    pub protocol_version: u16,
    pub network_id: String,
    pub generated_at: u64,
    pub chain: Option<ChainHead>,
    pub chain_source: String,
    pub shards: Vec<ShardHead>,
    pub election: Option<ElectionObservation>,
    pub totals: NetworkTotals,
    pub observers: Vec<ObserverView>,
    pub nodes: Vec<NodeView>,
    pub production: Vec<ProductionView>,
}

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

pub struct ObservationStore {
    network_id: String,
    identity: ObserverIdentity,
    next_sequence: u64,
    retention_seconds: u64,
    observations: BTreeMap<String, SignedObservation>,
}

impl ObservationStore {
    pub fn new(network_id: String, identity: ObserverIdentity, retention_seconds: u64) -> Self {
        Self {
            network_id,
            identity,
            next_sequence: unix_time_millis().saturating_mul(1_000),
            retention_seconds,
            observations: BTreeMap::new(),
        }
    }

    pub fn publish(
        &mut self,
        payload: ObservationPayload,
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

    pub fn ingest(&mut self, incoming: Vec<SignedObservation>, now: u64) -> Result<usize> {
        ensure!(
            incoming.len() <= MAX_EXCHANGE_OBSERVATIONS,
            "exchange contains too many observations"
        );
        for observation in &incoming {
            verify_observation(observation, &self.network_id, now)?;
        }
        let new_observers = incoming
            .iter()
            .map(|observation| &observation.observer_id)
            .filter(|observer_id| !self.observations.contains_key(*observer_id))
            .collect::<BTreeSet<_>>()
            .len();
        ensure!(
            self.observations.len().saturating_add(new_observers) <= MAX_OBSERVERS,
            "observer store capacity exceeded"
        );
        let mut accepted = 0;
        for observation in incoming {
            let replace = self
                .observations
                .get(&observation.observer_id)
                .is_none_or(|current| observation.sequence > current.sequence);
            if replace {
                self.observations
                    .insert(observation.observer_id.clone(), observation);
                accepted += 1;
            }
        }
        self.prune(now);
        Ok(accepted)
    }

    pub fn known(&self) -> BTreeMap<String, u64> {
        self.observations
            .iter()
            .map(|(id, observation)| (id.clone(), observation.sequence))
            .collect()
    }

    pub fn local(&self) -> Option<SignedObservation> {
        self.observations.get(self.identity.observer_id()).cloned()
    }

    pub fn delta(&self, known: &BTreeMap<String, u64>, now: u64) -> Vec<SignedObservation> {
        self.observations
            .values()
            .filter(|observation| {
                observation.expires_at > now
                    && known
                        .get(&observation.observer_id)
                        .is_none_or(|sequence| observation.sequence > *sequence)
            })
            .take(MAX_EXCHANGE_OBSERVATIONS)
            .cloned()
            .collect()
    }

    pub fn endpoints(&self, now: u64) -> Vec<String> {
        let own_id = self.identity.observer_id();
        self.observations
            .values()
            .filter(|observation| observation.observer_id != own_id && observation.expires_at > now)
            .map(|observation| observation.payload.endpoint.clone())
            .collect()
    }

    pub fn aggregate(&mut self, now: u64) -> NetworkView {
        self.prune(now);
        let selected = self
            .observations
            .values()
            .filter_map(|observation| {
                observation
                    .payload
                    .chain
                    .as_ref()
                    .map(|chain| (observation, chain))
            })
            .max_by_key(|(_, chain)| (chain.head.seqno, chain.head.observed_at));
        let selected_chain = selected.map(|(_, chain)| chain);
        let chain_source = selected.map_or_else(
            || "unavailable".to_owned(),
            |(observation, _)| {
                if observation.observer_id == self.identity.observer_id() {
                    "local_verification".to_owned()
                } else {
                    "peer_attestation".to_owned()
                }
            },
        );
        let network_head = selected_chain.map(|chain| chain.head.seqno);
        let mut production = selected_chain
            .map(|chain| chain.production.clone())
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
                endpoint: observation.payload.endpoint.clone(),
                generated_at: observation.generated_at,
                expires_at: observation.expires_at,
                online: observer_online,
                node_count: observation.payload.nodes.len(),
            });
            for node in &observation.payload.nodes {
                let online = observer_online && node.running;
                let validator_keys = node
                    .validator_public_keys
                    .iter()
                    .chain(node.validator_public_key.iter())
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
                let current_membership = node.current_validator;
                let next_membership = node.next_validator;
                let active_validator = current_membership.unwrap_or(masterchain > 0 || shard > 0);
                let mut node = node.clone();
                node.sync_lag_blocks = network_head
                    .zip(node.head_seqno)
                    .map(|(network, node)| network.saturating_sub(node));
                let sync_status = sync_status(online, node.sync_lag_blocks);
                let validator_status = validator_status(
                    node.roles.iter().any(|role| role == "validator"),
                    node.participate_in_elections,
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
                    node,
                });
            }
        }
        observers.sort_by(|left, right| left.observer_id.cmp(&right.observer_id));
        nodes.sort_by(|left, right| {
            left.node
                .name
                .cmp(&right.node.name)
                .then_with(|| left.observer_id.cmp(&right.observer_id))
        });

        let chain = selected_chain.map(|chain| chain.head.clone());
        let shards = selected_chain
            .map(|chain| chain.shards.clone())
            .unwrap_or_default();
        let election = selected_chain.and_then(|chain| chain.election.clone());
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
                .filter(|node| node.node.roles.iter().any(|role| role == "validator"))
                .count(),
            active_validators: nodes.iter().filter(|node| node.active_validator).count(),
            full_nodes: nodes
                .iter()
                .filter(|node| node.node.roles.iter().any(|role| role == "full_node"))
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
            chain_source,
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
    validate_endpoint(&observation.payload.endpoint)?;
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

fn validate_endpoint(endpoint: &str) -> Result<()> {
    let url = reqwest::Url::parse(endpoint).context("invalid observation endpoint URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "observation endpoint must use http or https"
    );
    ensure!(url.host_str().is_some(), "observation endpoint has no host");
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "observation endpoint must not contain credentials"
    );
    Ok(())
}

fn sync_status(online: bool, lag: Option<u32>) -> SyncStatus {
    if !online {
        return SyncStatus::Offline;
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

const ED25519_PUBLIC_KEY_TAG: [u8; 4] = [0xc6, 0xb4, 0x13, 0x48];

pub(crate) fn public_key_hex(value: &str) -> Option<String> {
    let bytes = STANDARD.decode(value).ok()?;
    match bytes.as_slice() {
        raw if raw.len() == 32 => Some(hex::encode(raw)),
        tagged if tagged.len() == 36 && tagged.starts_with(&ED25519_PUBLIC_KEY_TAG) => {
            Some(hex::encode(&tagged[ED25519_PUBLIC_KEY_TAG.len()..]))
        }
        _ => None,
    }
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

    fn payload(endpoint: &str, creator_key: Option<String>) -> ObservationPayload {
        ObservationPayload {
            endpoint: endpoint.to_owned(),
            software: "localton/test".to_owned(),
            launcher_started_at: Some(10),
            nodes: vec![NodeObservation {
                name: "node".to_owned(),
                public_ip: "127.0.0.1".to_owned(),
                roles: vec!["full_node".to_owned(), "validator".to_owned()],
                running: true,
                process_id: Some(1),
                status: "running".to_owned(),
                last_error: None,
                head_seqno: Some(7),
                sync_lag_blocks: Some(0),
                participate_in_elections: true,
                current_validator: None,
                next_validator: None,
                validator_public_key: creator_key,
                validator_public_keys: Vec::new(),
                validator_adnl: None,
            }],
            chain: None,
        }
    }

    #[test]
    fn signed_observation_rejects_tampering() {
        let identity = ObserverIdentity::from_secret([7; 32]);
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut observation = store
            .publish(payload("http://127.0.0.1:18003", None), 100, 20)
            .unwrap();
        observation.payload.software = "tampered".to_owned();

        let peer_identity = ObserverIdentity::from_secret([8; 32]);
        let mut peer = ObservationStore::new("network".to_owned(), peer_identity, 600);
        assert!(peer.ingest(vec![observation], 101).is_err());
    }

    #[test]
    fn historical_tagged_validator_key_matches_raw_block_creator() {
        let identity = ObserverIdentity::from_secret([9; 32]);
        let producing_key = STANDARD.encode([ED25519_PUBLIC_KEY_TAG.as_slice(), &[3; 32]].concat());
        let next_round_key =
            STANDARD.encode([ED25519_PUBLIC_KEY_TAG.as_slice(), &[4; 32]].concat());
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = payload("http://127.0.0.1:18003", Some(next_round_key));
        report.nodes[0].validator_public_keys = vec![producing_key];
        report.chain = Some(ChainObservation {
            head: ChainHead {
                seqno: 7,
                root_hash: "root".to_owned(),
                file_hash: "file".to_owned(),
                gen_utime: 99,
                observed_at: 100,
                shard_count: 1,
            },
            window_started_at: 90,
            shards: Vec::new(),
            election: None,
            production: vec![ProductionView {
                creator: hex::encode([3; 32]),
                masterchain_blocks: 1,
                shard_blocks: 0,
                last_block_at: 99,
            }],
            blocks: vec![BlockObservation {
                id: "-1:8000000000000000:7".to_owned(),
                workchain: -1,
                shard: "8000000000000000".to_owned(),
                seqno: 7,
                root_hash: "root".to_owned(),
                file_hash: "file".to_owned(),
                gen_utime: 99,
                creator: hex::encode([3; 32]),
            }],
        });
        store.publish(report, 100, 20).unwrap();

        let view = store.aggregate(101);
        assert!(view.nodes[0].active_validator);
        assert_eq!(view.nodes[0].produced_masterchain_blocks, 1);
        assert_eq!(view.chain_source, "local_verification");
    }

    #[test]
    fn peer_chain_is_marked_as_an_attestation() {
        let local = ObserverIdentity::from_secret([1; 32]);
        let mut store = ObservationStore::new("network".to_owned(), local, 600);
        let block = BlockObservation {
            id: "0:8000000000000000:8".to_owned(),
            workchain: 0,
            shard: "8000000000000000".to_owned(),
            seqno: 8,
            root_hash: "root".to_owned(),
            file_hash: "file".to_owned(),
            gen_utime: 100,
            creator: hex::encode([3; 32]),
        };
        let mut peer = ObservationStore::new(
            "network".to_owned(),
            ObserverIdentity::from_secret([2; 32]),
            600,
        );
        let mut report = payload("http://192.0.2.1:18003", None);
        report.chain = Some(ChainObservation {
            head: ChainHead {
                seqno: 8,
                root_hash: "root".to_owned(),
                file_hash: "file".to_owned(),
                gen_utime: 100,
                observed_at: 101,
                shard_count: 1,
            },
            window_started_at: 90,
            shards: Vec::new(),
            election: None,
            production: vec![ProductionView {
                creator: block.creator.clone(),
                masterchain_blocks: 0,
                shard_blocks: 1,
                last_block_at: block.gen_utime,
            }],
            blocks: vec![block],
        });
        let signed = peer.publish(report, 101, 20).unwrap();
        store.ingest(vec![signed], 102).unwrap();

        let view = store.aggregate(102);
        assert_eq!(view.totals.shard_blocks, 1);
        assert_eq!(view.chain_source, "peer_attestation");
    }

    #[test]
    fn highest_peer_head_recomputes_node_lag() {
        let local_identity = ObserverIdentity::from_secret([11; 32]);
        let local_id = local_identity.observer_id().to_owned();
        let mut store = ObservationStore::new("network".to_owned(), local_identity, 600);
        let mut local_report = payload("http://127.0.0.1:18003", None);
        local_report.chain = Some(chain_observation(7, None));
        store.publish(local_report, 100, 20).unwrap();

        let mut peer = ObservationStore::new(
            "network".to_owned(),
            ObserverIdentity::from_secret([12; 32]),
            600,
        );
        let mut peer_report = payload("http://192.0.2.1:18003", None);
        peer_report.nodes[0].head_seqno = Some(12);
        peer_report.chain = Some(chain_observation(12, None));
        let signed = peer.publish(peer_report, 101, 20).unwrap();
        store.ingest(vec![signed], 101).unwrap();

        let view = store.aggregate(101);
        let local_node = view
            .nodes
            .iter()
            .find(|node| node.observer_id == local_id)
            .unwrap();
        assert_eq!(view.chain.unwrap().seqno, 12);
        assert_eq!(view.chain_source, "peer_attestation");
        assert_eq!(local_node.node.sync_lag_blocks, Some(5));
        assert_eq!(local_node.sync_status, SyncStatus::CatchingUp);
        assert_eq!(view.totals.synchronized_nodes, 1);
        assert_eq!(view.totals.catching_up_nodes, 1);
    }

    #[test]
    fn disabled_active_validator_is_leaving_after_the_round() {
        let identity = ObserverIdentity::from_secret([13; 32]);
        let validator_key = STANDARD.encode([ED25519_PUBLIC_KEY_TAG.as_slice(), &[5; 32]].concat());
        let mut store = ObservationStore::new("network".to_owned(), identity, 600);
        let mut report = payload("http://127.0.0.1:18003", Some(validator_key));
        report.nodes[0].participate_in_elections = false;
        report.nodes[0].current_validator = Some(true);
        report.chain = Some(chain_observation(
            7,
            Some(ElectionObservation {
                round_id: 120,
                stage: "validation".to_owned(),
                validation_started_at: 0,
                elections_open_at: 30,
                elections_close_at: 90,
                next_set_activation_at: 120,
                validators_elected_for: 120,
                stake_held_for: 30,
                current_validators: 1,
                current_main_validators: 1,
                next_validators: None,
            }),
        ));
        store.publish(report, 100, 20).unwrap();

        let view = store.aggregate(101);
        assert!(view.nodes[0].active_validator);
        assert_eq!(view.nodes[0].validator_status, ValidatorStatus::Leaving);
        assert_eq!(view.totals.active_validators, 1);
    }

    fn chain_observation(seqno: u32, election: Option<ElectionObservation>) -> ChainObservation {
        ChainObservation {
            head: ChainHead {
                seqno,
                root_hash: format!("root-{seqno}"),
                file_hash: format!("file-{seqno}"),
                gen_utime: 99,
                observed_at: 100,
                shard_count: 1,
            },
            window_started_at: 90,
            shards: Vec::new(),
            election,
            production: Vec::new(),
            blocks: Vec::new(),
        }
    }
}
