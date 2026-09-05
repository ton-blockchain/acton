//! Studio's binding to the public Acton localnet process and HTTP protocol.
//! Docker, network mutations and startup readiness belong exclusively to acton-localnet.

use acton_localnet::{
    Network, NetworkHealth, Operation, OperationStatus, Status,
    catalog::{self, NetworkDirectory},
    client::Client,
    process::Launcher,
};
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};
use tokio::{process::Child, sync::Mutex};

use crate::environment::{
    EnvironmentConfig, EnvironmentRuntimeError, EnvironmentSnapshotOperation,
    EnvironmentSnapshotOperationKind, EnvironmentSnapshotOperationPhase, EnvironmentStatus,
    FullTonAccountImport,
};

pub(crate) struct FullLocalnet {
    pub(crate) location: NetworkDirectory,
    launcher: Launcher,
    auxiliary: Mutex<Option<Child>>,
    pub(crate) monitoring: std::sync::atomic::AtomicBool,
}

impl FullLocalnet {
    pub(crate) fn new(executable: &Path, workspace: &Path, location: NetworkDirectory) -> Self {
        Self {
            location,
            launcher: Launcher {
                executable: executable.to_owned(),
                project_root: workspace.to_owned(),
                catalog_root: root(workspace),
            },
            auxiliary: Mutex::new(None),
            monitoring: false.into(),
        }
    }

    pub(crate) fn spawn_start(&self) -> Result<Child, EnvironmentRuntimeError> {
        self.launcher.start(&self.location).map_err(error)
    }

    pub(crate) async fn shutdown_started(
        &self,
        child: &mut Child,
    ) -> Result<(), EnvironmentRuntimeError> {
        self.launcher
            .shutdown_started(&self.location, child)
            .await
            .map_err(error)
    }

    /// A stopped network can still serve snapshot commands. That auxiliary process
    /// is owned by this environment and is included in Studio's graceful shutdown.
    pub(crate) async fn client(&self) -> Result<Client, EnvironmentRuntimeError> {
        let mut owned = self.auxiliary.lock().await;
        let (client, child) = self
            .launcher
            .connect_or_start(self.location.clone())
            .await
            .map_err(error)?;
        if let Some(child) = child {
            if let Some(mut previous) = owned.take() {
                let _ = previous.wait().await;
            }
            *owned = Some(child);
        }
        drop(owned);
        Ok(client)
    }

    pub(crate) async fn network(&self) -> Result<Network, EnvironmentRuntimeError> {
        if let Ok(client) = Client::connect(&self.location.path).await
            && let Ok(network) = client.network().await
        {
            return Ok(network);
        }
        self.launcher.inspect(&self.location).await.map_err(error)
    }

    /// Reads live health from the process that owns Docker and the API probes.
    pub(crate) async fn health(&self) -> Result<NetworkHealth, EnvironmentRuntimeError> {
        self.client().await?.health().await.map_err(error)
    }

    pub(crate) async fn shutdown(&self) -> Result<(), EnvironmentRuntimeError> {
        if let Ok(client) = Client::connect(&self.location.path).await {
            client.shutdown().await.map_err(error)?;
        } else if !matches!(
            self.network().await?.status,
            Status::Stopped | Status::Deleted
        ) {
            // Reconcile orphaned containers through their own service after a
            // process failure. Studio never runs Docker cleanup itself.
            self.client().await?.shutdown().await.map_err(error)?;
        }
        let owned = self.auxiliary.lock().await.take();
        if let Some(mut child) = owned {
            acton_localnet::process::terminate(&mut child)
                .await
                .map_err(error)?;
        }
        Ok(())
    }
}

pub(crate) fn root(workspace: &Path) -> PathBuf {
    workspace.join(".acton-localnet")
}

pub(crate) async fn find_network(
    workspace: &Path,
    id: &str,
) -> Result<NetworkDirectory, EnvironmentRuntimeError> {
    catalog::list(&root(workspace))
        .await
        .map_err(error)?
        .into_iter()
        .find(|entry| entry.network.id == id)
        .ok_or_else(|| {
            error(acton_localnet::Error::NotFound {
                environment_id: id.to_owned(),
            })
        })
}

/// Only import source labels belong to Studio. All executable settings are projected
/// from localnet's record, so CLI mutations are reflected in the same environment.
pub(crate) fn configuration(
    network: &Network,
    imported_accounts: Vec<FullTonAccountImport>,
) -> EnvironmentConfig {
    let ports = network.config.ports();
    EnvironmentConfig::FullTonNetwork {
        api_v2_port: ports.api_v2,
        api_v3_port: ports.api_v3,
        admin_port: ports.admin,
        config_port: ports.config,
        observability_port: ports.observability,
        block_time_ms: network.config.block_time_ms,
        election_time_seconds: network.config.election_time_seconds,
        imported_accounts,
        nodes: network.nodes.clone(),
    }
}

pub(crate) const fn status(status: Status) -> EnvironmentStatus {
    match status {
        Status::Stopped | Status::Deleted => EnvironmentStatus::Stopped,
        Status::Starting => EnvironmentStatus::Starting,
        Status::Running => EnvironmentStatus::Running,
        Status::Stopping => EnvironmentStatus::Stopping,
        Status::Unknown | Status::Failed => EnvironmentStatus::Failed,
    }
}

/// Adapts the shared durable operation to Studio's existing snapshot presentation.
/// This contains no snapshot sequencing or completion decisions of its own.
pub(crate) fn snapshot_operation(operation: &Operation) -> EnvironmentSnapshotOperation {
    let phase = match operation.phase.as_str() {
        "preparing" => EnvironmentSnapshotOperationPhase::Preparing,
        "stopping" => EnvironmentSnapshotOperationPhase::Stopping,
        "creatingArchive" => EnvironmentSnapshotOperationPhase::CreatingArchive,
        "restoringState" => EnvironmentSnapshotOperationPhase::RestoringState,
        "resettingIndexer" => EnvironmentSnapshotOperationPhase::ResettingIndexer,
        _ if operation.status == OperationStatus::Completed => {
            EnvironmentSnapshotOperationPhase::Completed
        }
        _ if operation.status == OperationStatus::Failed => {
            EnvironmentSnapshotOperationPhase::Failed
        }
        _ => EnvironmentSnapshotOperationPhase::Starting,
    };
    let started_ms = operation.started_at.saturating_mul(1000);
    let timestamp = |ms: u64| {
        DateTime::<Utc>::from_timestamp_millis(ms as i64)
            .unwrap_or_default()
            .to_rfc3339()
    };
    EnvironmentSnapshotOperation {
        kind: if operation.kind == "createSnapshot" {
            EnvironmentSnapshotOperationKind::Create
        } else {
            EnvironmentSnapshotOperationKind::Restore
        },
        phase,
        started_at: timestamp(started_ms),
        finished_at: (operation.status != OperationStatus::Running)
            .then(|| timestamp(started_ms.saturating_add(operation.duration_ms))),
        snapshot_id: operation.snapshot_id.clone(),
        snapshot_name: operation.snapshot_name.clone(),
        startup_timings: operation.startup_timings.clone(),
        error: operation.error.clone(),
    }
}

pub(crate) fn error(error: acton_localnet::Error) -> EnvironmentRuntimeError {
    let message = error.to_string();
    match error {
        acton_localnet::Error::InvalidRequest { code, .. } => {
            EnvironmentRuntimeError::InvalidRequest { code, message }
        }
        acton_localnet::Error::Conflict { code, .. } => {
            EnvironmentRuntimeError::Conflict { code, message }
        }
        acton_localnet::Error::Api { status: 400, .. } => EnvironmentRuntimeError::InvalidRequest {
            code: "localnet_invalid_request",
            message,
        },
        acton_localnet::Error::Api { status: 409, .. } => EnvironmentRuntimeError::Conflict {
            code: "localnet_conflict",
            message,
        },
        _ => EnvironmentRuntimeError::Internal {
            code: "localnet_failed",
            message,
        },
    }
}
