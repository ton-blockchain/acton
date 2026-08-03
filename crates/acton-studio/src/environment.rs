use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub config: CreateEnvironmentConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
        validators: Option<u16>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentRequest {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
        validators: u16,
    },
    RemoteTonNetwork {
        network: PublicTonNetwork,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicTonNetwork {
    Testnet,
    Mainnet,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentCapability {
    ApiV2,
    ApiV3,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentEndpoints {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_v2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_v3: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentNetwork {
    pub id: String,
    pub label: String,
    pub chain_id: i32,
    pub test_only: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentStatus {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnvironmentLifecycle {
    Managed,
    External,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub runtime_endpoints: EnvironmentEndpoints,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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
                EnvironmentCapability::Snapshots,
                EnvironmentCapability::Checkpoints,
            ],
            Self::FullTonNetwork { .. } => vec![
                EnvironmentCapability::ApiV2,
                EnvironmentCapability::ApiV3,
                EnvironmentCapability::Explorer,
                EnvironmentCapability::Integration,
                EnvironmentCapability::GramFaucet,
                EnvironmentCapability::Wallets,
                EnvironmentCapability::Simulator,
                EnvironmentCapability::Contracts,
            ],
            Self::RemoteTonNetwork { .. } => vec![
                EnvironmentCapability::ApiV2,
                EnvironmentCapability::ApiV3,
                EnvironmentCapability::Explorer,
                EnvironmentCapability::Integration,
                EnvironmentCapability::Wallets,
                EnvironmentCapability::Simulator,
                EnvironmentCapability::Contracts,
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
            },
            Self::ActonLocalnet { .. } => EnvironmentNetwork {
                id: "acton-localnet".to_owned(),
                label: "Acton localnet".to_owned(),
                chain_id: -3,
                test_only: true,
            },
            Self::FullTonNetwork { .. } => EnvironmentNetwork {
                id: "full-ton-network".to_owned(),
                label: "Local TON network".to_owned(),
                chain_id: -239,
                test_only: true,
            },
            Self::RemoteTonNetwork {
                network: PublicTonNetwork::Testnet,
            } => EnvironmentNetwork {
                id: "testnet".to_owned(),
                label: "Testnet".to_owned(),
                chain_id: -3,
                test_only: true,
            },
            Self::RemoteTonNetwork {
                network: PublicTonNetwork::Mainnet,
            } => EnvironmentNetwork {
                id: "mainnet".to_owned(),
                label: "Mainnet".to_owned(),
                chain_id: -239,
                test_only: false,
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

    fn shutdown(&self) -> EnvironmentRuntimeFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
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
