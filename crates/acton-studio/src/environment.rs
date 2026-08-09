use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub config: CreateEnvironmentConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullTonAccountImport {
    pub source_environment_id: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip)]
    #[schema(ignore)]
    pub(crate) shard_account_boc_hex: Option<String>,
}

impl FullTonAccountImport {
    #[must_use]
    pub fn new(source_environment_id: impl Into<String>, address: impl Into<String>) -> Self {
        Self {
            source_environment_id: source_environment_id.into(),
            address: address.into(),
            name: None,
            shard_account_boc_hex: None,
        }
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CreateEnvironmentConfig {
    ActonLocalnet {
        port: Option<u16>,
        fork_network: Option<String>,
        fork_block_number: Option<u64>,
        #[serde(default)]
        accounts: Vec<String>,
        rate_limit: Option<u32>,
        response_delay_ms: Option<u64>,
        block_interval_ms: Option<u64>,
        #[serde(default)]
        no_mining: bool,
        #[serde(default)]
        mine_empty_blocks: bool,
    },
    FullTonNetwork {
        api_v2_port: Option<u16>,
        api_v3_port: Option<u16>,
        admin_port: Option<u16>,
        config_port: Option<u16>,
        validators: Option<u16>,
        #[serde(default)]
        imported_accounts: Vec<FullTonAccountImport>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentRequest {
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentSnapshotRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshot {
    pub format_version: u32,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub created_at: u64,
    pub archive_size_bytes: u64,
    pub state_size_bytes: u64,
    pub state_schema_version: u32,
    pub ton_release: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masterchain_seqno: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentSnapshotOperationKind {
    Create,
    Restore,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentSnapshotOperationPhase {
    Preparing,
    Stopping,
    CreatingArchive,
    RestoringState,
    ResettingIndexer,
    Starting,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStartupTimings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ton_ready_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_ready_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_ready_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentSnapshotOperation {
    pub kind: EnvironmentSnapshotOperationKind,
    pub phase: EnvironmentSnapshotOperationPhase,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_timings: Option<EnvironmentStartupTimings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl EnvironmentSnapshotOperation {
    #[must_use]
    pub const fn is_active(&self) -> bool {
        !matches!(
            self.phase,
            EnvironmentSnapshotOperationPhase::Completed
                | EnvironmentSnapshotOperationPhase::Failed
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EnvironmentConfig {
    ActonLocalnet {
        port: u16,
        fork_network: Option<String>,
        fork_block_number: Option<u64>,
        accounts: Vec<String>,
        rate_limit: Option<u32>,
        response_delay_ms: Option<u64>,
        block_interval_ms: Option<u64>,
        no_mining: bool,
        mine_empty_blocks: bool,
    },
    FullTonNetwork {
        api_v2_port: u16,
        api_v3_port: u16,
        admin_port: u16,
        config_port: u16,
        validators: u16,
        imported_accounts: Vec<FullTonAccountImport>,
    },
    RemoteTonNetwork {
        network: PublicTonNetwork,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum PublicTonNetwork {
    Testnet,
    Mainnet,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentCapability {
    ApiV2,
    ApiV3,
    ConfigApi,
    ControlApi,
    Explorer,
    Integration,
    GramFaucet,
    JettonFaucet,
    Wallets,
    Simulator,
    Contracts,
    ApiCalls,
    Mining,
    TimeTravel,
    Snapshots,
    Checkpoints,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentEndpoints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_v2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_v3: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentNetwork {
    pub id: String,
    pub label: String,
    pub chain_id: i32,
    pub test_only: bool,
    pub supports_actions: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentLifecycle {
    Managed,
    External,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StudioEnvironment {
    pub id: String,
    pub name: String,
    pub status: EnvironmentStatus,
    pub lifecycle: EnvironmentLifecycle,
    pub rpc_url: String,
    pub config: EnvironmentConfig,
    pub capabilities: Vec<EnvironmentCapability>,
    pub endpoints: EnvironmentEndpoints,
    pub network: EnvironmentNetwork,
    #[serde(skip)]
    #[schema(ignore)]
    pub runtime_endpoints: EnvironmentEndpoints,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_timings: Option<EnvironmentStartupTimings>,
}

impl StudioEnvironment {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        status: EnvironmentStatus,
        config: EnvironmentConfig,
        runtime_endpoints: EnvironmentEndpoints,
    ) -> Self {
        Self::with_lifecycle(
            id,
            name,
            status,
            EnvironmentLifecycle::Managed,
            config,
            runtime_endpoints,
        )
    }

    #[must_use]
    pub fn new_external(
        id: impl Into<String>,
        name: impl Into<String>,
        status: EnvironmentStatus,
        config: EnvironmentConfig,
        runtime_endpoints: EnvironmentEndpoints,
    ) -> Self {
        Self::with_lifecycle(
            id,
            name,
            status,
            EnvironmentLifecycle::External,
            config,
            runtime_endpoints,
        )
    }

    fn with_lifecycle(
        id: impl Into<String>,
        name: impl Into<String>,
        status: EnvironmentStatus,
        lifecycle: EnvironmentLifecycle,
        config: EnvironmentConfig,
        runtime_endpoints: EnvironmentEndpoints,
    ) -> Self {
        let capabilities = config.capabilities();
        let network = config.network();
        Self {
            id: id.into(),
            name: name.into(),
            status,
            lifecycle,
            rpc_url: String::new(),
            config,
            capabilities,
            endpoints: EnvironmentEndpoints::default(),
            network,
            runtime_endpoints,
            error: None,
            startup_timings: None,
        }
    }
}

impl EnvironmentConfig {
    #[must_use]
    pub fn capabilities(&self) -> Vec<EnvironmentCapability> {
        match self {
            Self::ActonLocalnet { .. } => vec![
                EnvironmentCapability::ApiV2,
                EnvironmentCapability::ApiV3,
                EnvironmentCapability::ControlApi,
                EnvironmentCapability::Explorer,
                EnvironmentCapability::Integration,
                EnvironmentCapability::GramFaucet,
                EnvironmentCapability::JettonFaucet,
                EnvironmentCapability::Wallets,
                EnvironmentCapability::Simulator,
                EnvironmentCapability::Contracts,
                EnvironmentCapability::ApiCalls,
                EnvironmentCapability::Mining,
                EnvironmentCapability::TimeTravel,
                EnvironmentCapability::Checkpoints,
            ],
            Self::FullTonNetwork { .. } => vec![
                EnvironmentCapability::ApiV2,
                EnvironmentCapability::ApiV3,
                EnvironmentCapability::ConfigApi,
                EnvironmentCapability::ControlApi,
                EnvironmentCapability::Explorer,
                EnvironmentCapability::Integration,
                EnvironmentCapability::GramFaucet,
                EnvironmentCapability::Wallets,
                EnvironmentCapability::Simulator,
                EnvironmentCapability::Contracts,
                EnvironmentCapability::ApiCalls,
                EnvironmentCapability::Snapshots,
            ],
            Self::RemoteTonNetwork { .. } => vec![
                EnvironmentCapability::ApiV2,
                EnvironmentCapability::ApiV3,
                EnvironmentCapability::Explorer,
                EnvironmentCapability::Integration,
                EnvironmentCapability::Wallets,
                EnvironmentCapability::Simulator,
                EnvironmentCapability::Contracts,
                EnvironmentCapability::ApiCalls,
            ],
        }
    }

    #[must_use]
    pub fn network(&self) -> EnvironmentNetwork {
        match self {
            Self::ActonLocalnet {
                fork_network: Some(network),
                ..
            } => EnvironmentNetwork {
                id: network.clone(),
                label: match network.as_str() {
                    "mainnet" => "Mainnet fork".to_owned(),
                    "testnet" => "Testnet fork".to_owned(),
                    _ => format!("{network} fork"),
                },
                chain_id: -3,
                test_only: true,
                supports_actions: false,
            },
            Self::ActonLocalnet { .. } => EnvironmentNetwork {
                id: "acton-localnet".to_owned(),
                label: "Simulated localnet".to_owned(),
                chain_id: -3,
                test_only: true,
                supports_actions: false,
            },
            Self::FullTonNetwork { .. } => EnvironmentNetwork {
                id: "full-ton-network".to_owned(),
                label: "Full localnet".to_owned(),
                chain_id: -3,
                test_only: true,
                supports_actions: true,
            },
            Self::RemoteTonNetwork {
                network: PublicTonNetwork::Testnet,
            } => EnvironmentNetwork {
                id: "testnet".to_owned(),
                label: "Testnet".to_owned(),
                chain_id: -3,
                test_only: true,
                supports_actions: true,
            },
            Self::RemoteTonNetwork {
                network: PublicTonNetwork::Mainnet,
            } => EnvironmentNetwork {
                id: "mainnet".to_owned(),
                label: "Mainnet".to_owned(),
                chain_id: -239,
                test_only: false,
                supports_actions: true,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EnvironmentRuntimeError {
    #[error("{message}")]
    InvalidRequest { code: &'static str, message: String },
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    #[error("Environment {environment_id} was not found")]
    NotFound { environment_id: String },
    #[error("{message}")]
    Internal { code: &'static str, message: String },
}

pub type EnvironmentRuntimeFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, EnvironmentRuntimeError>> + Send + 'a>>;

pub trait EnvironmentRuntime: Send + Sync {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>>;

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment>;

    fn create(
        &self,
        request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment>;

    fn update(
        &self,
        environment_id: &str,
        request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment>;

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()>;

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment>;

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment>;

    fn list_snapshots(
        &self,
        _environment_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Vec<EnvironmentSnapshot>> {
        snapshots_unavailable()
    }

    fn create_snapshot(
        &self,
        _environment_id: &str,
        _request: CreateEnvironmentSnapshotRequest,
    ) -> EnvironmentRuntimeFuture<'_, EnvironmentSnapshotOperation> {
        snapshots_unavailable()
    }

    fn restore_snapshot(
        &self,
        _environment_id: &str,
        _snapshot_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, EnvironmentSnapshotOperation> {
        snapshots_unavailable()
    }

    fn delete_snapshot(
        &self,
        _environment_id: &str,
        _snapshot_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, ()> {
        snapshots_unavailable()
    }

    fn snapshot_operation(
        &self,
        _environment_id: &str,
    ) -> EnvironmentRuntimeFuture<'_, Option<EnvironmentSnapshotOperation>> {
        snapshots_unavailable()
    }

    fn shutdown(&self) -> EnvironmentRuntimeFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn snapshots_unavailable<T>() -> EnvironmentRuntimeFuture<'static, T> {
    Box::pin(async {
        Err(EnvironmentRuntimeError::Conflict {
            code: "environment_snapshots_unavailable",
            message: "Snapshots are not available for this environment".to_owned(),
        })
    })
}

pub(crate) struct EmptyEnvironmentRuntime;

impl EnvironmentRuntime for EmptyEnvironmentRuntime {
    fn list(&self) -> EnvironmentRuntimeFuture<'_, Vec<StudioEnvironment>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn create(
        &self,
        _request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        Box::pin(async {
            Err(EnvironmentRuntimeError::Internal {
                code: "environment_runtime_unavailable",
                message: "Environment runtime is not configured".to_owned(),
            })
        })
    }

    fn get(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn update(
        &self,
        environment_id: &str,
        _request: UpdateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn delete(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, ()> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }
}
