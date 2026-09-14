//! Adapts simulated JSON snapshots to Studio's shared snapshot lifecycle.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::time::sleep;

use super::{
    EnvironmentDriver, LOCALNET_READY_POLL_INTERVAL, LOCALNET_READY_TIMEOUT, LocalEnvironment,
    LocalProcessRuntimeInner, ensure_environment_not_deleted, restart_environment_locked,
};
use crate::environment::{
    EnvironmentConfig, EnvironmentRuntimeError, EnvironmentSnapshotOperation,
    EnvironmentSnapshotOperationKind as Kind, EnvironmentSnapshotOperationPhase as Phase,
    EnvironmentStatus,
};
use ton_localnet::snapshots::{Snapshot, SnapshotStore};

/// File inventory remains available without starting a stopped environment's HTTP server.
pub(super) async fn files<T: Send + 'static, E: std::fmt::Display + Send + 'static>(
    database: &Path,
    action: impl FnOnce(SnapshotStore) -> Result<T, E> + Send + 'static,
) -> Result<T, EnvironmentRuntimeError> {
    let store = SnapshotStore::for_database(database);
    tokio::task::spawn_blocking(move || action(store))
        .await
        .map_err(error)?
        .map_err(error)
}

/// Accepts one operation under the environment's lifecycle lock. A dropped HTTP request or
/// page navigation cannot cancel a save, interrupt restoration, or release that lock early.
pub(super) async fn start(
    runtime: Arc<LocalProcessRuntimeInner>,
    environment: Arc<LocalEnvironment>,
    kind: Kind,
    name: Option<String>,
    snapshot_id: Option<String>,
) -> Result<EnvironmentSnapshotOperation, EnvironmentRuntimeError> {
    let guard = Arc::clone(&environment.lifecycle)
        .try_lock_owned()
        .map_err(|_| EnvironmentRuntimeError::Conflict {
            code: "environment_busy",
            message: "Another environment operation is running".to_owned(),
        })?;
    ensure_environment_not_deleted(&environment).await?;

    let status = environment.details.read().await.status;
    if !matches!(
        status,
        EnvironmentStatus::Running | EnvironmentStatus::Stopped
    ) {
        return Err(EnvironmentRuntimeError::Conflict {
            code: "environment_not_ready",
            message: "Wait until the environment has started or stopped before using snapshots"
                .to_owned(),
        });
    }

    let operation = EnvironmentSnapshotOperation {
        kind,
        phase: Phase::Preparing,
        started_at: chrono::Utc::now().to_rfc3339(),
        finished_at: None,
        snapshot_id,
        snapshot_name: name,
        startup_timings: None,
        error: None,
    };
    *environment.snapshot_operation.write().await = Some(operation.clone());
    let accepted = operation.clone();

    tokio::spawn(async move {
        let started = Instant::now();
        let target = environment.details.read().await.id.clone();
        tracing::info!(
            operation = ?kind,
            node = %target,
            outcome = "started",
            "Starting simulated snapshot operation"
        );

        let result = run(&runtime, &environment, &operation, status).await;
        let mut finished = operation;
        finished.finished_at = Some(chrono::Utc::now().to_rfc3339());

        match result {
            Ok(snapshot) => {
                finished.phase = Phase::Completed;
                finished.snapshot_id = Some(snapshot.id);
                finished.snapshot_name = snapshot.name;
                tracing::info!(
                    operation = ?kind,
                    node = %target,
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "completed",
                    "Simulated snapshot operation completed"
                );
            }
            Err(error) => {
                finished.phase = Phase::Failed;
                finished.error = Some(error.to_string());
                tracing::error!(
                    operation = ?kind,
                    node = %target,
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "failed",
                    error = %error,
                    "Simulated snapshot operation failed"
                );
            }
        }

        *environment.snapshot_operation.write().await = Some(finished);
        drop(guard);
    });

    Ok(accepted)
}

async fn run(
    runtime: &Arc<LocalProcessRuntimeInner>,
    environment: &Arc<LocalEnvironment>,
    operation: &EnvironmentSnapshotOperation,
    status: EnvironmentStatus,
) -> Result<Snapshot, EnvironmentRuntimeError> {
    let EnvironmentDriver::ActonSimulatedLocalnet {
        port,
        db_path,
        config,
        ..
    } = &environment.driver
    else {
        unreachable!()
    };

    if operation.kind == Kind::Create && status == EnvironmentStatus::Stopped {
        set_phase(environment, Phase::SavingState).await;
        let EnvironmentConfig::ActonSimulatedLocalnet {
            fork_network,
            fork_block_number,
            ..
        } = config
        else {
            unreachable!()
        };

        let database = db_path.clone();
        let name = operation.snapshot_name.clone();
        let network = fork_network.clone();
        let block = *fork_block_number;

        return files(db_path, move |store| {
            store.create_from_database(&database, name, network, block)
        })
        .await;
    }

    let client = reqwest::Client::new();
    if status == EnvironmentStatus::Stopped {
        // Match Full localnet restoration: the restored environment becomes available to use.
        restart_environment_locked(runtime, environment).await?;

        let deadline = Instant::now() + LOCALNET_READY_TIMEOUT;
        loop {
            if let Ok(response) = client
                .get(format!("http://127.0.0.1:{port}/acton_nodeInfo"))
                .timeout(Duration::from_secs(1))
                .send()
                .await
                && response.status().is_success()
            {
                break;
            }

            if Instant::now() >= deadline {
                return Err(error(
                    "Timed out waiting for the simulated node before restoring its snapshot",
                ));
            }

            sleep(LOCALNET_READY_POLL_INTERVAL).await;
        }
    }

    let (method, payload, phase) = match operation.kind {
        Kind::Create => (
            "acton_createSnapshot",
            serde_json::json!({"name": operation.snapshot_name}),
            Phase::SavingState,
        ),
        Kind::Restore => (
            "acton_restoreSnapshot",
            serde_json::json!({"id": operation.snapshot_id}),
            Phase::RestoringState,
        ),
    };
    set_phase(environment, phase).await;

    let response = client
        .post(format!("http://127.0.0.1:{port}/{method}"))
        .json(&payload)
        .send()
        .await
        .map_err(error)?;

    let status = response.status();
    let body: serde_json::Value = response.json().await.map_err(error)?;
    if !status.is_success() || body["ok"] == false {
        return Err(error(
            body["error"]
                .as_str()
                .unwrap_or("Simulated snapshot operation failed"),
        ));
    }

    serde_json::from_value(body["result"].clone()).map_err(error)
}

async fn set_phase(environment: &LocalEnvironment, phase: Phase) {
    if let Some(operation) = environment.snapshot_operation.write().await.as_mut() {
        operation.phase = phase;
    }
}

fn error(error: impl std::fmt::Display) -> EnvironmentRuntimeError {
    EnvironmentRuntimeError::Internal {
        code: "snapshot_failed",
        message: error.to_string(),
    }
}
