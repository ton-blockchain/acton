//! One owner per state directory and one mutation at a time per network.

mod admin;
mod health;
mod lifecycle;
mod nodes;
mod operations;
mod progress;
mod readiness;

use crate::{Error, Network, Operation, OperationStatus, Status};
use crate::{docker::DockerNetwork, storage};
pub(crate) use operations::Action;
use operations::Context;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{Mutex, RwLock};

/// Shared service state. The filesystem lock stays held across every clone and
/// background operation, preventing independent runtimes from racing Docker.
#[derive(Clone)]
pub struct Runtime {
    inner: Arc<Inner>,
}

struct Inner {
    root: PathBuf,
    _lock: std::fs::File,
    entry: Arc<Entry>,
    admission: Mutex<bool>,
    closing: tokio::sync::watch::Sender<bool>,
    health_history: Mutex<VecDeque<crate::NetworkHealthSample>>,
}

struct Entry {
    data_dir: PathBuf,
    record: RwLock<Network>,
    mutation: Arc<Mutex<()>>,
    admin_operation: RwLock<Option<crate::AdminOperation>>,
}

impl Runtime {
    /// Opens exactly one persisted network. The lock and every background task
    /// belong to this directory; sibling networks are never read or mutated.
    pub async fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        let lock = storage::lock(root.as_ref())?;
        let root =
            dunce::canonicalize(root.as_ref()).map_err(|e| Error::storage(root.as_ref(), e))?;
        let operations = root.join("operations");
        tokio::fs::create_dir_all(&operations)
            .await
            .map_err(|e| Error::storage(&operations, e))?;
        let path = root.join("network.json");
        let mut record: Network = storage::read_json(&path).await?;
        storage::validate_id(&record.id)?;
        if record.config.port_base == 0 || record.config.port_base > 65531 {
            return Err(Error::invalid("Invalid persisted port range"));
        }

        if root.join("admin-recovery.json").exists() {
            let driver = DockerNetwork::load(&root, &record).await?.ok_or_else(|| {
                Error::invalid("Administrative recovery requires its deployment descriptor")
            })?;
            driver.recover_admin().await?;
        }

        if let Some(op) = &mut record.operation
            && op.status == OperationStatus::Running
        {
            // Replaying a snapshot or topology mutation could destroy newer state.
            // Record the interruption and require an explicit retry.
            op.status = OperationStatus::Failed;
            op.error_code = Some("operation_interrupted".to_owned());
            op.error_status = Some(409);
            "interrupted".clone_into(&mut op.phase);
            op.error = Some("The service stopped before the operation completed; inspect status and logs before retrying".to_owned());
            storage::write_json(&operations.join(format!("{}.json", op.id)), op).await?;
        }
        if record
            .snapshot_operation
            .as_ref()
            .is_some_and(|op| op.status == OperationStatus::Running)
        {
            record.snapshot_operation = record.operation.clone();
        }
        if record.status != Status::Deleted {
            record.status = Status::Unknown;
        }
        storage::write_json(&path, &record).await?;

        Ok(Self {
            inner: Arc::new(Inner {
                entry: Arc::new(Entry {
                    data_dir: root.clone(),
                    record: RwLock::new(record),
                    mutation: Arc::new(Mutex::new(())),
                    admin_operation: RwLock::new(None),
                }),
                root,
                _lock: lock,
                admission: Mutex::new(true),
                closing: tokio::sync::watch::channel(false).0,
                health_history: Mutex::new(VecDeque::new()),
            }),
        })
    }

    /// Returns cached progress even while Docker or indexing is still starting.
    pub async fn get(&self) -> Network {
        self.inner.entry.record.read().await.clone()
    }

    /// Reads a durable operation by ID, including operations on deleted networks.
    pub async fn operation(&self, id: &str) -> Result<Operation, Error> {
        crate::inspection::operation(&self.inner.root, id).await
    }

    /// Lists archives while holding the deployment lock so a restore cannot
    /// replace the snapshot volume midway through the Localton command.
    pub async fn snapshots(&self) -> Result<Vec<crate::Snapshot>, Error> {
        let entry = self.entry().await?;
        let _guard = entry.mutation.try_lock().map_err(|_| Error::busy())?;

        if !entry.data_dir.join("runtime.json").exists() {
            return Ok(Vec::new());
        }

        self.driver(&entry).await?.list_snapshots().await
    }

    /// Returns a bounded tail of the deployment log; the complete log stays on
    /// disk so polling clients cannot exhaust memory with a large Docker log.
    pub async fn logs(&self, lines: usize) -> Result<String, Error> {
        let entry = self.entry().await?;
        crate::inspection::logs(&entry.data_dir, lines).await
    }

    async fn entry(&self) -> Result<Arc<Entry>, Error> {
        let entry = Arc::clone(&self.inner.entry);
        let record = entry.record.read().await;
        if record.status == Status::Deleted {
            return Err(Error::NotFound {
                environment_id: record.id.clone(),
            });
        }
        drop(record);
        Ok(entry)
    }

    async fn driver(&self, entry: &Entry) -> Result<DockerNetwork, Error> {
        let record = entry.record.read().await.clone();
        let driver = DockerNetwork::materialize(&entry.data_dir, &self.inner.root, &record).await?;
        entry.record.write().await.state = Some(driver.state_location());

        Ok(driver)
    }

    async fn save(entry: &Entry) -> Result<(), Error> {
        storage::write_json(
            &entry.data_dir.join("network.json"),
            &*entry.record.read().await,
        )
        .await
    }

    /// Rejects new mutations and asks startup work to stop at its next safe boundary.
    /// Snapshot archive writes finish before the network is stopped.
    pub async fn prepare_shutdown(&self) -> Result<(), Error> {
        let mut admission = self.inner.admission.lock().await;
        *admission = false;
        self.inner.closing.send_replace(true);
        drop(admission);
        Ok(())
    }
}
