use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub port: Option<u16>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
    #[serde(default)]
    pub accounts: Vec<String>,
    pub rate_limit: Option<u32>,
    pub response_delay_ms: Option<u64>,
    pub block_interval_ms: Option<u64>,
    #[serde(default)]
    pub no_mining: bool,
    #[serde(default)]
    pub mine_empty_blocks: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfig {
    pub port: u16,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
    pub accounts: Vec<String>,
    pub rate_limit: Option<u32>,
    pub response_delay_ms: Option<u64>,
    pub block_interval_ms: Option<u64>,
    pub no_mining: bool,
    pub mine_empty_blocks: bool,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioEnvironment {
    pub id: String,
    pub name: String,
    pub status: EnvironmentStatus,
    pub rpc_url: String,
    pub config: EnvironmentConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
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

    fn create(
        &self,
        request: CreateEnvironmentRequest,
    ) -> EnvironmentRuntimeFuture<'_, StudioEnvironment>;

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

    fn stop(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }

    fn restart(&self, environment_id: &str) -> EnvironmentRuntimeFuture<'_, StudioEnvironment> {
        let environment_id = environment_id.to_owned();
        Box::pin(async move { Err(EnvironmentRuntimeError::NotFound { environment_id }) })
    }
}
