use std::collections::{BTreeMap, HashMap};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use ton::ton_core::cell::TonHash;
use utoipa::ToSchema;
use uuid::Uuid;

const CONTRACT_REGISTRY_FORMAT_VERSION: u32 = 2;
const CONTRACT_REGISTRY_FILE_NAME: &str = "registry.json";
const MAX_DEPLOYMENT_CANDIDATES: usize = 128;
const DEPLOYMENT_CANDIDATE_TTL_MS: u64 = 24 * 60 * 60 * 1_000;

#[derive(Clone)]
pub struct ContractRegistryStore {
    inner: Arc<ContractRegistryStoreInner>,
}

struct ContractRegistryStoreInner {
    environments_root: Option<PathBuf>,
    registries: Mutex<HashMap<String, ContractRegistry>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContractRegistry {
    format_version: u32,
    contracts: BTreeMap<String, RegisteredContract>,
    deployment_candidates: BTreeMap<String, DeploymentCandidate>,
    address_names: BTreeMap<String, String>,
    compiler_abis: BTreeMap<String, SavedCompilerAbi>,
    verified_sources: BTreeMap<String, SavedVerifiedSource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractRegistryFormat {
    format_version: u32,
}

impl Default for ContractRegistry {
    fn default() -> Self {
        Self {
            format_version: CONTRACT_REGISTRY_FORMAT_VERSION,
            contracts: BTreeMap::new(),
            deployment_candidates: BTreeMap::new(),
            address_names: BTreeMap::new(),
            compiler_abis: BTreeMap::new(),
            verified_sources: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RegisteredContract {
    pub(crate) address: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DeploymentCandidate {
    pub(crate) address: String,
    pub(crate) code_hash: String,
    pub(crate) observed_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeploymentCandidateRegistration {
    pub(crate) canonical_address: String,
    pub(crate) display_address: String,
    pub(crate) code_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContractRegistration {
    pub(crate) canonical_address: String,
    pub(crate) display_address: String,
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SavedCompilerAbi {
    pub(crate) code_hash: String,
    #[schema(value_type = Object)]
    pub(crate) abi: Value,
    pub(crate) saved_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SavedVerifiedSource {
    pub(crate) artifact_id: String,
    pub(crate) code_hash: String,
    #[schema(value_type = Object)]
    pub(crate) source: Value,
    pub(crate) saved_at: u64,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterContractRequest {
    pub(crate) address: String,
    pub(crate) name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteContractRequest {
    pub(crate) address: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetAddressNameRequest {
    pub(crate) address: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterCompilerAbisRequest {
    pub(crate) entries: Vec<CompilerAbiRegistration>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompilerAbiRegistration {
    #[schema(value_type = Object)]
    pub(crate) abi: Value,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegisterVerifiedSourcesRequest {
    pub(crate) entries: Vec<VerifiedSourceRegistration>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct VerifiedSourceRegistration {
    pub(crate) code_hash: String,
    #[schema(value_type = Object)]
    pub(crate) source: Value,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodeHashRequest {
    pub(crate) code_hash: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ArtifactIdRequest {
    pub(crate) artifact_id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetVerifiedSourceRequest {
    pub(crate) address: Option<String>,
    pub(crate) code_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContractArtifact {
    pub(crate) artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entrypoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) compiler_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) compiler_version: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ContractSourceKind {
    Local,
    Fork,
    Network,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContractListEntry {
    pub(crate) address: String,
    pub(crate) status: String,
    pub(crate) code_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) abi_name: Option<String>,
    pub(crate) source_kind: ContractSourceKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) artifact: Option<ContractArtifact>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RegistrySnapshot {
    pub(crate) contracts: BTreeMap<String, RegisteredContract>,
    #[serde(skip)]
    pub(crate) deployment_candidates: BTreeMap<String, DeploymentCandidate>,
    pub(crate) address_names: BTreeMap<String, String>,
    pub(crate) compiler_abis: BTreeMap<String, SavedCompilerAbi>,
    pub(crate) verified_sources: BTreeMap<String, SavedVerifiedSource>,
}

impl RegistrySnapshot {
    pub(crate) fn address_name(&self, canonical_address: &str) -> Option<&str> {
        self.address_names
            .get(canonical_address)
            .map(String::as_str)
    }

    pub(crate) fn compiler_abi(&self, code_hash: &str) -> Option<&SavedCompilerAbi> {
        let code_hash = normalize_code_hash(code_hash);
        self.compiler_abis.get(&code_hash).or_else(|| {
            self.compiler_abis
                .values()
                .find(|entry| compiler_abi_aliases(&entry.abi).contains(&code_hash))
        })
    }

    pub(crate) fn latest_verified_source(&self, code_hash: &str) -> Option<&SavedVerifiedSource> {
        let code_hash = normalize_code_hash(code_hash);
        let mut related_hashes = vec![code_hash.clone()];
        for entry in self.compiler_abis.values() {
            let aliases = compiler_abi_aliases(&entry.abi);
            if entry.code_hash == code_hash || aliases.contains(&code_hash) {
                related_hashes.extend(aliases);
            }
        }
        related_hashes.sort();
        related_hashes.dedup();

        self.verified_sources
            .values()
            .filter(|source| related_hashes.contains(&source.code_hash))
            .max_by(|left, right| {
                left.saved_at
                    .cmp(&right.saved_at)
                    .then_with(|| left.artifact_id.cmp(&right.artifact_id))
            })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ContractRegistryError {
    #[error("Failed to {operation} {path}: {source}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Failed to parse contract registry {path}: {source}")]
    InvalidJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Unsupported contract registry format version {actual}; expected {expected} in {path}")]
    UnsupportedFormat {
        path: PathBuf,
        expected: u32,
        actual: u32,
    },
    #[error("{message}")]
    InvalidRegistration { message: String },
}

impl ContractRegistryStore {
    #[must_use]
    pub(crate) fn ephemeral() -> Self {
        Self::new(None)
    }

    #[must_use]
    pub fn for_project(project_root: impl AsRef<Path>) -> Self {
        Self::new(Some(
            project_root.as_ref().join(".studio").join("environments"),
        ))
    }

    fn new(environments_root: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(ContractRegistryStoreInner {
                environments_root,
                registries: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub(crate) async fn snapshot(
        &self,
        environment_id: &str,
    ) -> Result<RegistrySnapshot, ContractRegistryError> {
        let mut registries = self.inner.registries.lock().await;
        let registry = self.load_locked(&mut registries, environment_id)?;
        if prune_deployment_candidates(registry, unix_timestamp()) {
            self.persist(environment_id, registry)?;
        }
        let snapshot = RegistrySnapshot {
            contracts: registry.contracts.clone(),
            deployment_candidates: registry.deployment_candidates.clone(),
            address_names: registry.address_names.clone(),
            compiler_abis: registry.compiler_abis.clone(),
            verified_sources: registry.verified_sources.clone(),
        };
        drop(registries);
        Ok(snapshot)
    }

    pub(crate) async fn register_contract(
        &self,
        environment_id: &str,
        canonical_address: String,
        display_address: String,
        name: Option<String>,
    ) -> Result<RegisteredContract, ContractRegistryError> {
        let mut registered = self
            .register_contracts(
                environment_id,
                vec![ContractRegistration {
                    canonical_address,
                    display_address,
                    name,
                }],
            )
            .await?;
        Ok(registered
            .pop()
            .expect("one contract registration produces one result"))
    }

    pub(crate) async fn register_contracts(
        &self,
        environment_id: &str,
        registrations: Vec<ContractRegistration>,
    ) -> Result<Vec<RegisteredContract>, ContractRegistryError> {
        let changes = registrations
            .into_iter()
            .map(|registration| {
                let registered = RegisteredContract {
                    address: registration.display_address,
                };
                (
                    registration.canonical_address,
                    non_empty_text(registration.name),
                    registered,
                )
            })
            .collect::<Vec<_>>();
        let registered = changes
            .iter()
            .map(|(_, _, contract)| contract.clone())
            .collect();
        self.mutate(environment_id, |registry| {
            for (canonical_address, name, contract) in changes {
                registry
                    .contracts
                    .insert(canonical_address.clone(), contract);
                registry.deployment_candidates.remove(&canonical_address);
                if let Some(name) = name {
                    registry.address_names.insert(canonical_address, name);
                }
            }
        })
        .await?;
        Ok(registered)
    }

    pub(crate) async fn record_deployment_candidates(
        &self,
        environment_id: &str,
        candidates: Vec<DeploymentCandidateRegistration>,
    ) -> Result<usize, ContractRegistryError> {
        if candidates.is_empty() {
            return Ok(0);
        }

        let mut registries = self.inner.registries.lock().await;
        let mut registry = self.load_locked(&mut registries, environment_id)?.clone();
        let mut inserted = 0;
        let observed_at = unix_timestamp();
        let mut changed = prune_deployment_candidates(&mut registry, observed_at);
        for candidate in candidates {
            if registry
                .contracts
                .contains_key(&candidate.canonical_address)
            {
                continue;
            }
            let saved = DeploymentCandidate {
                address: candidate.display_address,
                code_hash: normalize_code_hash(&candidate.code_hash),
                observed_at,
            };
            if registry
                .deployment_candidates
                .insert(candidate.canonical_address, saved.clone())
                .as_ref()
                != Some(&saved)
            {
                inserted += 1;
                changed = true;
            }
        }
        changed |= prune_deployment_candidates(&mut registry, observed_at);
        if !changed {
            return Ok(0);
        }

        self.persist(environment_id, &registry)?;
        registries.insert(environment_id.to_owned(), registry);
        drop(registries);
        Ok(inserted)
    }

    pub(crate) async fn delete_contract(
        &self,
        environment_id: &str,
        canonical_address: &str,
    ) -> Result<(), ContractRegistryError> {
        self.mutate(environment_id, |registry| {
            registry.contracts.remove(canonical_address);
            registry.deployment_candidates.remove(canonical_address);
            registry.address_names.remove(canonical_address);
        })
        .await
    }

    pub(crate) async fn confirm_deployment_candidates(
        &self,
        environment_id: &str,
        canonical_addresses: &[String],
    ) -> Result<(), ContractRegistryError> {
        if canonical_addresses.is_empty() {
            return Ok(());
        }
        self.mutate(environment_id, |registry| {
            for canonical_address in canonical_addresses {
                let Some(candidate) = registry.deployment_candidates.remove(canonical_address)
                else {
                    continue;
                };
                registry
                    .contracts
                    .entry(canonical_address.clone())
                    .or_insert(RegisteredContract {
                        address: candidate.address,
                    });
            }
        })
        .await
    }

    pub(crate) async fn set_address_name(
        &self,
        environment_id: &str,
        canonical_address: String,
        name: String,
    ) -> Result<(), ContractRegistryError> {
        self.mutate(environment_id, |registry| {
            if name.trim().is_empty() {
                registry.address_names.remove(&canonical_address);
            } else {
                registry
                    .address_names
                    .insert(canonical_address, name.trim().to_owned());
            }
        })
        .await
    }

    pub(crate) async fn register_compiler_abis(
        &self,
        environment_id: &str,
        entries: &[CompilerAbiRegistration],
    ) -> Result<(), ContractRegistryError> {
        let saved_at = unix_timestamp();
        let mut saved_entries = Vec::new();
        for entry in entries {
            let code_hashes = compiler_abi_code_hashes(&entry.abi)?;
            let code_hash = code_hashes
                .into_iter()
                .next()
                .expect("compiler ABI has at least one code hash");
            saved_entries.push((
                code_hash.clone(),
                SavedCompilerAbi {
                    code_hash,
                    abi: entry.abi.clone(),
                    saved_at,
                },
            ));
        }

        self.mutate(environment_id, |registry| {
            registry.compiler_abis.extend(saved_entries);
        })
        .await
    }

    pub(crate) async fn delete_compiler_abi(
        &self,
        environment_id: &str,
        code_hash: &str,
    ) -> Result<(), ContractRegistryError> {
        let code_hash = normalize_code_hash(code_hash);
        self.mutate(environment_id, |registry| {
            let primary = registry.compiler_abis.iter().find_map(|(primary, entry)| {
                (primary == &code_hash || compiler_abi_aliases(&entry.abi).contains(&code_hash))
                    .then(|| primary.clone())
            });
            if let Some(primary) = primary {
                registry.compiler_abis.remove(&primary);
            }
        })
        .await
    }

    pub(crate) async fn register_verified_sources(
        &self,
        environment_id: &str,
        entries: &[VerifiedSourceRegistration],
    ) -> Result<(), ContractRegistryError> {
        let saved_at = unix_timestamp();
        let mut sources = Vec::with_capacity(entries.len());
        let mut compiler_abis = Vec::new();
        for entry in entries {
            let code_hash = normalize_code_hash(&entry.code_hash);
            if code_hash.is_empty() {
                return Err(ContractRegistryError::InvalidRegistration {
                    message: "verified source registration requires code_hash".to_owned(),
                });
            }
            let artifact_id = source_artifact_id(&entry.source);
            sources.push((
                artifact_id.clone(),
                SavedVerifiedSource {
                    artifact_id,
                    code_hash: code_hash.clone(),
                    source: entry.source.clone(),
                    saved_at,
                },
            ));
            if let Some(abi) = compiler_abi_from_verified_source(&code_hash, &entry.source) {
                compiler_abis.push((
                    code_hash.clone(),
                    SavedCompilerAbi {
                        code_hash,
                        abi,
                        saved_at,
                    },
                ));
            }
        }

        self.try_mutate(environment_id, |registry| {
            for (artifact_id, source) in sources {
                if let Some(existing) = registry.verified_sources.get(&artifact_id) {
                    if existing.code_hash != source.code_hash || existing.source != source.source {
                        return Err(ContractRegistryError::InvalidRegistration {
                            message: format!(
                                "source artifact {artifact_id} is immutable and already has different content"
                            ),
                        });
                    }
                    continue;
                }
                registry.verified_sources.insert(artifact_id, source);
            }
            registry.compiler_abis.extend(compiler_abis);
            Ok(())
        })
        .await
    }

    pub(crate) async fn delete_verified_source(
        &self,
        environment_id: &str,
        code_hash: &str,
    ) -> Result<(), ContractRegistryError> {
        let code_hash = normalize_code_hash(code_hash);
        self.mutate(environment_id, |registry| {
            registry
                .verified_sources
                .retain(|_, source| source.code_hash != code_hash);
        })
        .await
    }

    pub(crate) async fn delete_verified_source_artifact(
        &self,
        environment_id: &str,
        artifact_id: &str,
    ) -> Result<(), ContractRegistryError> {
        self.mutate(environment_id, |registry| {
            registry.verified_sources.remove(artifact_id);
        })
        .await
    }

    async fn mutate(
        &self,
        environment_id: &str,
        mutation: impl FnOnce(&mut ContractRegistry),
    ) -> Result<(), ContractRegistryError> {
        self.try_mutate(environment_id, |registry| {
            mutation(registry);
            Ok(())
        })
        .await
    }

    async fn try_mutate(
        &self,
        environment_id: &str,
        mutation: impl FnOnce(&mut ContractRegistry) -> Result<(), ContractRegistryError>,
    ) -> Result<(), ContractRegistryError> {
        let mut registries = self.inner.registries.lock().await;
        let mut registry = self.load_locked(&mut registries, environment_id)?.clone();
        mutation(&mut registry)?;
        self.persist(environment_id, &registry)?;
        registries.insert(environment_id.to_owned(), registry);
        drop(registries);
        Ok(())
    }

    fn load_locked<'a>(
        &self,
        registries: &'a mut HashMap<String, ContractRegistry>,
        environment_id: &str,
    ) -> Result<&'a mut ContractRegistry, ContractRegistryError> {
        if !registries.contains_key(environment_id) {
            let registry = self.load(environment_id)?;
            registries.insert(environment_id.to_owned(), registry);
        }
        Ok(registries
            .get_mut(environment_id)
            .expect("contract registry was inserted"))
    }

    fn load(&self, environment_id: &str) -> Result<ContractRegistry, ContractRegistryError> {
        let Some(path) = self.registry_path(environment_id) else {
            return Ok(ContractRegistry::default());
        };
        if !path.exists() {
            return Ok(ContractRegistry::default());
        }
        let bytes = std::fs::read(&path).map_err(|source| ContractRegistryError::Io {
            operation: "read",
            path: path.clone(),
            source,
        })?;
        let format: ContractRegistryFormat = serde_json::from_slice(&bytes).map_err(|source| {
            ContractRegistryError::InvalidJson {
                path: path.clone(),
                source,
            }
        })?;
        if format.format_version == 1 {
            let reset = ContractRegistry::default();
            self.persist(environment_id, &reset)?;
            return Ok(reset);
        }
        if format.format_version != CONTRACT_REGISTRY_FORMAT_VERSION {
            return Err(ContractRegistryError::UnsupportedFormat {
                path,
                expected: CONTRACT_REGISTRY_FORMAT_VERSION,
                actual: format.format_version,
            });
        }
        let mut registry: ContractRegistry = serde_json::from_slice(&bytes).map_err(|source| {
            ContractRegistryError::InvalidJson {
                path: path.clone(),
                source,
            }
        })?;
        if prune_deployment_candidates(&mut registry, unix_timestamp()) {
            self.persist(environment_id, &registry)?;
        }
        Ok(registry)
    }

    fn persist(
        &self,
        environment_id: &str,
        registry: &ContractRegistry,
    ) -> Result<(), ContractRegistryError> {
        let Some(path) = self.registry_path(environment_id) else {
            return Ok(());
        };
        let parent = path
            .parent()
            .expect("contract registry path always has a parent");
        std::fs::create_dir_all(parent).map_err(|source| ContractRegistryError::Io {
            operation: "create directory for",
            path: parent.to_path_buf(),
            source,
        })?;
        let bytes = serde_json::to_vec_pretty(registry).map_err(|source| {
            ContractRegistryError::InvalidJson {
                path: path.clone(),
                source,
            }
        })?;
        let temporary_path = parent.join(format!(".registry.{}.tmp", Uuid::new_v4()));
        let mut temporary_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|source| ContractRegistryError::Io {
                operation: "create temporary",
                path: temporary_path.clone(),
                source,
            })?;
        if let Err(source) = temporary_file
            .write_all(&bytes)
            .and_then(|()| temporary_file.sync_all())
        {
            drop(temporary_file);
            let _ = std::fs::remove_file(&temporary_path);
            return Err(ContractRegistryError::Io {
                operation: "write temporary",
                path: temporary_path,
                source,
            });
        }
        drop(temporary_file);
        if let Err(source) = std::fs::rename(&temporary_path, &path) {
            let _ = std::fs::remove_file(&temporary_path);
            return Err(ContractRegistryError::Io {
                operation: "replace",
                path,
                source,
            });
        }
        Ok(())
    }

    fn registry_path(&self, environment_id: &str) -> Option<PathBuf> {
        self.inner
            .environments_root
            .as_ref()
            .map(|root| root.join(environment_id).join(CONTRACT_REGISTRY_FILE_NAME))
    }
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    })
}

fn prune_deployment_candidates(registry: &mut ContractRegistry, now: u64) -> bool {
    let original_len = registry.deployment_candidates.len();
    registry.deployment_candidates.retain(|_, candidate| {
        now.saturating_sub(candidate.observed_at) <= DEPLOYMENT_CANDIDATE_TTL_MS
    });

    if registry.deployment_candidates.len() > MAX_DEPLOYMENT_CANDIDATES {
        let mut oldest = registry
            .deployment_candidates
            .iter()
            .map(|(address, candidate)| (candidate.observed_at, address.clone()))
            .collect::<Vec<_>>();
        oldest.sort_unstable();
        for (_, address) in oldest
            .into_iter()
            .take(registry.deployment_candidates.len() - MAX_DEPLOYMENT_CANDIDATES)
        {
            registry.deployment_candidates.remove(&address);
        }
    }

    registry.deployment_candidates.len() != original_len
}

fn normalize_code_hash(value: &str) -> String {
    let value = value.trim();
    value
        .parse::<TonHash>()
        .map_or_else(|_| value.to_ascii_lowercase(), |hash| hash.to_hex())
}

fn compiler_abi_code_hashes(abi: &Value) -> Result<Vec<String>, ContractRegistryError> {
    let hashes = compiler_abi_aliases(abi);
    if hashes.is_empty() {
        return Err(ContractRegistryError::InvalidRegistration {
            message: "compiler ABI registration requires abi.code_hashes[0]".to_owned(),
        });
    }
    Ok(hashes)
}

fn compiler_abi_aliases(abi: &Value) -> Vec<String> {
    let hashes = abi
        .get("code_hashes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_code_hash)
        .filter(|hash| !hash.is_empty())
        .collect::<Vec<_>>();
    let mut unique = Vec::with_capacity(hashes.len());
    for hash in hashes {
        if !unique.contains(&hash) {
            unique.push(hash);
        }
    }
    unique
}

fn source_artifact_id(source: &Value) -> String {
    source
        .pointer("/bundle/source_bundle_hash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or_else(
            || {
                let bytes = serde_json::to_vec(source).expect("JSON value must serialize");
                hex::encode(Sha256::digest(bytes))
            },
            ToOwned::to_owned,
        )
}

fn compiler_abi_from_verified_source(code_hash: &str, source: &Value) -> Option<Value> {
    let bundle = source.get("bundle")?;
    let compiler_abi = bundle
        .get("compiler_abi")
        .filter(|abi| abi.is_object())
        .cloned()
        .or_else(|| {
            bundle
                .get("files")
                .and_then(Value::as_array)?
                .iter()
                .find_map(|file| {
                    let path = file.get("path").and_then(Value::as_str)?;
                    if !path.ends_with(".abi.json") {
                        return None;
                    }
                    let content = file.get("content").and_then(Value::as_str)?;
                    serde_json::from_str::<Value>(content)
                        .ok()
                        .filter(Value::is_object)
                })
        })?;
    let display_name = compiler_abi
        .get("contract_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned);
    Some(serde_json::json!({
        "compiler_abi": compiler_abi,
        "display_name": display_name,
        "code_hashes": [code_hash],
        "links": [],
    }))
}

fn unix_timestamp() -> u64 {
    u64::try_from(Utc::now().timestamp_millis()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use serde_json::{Value, json};
    use tempfile::tempdir;

    use super::{
        CompilerAbiRegistration, ContractRegistryError, ContractRegistryStore,
        DeploymentCandidateRegistration, MAX_DEPLOYMENT_CANDIDATES, VerifiedSourceRegistration,
        unix_timestamp,
    };

    #[tokio::test]
    async fn registry_survives_store_recreation_and_keeps_project_metadata_together() {
        let project = tempdir().expect("temporary project");
        let store = ContractRegistryStore::for_project(project.path());
        store
            .register_contract(
                "environment-7",
                "0:1111".to_owned(),
                "EQContract".to_owned(),
                Some("  Treasury  ".to_owned()),
            )
            .await
            .expect("contract registration");
        store
            .set_address_name(
                "environment-7",
                "0:1111".to_owned(),
                "Treasury override".to_owned(),
            )
            .await
            .expect("address name registration");
        store
            .register_verified_sources(
                "environment-7",
                &[VerifiedSourceRegistration {
                    code_hash: "AABB".to_owned(),
                    source: json!({
                        "bundle": {
                            "source_bundle_hash": "bundle-1",
                            "compiler_abi": {
                                "contract_name": "Treasury",
                                "get_methods": []
                            }
                        }
                    }),
                }],
            )
            .await
            .expect("source registration");

        let snapshot = ContractRegistryStore::for_project(project.path())
            .snapshot("environment-7")
            .await
            .expect("persisted registry");
        let mut value = serde_json::to_value(snapshot).expect("serializable snapshot");
        for entry in ["compilerAbis", "verifiedSources"] {
            for item in value[entry]
                .as_object_mut()
                .expect("saved metadata must be an object")
                .values_mut()
            {
                item["savedAt"] = json!("<timestamp>");
            }
        }

        expect![[r#"
            {
              "addressNames": {
                "0:1111": "Treasury override"
              },
              "compilerAbis": {
                "aabb": {
                  "abi": {
                    "code_hashes": [
                      "aabb"
                    ],
                    "compiler_abi": {
                      "contract_name": "Treasury",
                      "get_methods": []
                    },
                    "display_name": "Treasury",
                    "links": []
                  },
                  "codeHash": "aabb",
                  "savedAt": "<timestamp>"
                }
              },
              "contracts": {
                "0:1111": {
                  "address": "EQContract"
                }
              },
              "verifiedSources": {
                "bundle-1": {
                  "artifactId": "bundle-1",
                  "codeHash": "aabb",
                  "savedAt": "<timestamp>",
                  "source": {
                    "bundle": {
                      "compiler_abi": {
                        "contract_name": "Treasury",
                        "get_methods": []
                      },
                      "source_bundle_hash": "bundle-1"
                    }
                  }
                }
              }
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&value).expect("snapshot JSON must serialize"));

        let environment_dir = project.path().join(".studio/environments/environment-7");
        let entries = std::fs::read_dir(environment_dir)
            .expect("registry directory")
            .map(|entry| {
                entry
                    .expect("registry directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(entries, ["registry.json"]);
    }

    #[tokio::test]
    async fn legacy_registry_is_reset_instead_of_loading_polluted_contracts() {
        let project = tempdir().expect("temporary project");
        let environment_dir = project.path().join(".studio/environments/testnet");
        std::fs::create_dir_all(&environment_dir).expect("registry directory");
        let registry_path = environment_dir.join("registry.json");
        std::fs::write(
            &registry_path,
            serde_json::to_vec_pretty(&json!({
                "formatVersion": 1,
                "contracts": {
                    "0:random": {"address": "EQRandom"}
                },
                "addressNames": {},
                "compilerAbis": {},
                "verifiedSources": {}
            }))
            .expect("legacy registry JSON"),
        )
        .expect("legacy registry");

        let snapshot = ContractRegistryStore::for_project(project.path())
            .snapshot("testnet")
            .await
            .expect("reset registry");
        assert!(snapshot.contracts.is_empty());
        let persisted: Value = serde_json::from_slice(
            &std::fs::read(registry_path).expect("reset registry must be persisted"),
        )
        .expect("reset registry JSON");

        expect![[r#"
            {
              "addressNames": {},
              "compilerAbis": {},
              "contracts": {},
              "deploymentCandidates": {},
              "formatVersion": 2,
              "verifiedSources": {}
            }"#]]
        .assert_eq(
            &serde_json::to_string_pretty(&persisted).expect("registry JSON must serialize"),
        );
    }

    #[tokio::test]
    async fn future_registry_is_rejected_without_overwriting_it() {
        let project = tempdir().expect("temporary project");
        let environment_dir = project.path().join(".studio/environments/testnet");
        std::fs::create_dir_all(&environment_dir).expect("registry directory");
        let registry_path = environment_dir.join("registry.json");
        let original = serde_json::to_vec_pretty(&json!({
            "formatVersion": 99,
            "futureField": "must survive",
        }))
        .expect("future registry JSON");
        std::fs::write(&registry_path, &original).expect("future registry");

        let error = ContractRegistryStore::for_project(project.path())
            .snapshot("testnet")
            .await
            .expect_err("future registry must be rejected");
        let error = match error {
            ContractRegistryError::UnsupportedFormat {
                expected, actual, ..
            } => format!("unsupported format: expected {expected}, actual {actual}"),
            error => format!("unexpected error: {error}"),
        };
        let current = std::fs::read(&registry_path).expect("future registry must remain readable");
        let files = std::fs::read_dir(environment_dir)
            .expect("registry directory")
            .map(|entry| {
                entry
                    .expect("registry entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();

        expect![[r#"
            {
              "error": "unsupported format: expected 2, actual 99",
              "files": [
                "registry.json"
              ],
              "unchanged": true
            }"#]]
        .assert_eq(
            &serde_json::to_string_pretty(&json!({
                "error": error,
                "unchanged": current == original,
                "files": files,
            }))
            .expect("summary JSON"),
        );
    }

    #[tokio::test]
    async fn pending_deployments_are_bounded_and_expire() {
        let project = tempdir().expect("temporary project");
        let environment_dir = project.path().join(".studio/environments/testnet");
        std::fs::create_dir_all(&environment_dir).expect("registry directory");
        let registry_path = environment_dir.join("registry.json");
        let observed_at = unix_timestamp();
        let mut candidates = serde_json::Map::new();
        candidates.insert(
            "stale".to_owned(),
            json!({
                "address": "EQStale",
                "codeHash": "00",
                "observedAt": 0,
            }),
        );
        for index in 0..(MAX_DEPLOYMENT_CANDIDATES + 2) {
            candidates.insert(
                format!("candidate-{index:03}"),
                json!({
                    "address": format!("EQ{index:03}"),
                    "codeHash": format!("{index:02x}"),
                    "observedAt": observed_at,
                }),
            );
        }
        std::fs::write(
            &registry_path,
            serde_json::to_vec_pretty(&json!({
                "formatVersion": 2,
                "contracts": {},
                "deploymentCandidates": candidates,
                "addressNames": {},
                "compilerAbis": {},
                "verifiedSources": {},
            }))
            .expect("registry JSON"),
        )
        .expect("registry");

        let snapshot = ContractRegistryStore::for_project(project.path())
            .snapshot("testnet")
            .await
            .expect("bounded registry");
        let persisted: Value =
            serde_json::from_slice(&std::fs::read(registry_path).expect("persisted registry"))
                .expect("persisted registry JSON");
        let keys = snapshot
            .deployment_candidates
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        expect![[r#"
            {
              "containsStale": false,
              "first": "candidate-002",
              "last": "candidate-129",
              "persistedCount": 128,
              "snapshotCount": 128
            }"#]]
        .assert_eq(
            &serde_json::to_string_pretty(&json!({
                "snapshotCount": snapshot.deployment_candidates.len(),
                "persistedCount": persisted["deploymentCandidates"]
                    .as_object()
                    .expect("persisted candidates")
                    .len(),
                "containsStale": snapshot.deployment_candidates.contains_key("stale"),
                "first": keys.first(),
                "last": keys.last(),
            }))
            .expect("summary JSON"),
        );
    }

    #[tokio::test]
    async fn deleting_contract_records_preserves_project_artifacts() {
        let store = ContractRegistryStore::ephemeral();
        store
            .register_verified_sources(
                "testnet",
                &[VerifiedSourceRegistration {
                    code_hash: "AA".to_owned(),
                    source: json!({
                        "bundle": {
                            "source_bundle_hash": "counter-source",
                            "compiler_abi": {
                                "contract_name": "Counter"
                            }
                        }
                    }),
                }],
            )
            .await
            .expect("source registration");
        store
            .register_contract(
                "testnet",
                "0:registered".to_owned(),
                "EQRegistered".to_owned(),
                Some("Registered".to_owned()),
            )
            .await
            .expect("contract registration");
        store
            .record_deployment_candidates(
                "testnet",
                vec![DeploymentCandidateRegistration {
                    canonical_address: "0:pending".to_owned(),
                    display_address: "EQPending".to_owned(),
                    code_hash: "AA".to_owned(),
                }],
            )
            .await
            .expect("pending deployment");
        store
            .set_address_name("testnet", "0:pending".to_owned(), "Pending".to_owned())
            .await
            .expect("pending name");

        store
            .delete_contract("testnet", "0:registered")
            .await
            .expect("delete registered contract");
        store
            .delete_contract("testnet", "0:pending")
            .await
            .expect("delete pending contract");

        let snapshot = store.snapshot("testnet").await.expect("registry snapshot");
        expect![[r#"
            {
              "addressNames": 0,
              "compilerAbis": 1,
              "contracts": 0,
              "pendingDeployments": 0,
              "verifiedSources": 1
            }"#]]
        .assert_eq(
            &serde_json::to_string_pretty(&json!({
                "contracts": snapshot.contracts.len(),
                "pendingDeployments": snapshot.deployment_candidates.len(),
                "addressNames": snapshot.address_names.len(),
                "compilerAbis": snapshot.compiler_abis.len(),
                "verifiedSources": snapshot.verified_sources.len(),
            }))
            .expect("summary JSON"),
        );
    }

    #[tokio::test]
    async fn clones_share_mutations_without_lost_updates() {
        let store = ContractRegistryStore::ephemeral();
        let first = store.clone();
        let second = store.clone();
        let (first_result, second_result) = tokio::join!(
            first.register_contract(
                "environment-1",
                "0:first".to_owned(),
                "EQFirst".to_owned(),
                None,
            ),
            second.register_contract(
                "environment-1",
                "0:second".to_owned(),
                "EQSecond".to_owned(),
                None,
            )
        );
        first_result.expect("first registration");
        second_result.expect("second registration");

        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        assert_eq!(snapshot.contracts.len(), 2);
    }

    #[tokio::test]
    async fn contract_name_has_one_address_name_source_of_truth() {
        let store = ContractRegistryStore::ephemeral();
        store
            .register_contract(
                "environment-1",
                "0:contract".to_owned(),
                "EQContract".to_owned(),
                Some("  Counter  ".to_owned()),
            )
            .await
            .expect("contract registration");

        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        assert_eq!(snapshot.address_name("0:contract"), Some("Counter"));
        assert_eq!(snapshot.contracts["0:contract"].address, "EQContract");
    }

    #[tokio::test]
    async fn deployment_candidates_are_confirmed_without_replacing_metadata() {
        let store = ContractRegistryStore::ephemeral();
        store
            .register_contract(
                "environment-1",
                "0:contract".to_owned(),
                "EQContract".to_owned(),
                Some("Counter".to_owned()),
            )
            .await
            .expect("manual contract registration");

        let recorded = store
            .record_deployment_candidates(
                "environment-1",
                vec![
                    DeploymentCandidateRegistration {
                        canonical_address: "0:contract".to_owned(),
                        display_address: "kQContract".to_owned(),
                        code_hash: "AA".to_owned(),
                    },
                    DeploymentCandidateRegistration {
                        canonical_address: "0:second".to_owned(),
                        display_address: "EQSecond".to_owned(),
                        code_hash: "BB".to_owned(),
                    },
                ],
            )
            .await
            .expect("deployment candidate registration");
        assert_eq!(recorded, 1);

        store
            .confirm_deployment_candidates("environment-1", &["0:second".to_owned()])
            .await
            .expect("deployment confirmation");

        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        assert_eq!(snapshot.contracts["0:contract"].address, "EQContract");
        assert_eq!(snapshot.address_name("0:contract"), Some("Counter"));
        assert_eq!(snapshot.contracts["0:second"].address, "EQSecond");
        assert!(snapshot.deployment_candidates.is_empty());
    }

    #[tokio::test]
    async fn invalid_abi_batch_is_rejected_without_partial_mutation() {
        let store = ContractRegistryStore::ephemeral();
        let error = store
            .register_compiler_abis(
                "environment-1",
                &[
                    CompilerAbiRegistration {
                        abi: json!({"code_hashes": ["AABB"]}),
                    },
                    CompilerAbiRegistration {
                        abi: json!({"contract_name": "Missing hash"}),
                    },
                ],
            )
            .await
            .expect_err("invalid ABI batch must fail");
        assert!(matches!(
            error,
            ContractRegistryError::InvalidRegistration { .. }
        ));
        assert!(
            store
                .snapshot("environment-1")
                .await
                .expect("registry snapshot")
                .compiler_abis
                .is_empty()
        );
    }

    #[tokio::test]
    async fn code_hashes_use_one_canonical_key_for_hex_and_base64() {
        let store = ContractRegistryStore::ephemeral();
        let hex_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let base64_hash = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        store
            .register_compiler_abis(
                "environment-1",
                &[CompilerAbiRegistration {
                    abi: json!({
                        "code_hashes": [base64_hash],
                        "contract_name": "Counter",
                    }),
                }],
            )
            .await
            .expect("base64 ABI registration");
        store
            .register_verified_sources(
                "environment-1",
                &[VerifiedSourceRegistration {
                    code_hash: base64_hash.to_owned(),
                    source: json!({
                        "bundle": {
                            "source_bundle_hash": "zero-hash-source",
                        }
                    }),
                }],
            )
            .await
            .expect("base64 source registration");

        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        assert_eq!(
            snapshot
                .compiler_abi(hex_hash)
                .and_then(|entry| entry.abi["contract_name"].as_str()),
            Some("Counter")
        );
        assert_eq!(snapshot.compiler_abis.len(), 1);
        assert!(snapshot.compiler_abis.contains_key(hex_hash));
        assert_eq!(
            snapshot
                .latest_verified_source(hex_hash)
                .map(|source| source.artifact_id.as_str()),
            Some("zero-hash-source")
        );
    }

    #[tokio::test]
    async fn deleting_one_source_artifact_preserves_other_revisions() {
        let store = ContractRegistryStore::ephemeral();
        store
            .register_verified_sources(
                "environment-1",
                &[
                    VerifiedSourceRegistration {
                        code_hash: "AA".to_owned(),
                        source: json!({"bundle": {"source_bundle_hash": "old"}}),
                    },
                    VerifiedSourceRegistration {
                        code_hash: "AA".to_owned(),
                        source: json!({"bundle": {"source_bundle_hash": "new"}}),
                    },
                ],
            )
            .await
            .expect("source history registration");
        store
            .delete_verified_source_artifact("environment-1", "old")
            .await
            .expect("delete old artifact");

        let sources = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot")
            .verified_sources;
        assert_eq!(sources.len(), 1);
        assert_eq!(sources["new"].artifact_id, "new");
    }

    #[tokio::test]
    async fn source_artifact_ids_are_content_addressed_and_immutable() {
        let store = ContractRegistryStore::ephemeral();
        let first = VerifiedSourceRegistration {
            code_hash: "AA".to_owned(),
            source: json!({"source": "same"}),
        };
        store
            .register_verified_sources("environment-1", std::slice::from_ref(&first))
            .await
            .expect("first source registration");
        let first_saved_at = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot")
            .verified_sources
            .values()
            .next()
            .expect("saved source")
            .saved_at;
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        store
            .register_verified_sources("environment-1", &[first])
            .await
            .expect("identical source registration");
        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        assert_eq!(snapshot.verified_sources.len(), 1);
        assert_eq!(
            snapshot
                .verified_sources
                .keys()
                .next()
                .expect("content-addressed artifact"),
            "743e3e54001ffb7f2a8a58af99dd7aaa1a4de3c96db65304eecc7af8e6243f72"
        );
        assert_eq!(
            snapshot
                .verified_sources
                .values()
                .next()
                .expect("saved source")
                .saved_at,
            first_saved_at
        );

        store
            .register_verified_sources(
                "environment-1",
                &[VerifiedSourceRegistration {
                    code_hash: "BB".to_owned(),
                    source: json!({
                        "bundle": {"source_bundle_hash": "fixed"},
                        "source": "first"
                    }),
                }],
            )
            .await
            .expect("fixed source registration");
        let error = store
            .register_verified_sources(
                "environment-1",
                &[VerifiedSourceRegistration {
                    code_hash: "BB".to_owned(),
                    source: json!({
                        "bundle": {"source_bundle_hash": "fixed"},
                        "source": "changed"
                    }),
                }],
            )
            .await
            .expect_err("artifact ID collision must fail");
        assert!(matches!(
            error,
            ContractRegistryError::InvalidRegistration { .. }
        ));
        assert_eq!(
            store
                .snapshot("environment-1")
                .await
                .expect("registry snapshot")
                .verified_sources["fixed"]
                .source["source"],
            "first"
        );
    }

    #[tokio::test]
    async fn abi_alias_selects_latest_source_revision() {
        let store = ContractRegistryStore::ephemeral();
        store
            .register_verified_sources(
                "environment-1",
                &[VerifiedSourceRegistration {
                    code_hash: "primary".to_owned(),
                    source: json!({"bundle": {"source_bundle_hash": "source-primary"}}),
                }],
            )
            .await
            .expect("source registration");
        store
            .register_compiler_abis(
                "environment-1",
                &[CompilerAbiRegistration {
                    abi: json!({"code_hashes": ["primary", "alias"]}),
                }],
            )
            .await
            .expect("ABI registration");

        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        assert_eq!(
            snapshot
                .latest_verified_source("ALIAS")
                .expect("source selected through ABI alias")
                .artifact_id,
            "source-primary"
        );
        assert_eq!(snapshot.compiler_abis.len(), 1);
        assert!(snapshot.compiler_abi("alias").is_some());

        store
            .delete_compiler_abi("environment-1", "alias")
            .await
            .expect("delete ABI through alias");
        assert!(
            store
                .snapshot("environment-1")
                .await
                .expect("registry snapshot")
                .compiler_abis
                .is_empty()
        );
    }

    #[tokio::test]
    async fn source_bundle_abi_file_is_registered_for_contract_enrichment() {
        let store = ContractRegistryStore::ephemeral();
        store
            .register_verified_sources(
                "environment-1",
                &[VerifiedSourceRegistration {
                    code_hash: "CAFE".to_owned(),
                    source: json!({
                        "bundle": {
                            "source_bundle_hash": "bundle-with-abi-file",
                            "files": [{
                                "path": "build/Counter.abi.json",
                                "content": "{\"contract_name\":\"Counter\",\"get_methods\":[]}"
                            }]
                        }
                    }),
                }],
            )
            .await
            .expect("source registration");

        let snapshot = store
            .snapshot("environment-1")
            .await
            .expect("registry snapshot");
        let abi = &snapshot
            .compiler_abi("cafe")
            .expect("compiler ABI derived from source bundle")
            .abi;
        assert_eq!(abi["display_name"], "Counter");
        assert_eq!(abi["compiler_abi"]["contract_name"], "Counter");
    }
}
