//! Persisted network definitions and the shared CLI/HTTP response contract.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Immutable genesis settings and host ports.
///
/// Account `BoCs` are explicit copies of `ShardAccount` state; importing them creates
/// a new chain rather than a live fork.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateNetwork {
    pub name: String,
    pub port_base: Option<u16>,
    #[serde(default)]
    pub ports: PortOptions,
    #[serde(default)]
    pub reserved_ports: Vec<u16>,
    pub block_time_ms: Option<u32>,
    pub election_time_seconds: Option<u32>,
    #[serde(default)]
    pub imported_account_bocs: Vec<String>,
}

/// Optional host bindings for applications with independently configured endpoints.
/// Unspecified ports use the CLI's five-port range; reservations include stopped networks.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortOptions {
    pub config: Option<u16>,
    pub admin: Option<u16>,
    pub api_v2: Option<u16>,
    pub api_v3: Option<u16>,
    pub observability: Option<u16>,
}

/// Concrete endpoint bindings owned by one network, independent of its caller.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPorts {
    pub config: u16,
    pub admin: u16,
    pub api_v2: u16,
    pub api_v3: u16,
    pub observability: u16,
}

impl NetworkPorts {
    /// Enumerates bindings for reservation checks, including networks currently stopped.
    #[must_use]
    pub const fn all(self) -> [u16; 5] {
        [
            self.config,
            self.admin,
            self.api_v2,
            self.api_v3,
            self.observability,
        ]
    }
}

/// Selected ports and genesis inputs belong to this deployment for its lifetime.
/// Docker image/context identity is pinned separately in the runtime descriptor.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub port_base: u16,
    pub ports: Option<NetworkPorts>,
    pub block_time_ms: Option<u32>,
    pub election_time_seconds: Option<u32>,
    pub imported_account_bocs: Vec<String>,
}

/// Host endpoints remain assigned while an environment is stopped. Availability
/// follows `Network::status`, not the presence of a URL in this structure.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    pub api_v2: String,
    pub api_v3: String,
    pub admin: String,
    pub config: String,
    pub observability: String,
}

impl NetworkConfig {
    /// Returns the deployment's actual bindings, including individually selected ports.
    #[must_use]
    pub fn ports(&self) -> NetworkPorts {
        self.ports.unwrap_or(NetworkPorts {
            config: self.port_base,
            admin: self.port_base + 1,
            api_v2: self.port_base + 2,
            api_v3: self.port_base + 3,
            observability: self.port_base + 4,
        })
    }

    pub(crate) fn endpoints(&self) -> Endpoints {
        let ports = self.ports();
        Endpoints {
            config: format!("http://127.0.0.1:{}", ports.config),
            admin: format!("http://127.0.0.1:{}", ports.admin),
            api_v2: format!("http://127.0.0.1:{}/api/v2", ports.api_v2),
            api_v3: format!("http://127.0.0.1:{}/api/v3", ports.api_v3),
            observability: format!("http://127.0.0.1:{}", ports.observability),
        }
    }
}

/// An additional host-managed node. The genesis owner is never removable through
/// this collection. Validator entry and exit follow actual TON election rounds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub name: String,
    pub validator: bool,
    pub port_base: u16,
}

/// Observed lifecycle state. `Unknown` means Docker state has not been confirmed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
    Unknown,
    Deleted,
}

/// Fresh aggregate health for the network's APIs and Docker services.
///
/// This response is sampled on demand and is never persisted as lifecycle state.
/// Consumers must use `observed_at_ms` to decide whether a sample is still current.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkHealth {
    pub observed_at_ms: u64,
    pub status: NetworkHealthStatus,
    pub api_v2: ApiHealth,
    pub api_v3: ApiHealth,
    pub indexer_lag_blocks: Option<u32>,
    pub estimated_indexer_lag_ms: Option<u64>,
    pub services: Vec<ServiceHealth>,
    pub history: Vec<NetworkHealthSample>,
    pub infrastructure_error: Option<String>,
}

/// User-facing health summary derived from probes and required service state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum NetworkHealthStatus {
    Healthy,
    Syncing,
    Degraded,
    Stopped,
}

/// Result of a synthetic request to one TON HTTP API.
///
/// Latency measures this probe only. It is not a percentile of application traffic.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiHealth {
    pub status: ApiHealthStatus,
    pub endpoint: String,
    pub latency_ms: Option<u64>,
    pub masterchain_seqno: Option<u32>,
    pub block_time_unix: Option<u64>,
    pub block_age_ms: Option<u64>,
    pub error: Option<String>,
}

/// Readiness of one API endpoint at the sample time.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ApiHealthStatus {
    Ready,
    Syncing,
    Unavailable,
    Stopped,
}

/// Current Compose state for one long-running service or one-shot setup job.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {
    pub name: String,
    pub status: ServiceHealthStatus,
    pub state: Option<String>,
    pub health: Option<String>,
    pub exit_code: Option<i32>,
}

/// Normalized service state used by clients instead of Docker-specific strings.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ServiceHealthStatus {
    Ready,
    Starting,
    Completed,
    Stopped,
    Failed,
    Unknown,
}

/// Bounded time-series point retained by the owning localnet service.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkHealthSample {
    pub observed_at_ms: u64,
    pub api_v2_latency_ms: Option<u64>,
    pub api_v3_latency_ms: Option<u64>,
    pub api_v2_seqno: Option<u32>,
    pub api_v3_seqno: Option<u32>,
    pub indexer_lag_blocks: Option<u32>,
    pub block_age_ms: Option<u64>,
}

/// A network's durable definition plus the latest observable runtime state.
/// The service is its sole writer; clients do not edit this record on disk.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub id: String,
    pub name: String,
    pub config: NetworkConfig,
    pub endpoints: Endpoints,
    pub nodes: Vec<Node>,
    pub state: Option<NetworkState>,
    pub status: Status,
    pub operation: Option<Operation>,
    #[serde(default)]
    pub snapshot_operation: Option<Operation>,
    #[serde(default)]
    pub startup_timings: Option<StartupTimings>,
    pub error: Option<String>,
}

/// Blockchain files live in a Docker volume, separate from service metadata.
/// The directory is inside the container; it is not a path on the CLI host.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkState {
    pub directory: String,
    pub volume: String,
}

/// Operation progress survives client disconnects. A service restart marks active
/// operations interrupted and reconciles Docker instead of replaying mutations.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub kind: String,
    pub phase: String,
    pub status: OperationStatus,
    pub started_at: u64,
    pub duration_ms: u64,
    pub progress: Option<OperationProgress>,
    #[serde(default)]
    pub completed_steps: Vec<OperationStep>,
    pub error: Option<String>,
    pub log_path: String,
    /// Preserve failure classification after the accepting HTTP request has ended.
    /// Clients can distinguish a rejected mutation from an infrastructure failure.
    #[serde(default)]
    pub error_code: Option<String>,
    #[serde(default)]
    pub error_status: Option<u16>,
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub snapshot_name: Option<String>,
    #[serde(default)]
    pub startup_timings: Option<StartupTimings>,
}

/// Completed phases remain visible even when they finish between client polls.
/// Durations are measured by the owning service, independently of client latency.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationStep {
    pub phase: String,
    pub duration_ms: u64,
}

/// Measured work within the current phase, shared by terminal and HTTP clients.
///
/// `total` is absent when Docker has not announced the complete workload. Clients
/// must then display the observed count without inventing a completion percentage.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    pub completed: u64,
    pub total: Option<u64>,
    pub unit: String,
    pub detail: String,
}

/// Terminal states are durable; closing a polling client never cancels `Running`.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationStatus {
    Running,
    Completed,
    Failed,
}

/// Archive metadata returned by Localton's snapshot implementation. Restoring an
/// archive requires a stopped network and rebuilding the derived indexer data.
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub format_version: u32,
    pub id: String,
    pub name: Option<String>,
    pub created_at: u64,
    pub archive_size_bytes: u64,
    pub state_size_bytes: u64,
    pub state_schema_version: u32,
    pub ton_release: String,
    pub masterchain_seqno: Option<u32>,
}

/// Service-measured readiness milestones, shared by terminal and application views.
/// Indexer readiness requires a parsed chain height within one block of the TON node.
#[derive(Clone, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartupTimings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compose_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ton_ready_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexer_ready_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_ready_ms: Option<u64>,
}
