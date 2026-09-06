//! Standalone management of real TON development networks.
//!
//! The HTTP service owns state and serializes mutations. CLI clients may disconnect
//! without cancelling accepted operations. Studio uses the same process and HTTP client as the CLI.

pub mod admin;
pub use admin::{AdminOperation, AdminRequest};
pub mod activity;
pub mod catalog;
pub mod client;
mod docker;
mod error;
pub mod http;
pub mod inspection;
mod model;
mod network_config;
pub mod process;
mod runtime;
mod storage;

pub use error::Error;
pub use model::{
    ApiHealth, ApiHealthStatus, CreateNetwork, DockerContainer, Endpoints, Network, NetworkConfig,
    NetworkHealth, NetworkHealthSample, NetworkHealthStatus, NetworkPorts, NetworkState, Node,
    Operation, OperationProgress, OperationStatus, OperationStep, PortOptions, ServiceHealth,
    ServiceHealthStatus, Snapshot, StartupTimings, Status,
};
pub use network_config::UpdateNetworkConfig;
pub use runtime::Runtime;
pub use storage::{ServiceDescriptor, service_descriptor_path};
