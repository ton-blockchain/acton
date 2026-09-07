//! Snapshots support for the localnet Docker runtime.

use super::{
    COMPOSE_DELETE_TIMEOUT, DockerNetwork, LOCALTON_SNAPSHOT_DIR, LOCALTON_STATE_DIR,
    SNAPSHOT_TIMEOUT,
};
use crate::{Error, Node, Snapshot, storage};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Instant,
};

const RECOVERY_FILE: &str = "snapshot-recovery.json";

/// Published only after every cold node archive is durable. Node definitions are
/// part of the snapshot: restoring must not attach nodes from a newer chain.
#[derive(Clone, Deserialize, Serialize)]
struct Bundle {
    snapshot: Snapshot,
    nodes: Vec<Node>,
    archives: BTreeMap<String, Snapshot>,
}

/// The target topology is retained too, so rollback can discard volumes belonging
/// only to the attempted restore before a future node reuses their IDs.
#[derive(Deserialize, Serialize)]
struct Recovery {
    backup: Bundle,
    target_nodes: Vec<Node>,
}

impl Bundle {
    /// Reject an incomplete or damaged manifest before replacing any node state.
    fn validate(&self) -> Result<(), Error> {
        storage::validate_id(&self.snapshot.id)?;
        if self.snapshot.format_version != 3
            || self.archives.len() != self.nodes.len() + 1
            || !self.archives.contains_key("localton")
            || self
                .nodes
                .iter()
                .any(|node| !self.archives.contains_key(&node.id))
        {
            return Err(Error::invalid(
                "The snapshot does not contain a complete network topology",
            ));
        }
        for (service, archive) in &self.archives {
            storage::validate_id(service)?;
            storage::validate_id(&archive.id)?;
        }
        Ok(())
    }

    fn directory(&self, service: &str) -> String {
        format!(
            "{LOCALTON_SNAPSHOT_DIR}/networks/{}/{service}",
            self.snapshot.id
        )
    }
}

/// Listing reads committed manifests, not Docker or the mutation lock. A client
/// can discover an in-flight operation without racing archive creation/deletion.
pub(crate) async fn list_snapshots(data_dir: &Path) -> Result<Vec<Snapshot>, Error> {
    let path = data_dir.join("snapshots");
    let mut entries = match tokio::fs::read_dir(&path).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(Error::storage(&path, error)),
    };
    let mut snapshots = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| Error::storage(&path, e))?
    {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            // A concurrent delete can remove a manifest after read_dir returned it.
            let bytes = match tokio::fs::read(entry.path()).await {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(Error::storage(&entry.path(), error)),
            };
            let bundle: Bundle =
                serde_json::from_slice(&bytes).map_err(|e| Error::storage(&entry.path(), e))?;
            snapshots.push(bundle.snapshot);
        }
    }
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    Ok(snapshots)
}

impl DockerNetwork {
    fn snapshot_path(&self, id: &str) -> PathBuf {
        self.compose_file
            .with_file_name("snapshots")
            .join(format!("{id}.json"))
    }

    pub(crate) fn has_snapshot_recovery(&self) -> bool {
        self.compose_file.with_file_name(RECOVERY_FILE).exists()
    }

    /// Captures all stopped nodes before publishing a single inventory entry.
    pub(crate) async fn create_snapshot(
        &self,
        name: Option<&str>,
        nodes: &[Node],
    ) -> Result<Snapshot, Error> {
        let bundle = self.capture_snapshot(name, nodes).await?;
        let path = self.snapshot_path(&bundle.snapshot.id);
        let directory = self.compose_file.with_file_name("snapshots");
        tokio::fs::create_dir_all(&directory)
            .await
            .map_err(|e| Error::storage(&directory, e))?;
        if let Err(error) = storage::write_json(&path, &bundle).await {
            self.cleanup_archives(&bundle).await;
            return Err(error);
        }
        Ok(bundle.snapshot)
    }

    async fn capture_snapshot(&self, name: Option<&str>, nodes: &[Node]) -> Result<Bundle, Error> {
        let id = format!("snapshot-{}", uuid::Uuid::new_v4());
        let directory = format!("{LOCALTON_SNAPSHOT_DIR}/networks/{id}/localton");
        let primary: Snapshot = self
            .snapshot_json("localton", &directory, &["create"])
            .await?;
        let mut snapshot = primary.clone();
        snapshot.id = id;
        snapshot.name = name.map(str::to_owned);
        snapshot.format_version = 3;
        let mut bundle = Bundle {
            snapshot,
            nodes: nodes.to_vec(),
            archives: BTreeMap::from([("localton".into(), primary)]),
        };

        for node in nodes {
            storage::validate_id(&node.id)?;
            let result = self
                .snapshot_json::<Snapshot>(&node.id, &bundle.directory(&node.id), &["create"])
                .await;
            match result {
                Ok(archive) => {
                    bundle.snapshot.archive_size_bytes += archive.archive_size_bytes;
                    bundle.snapshot.state_size_bytes += archive.state_size_bytes;
                    bundle.archives.insert(node.id.clone(), archive);
                }
                Err(error) => {
                    self.cleanup_archives(&bundle).await;
                    return Err(error);
                }
            }
        }
        Ok(bundle)
    }

    /// Keeps a durable rollback snapshot until the runtime commits the restored
    /// topology. On interruption the owner rolls back before admitting mutations.
    pub(crate) async fn restore_snapshot(
        &self,
        id: &str,
        nodes: &[Node],
    ) -> Result<(Snapshot, Vec<Node>), Error> {
        storage::validate_id(id)?;
        let bundle: Bundle = storage::read_json(&self.snapshot_path(id)).await?;
        bundle.validate()?;
        let backup = self.capture_snapshot(None, nodes).await?;
        storage::write_json(
            &self.compose_file.with_file_name(RECOVERY_FILE),
            &Recovery {
                backup,
                target_nodes: bundle.nodes.clone(),
            },
        )
        .await?;
        self.restore_bundle(&bundle).await?;
        Ok((bundle.snapshot, bundle.nodes))
    }

    async fn restore_bundle(&self, bundle: &Bundle) -> Result<(), Error> {
        bundle.validate()?;
        for (service, archive) in &bundle.archives {
            storage::validate_id(service)?;
            let _: Snapshot = self
                .snapshot_json(
                    service,
                    &bundle.directory(service),
                    &["restore", &archive.id],
                )
                .await?;
        }
        Ok(())
    }

    /// Leaves services stopped; only the runtime may restart after persisting the
    /// recovered topology. A failed recovery retains the journal for the next try.
    pub(crate) async fn recover_snapshot(&self) -> Result<Option<Vec<Node>>, Error> {
        if !self.has_snapshot_recovery() {
            return Ok(None);
        }
        let journal: Recovery =
            storage::read_json(&self.compose_file.with_file_name(RECOVERY_FILE)).await?;
        self.stop().await?;
        self.restore_bundle(&journal.backup).await?;
        self.reset_indexer().await?;
        self.remove_obsolete_nodes(&journal.target_nodes, &journal.backup.nodes)
            .await?;
        self.write_compose(&journal.backup.nodes).await?;
        Ok(Some(journal.backup.nodes))
    }

    pub(crate) async fn finish_snapshot_restore(&self, nodes: &[Node]) -> Result<(), Error> {
        let path = self.compose_file.with_file_name(RECOVERY_FILE);
        let journal: Recovery = storage::read_json(&path).await?;
        self.remove_obsolete_nodes(&journal.backup.nodes, nodes)
            .await?;
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| Error::storage(&path, e))?;
        self.cleanup_archives(&journal.backup).await;
        Ok(())
    }

    async fn cleanup_archives(&self, bundle: &Bundle) {
        if let Err(error) = self.delete_archives(bundle).await {
            log::warn!(
                "operation=snapshot_cleanup target={} outcome=failed error={error}",
                bundle.snapshot.id
            );
        }
    }

    async fn remove_obsolete_nodes(
        &self,
        previous: &[Node],
        restored: &[Node],
    ) -> Result<(), Error> {
        for node in previous
            .iter()
            .filter(|node| !restored.iter().any(|saved| saved.id == node.id))
        {
            let mut command = self.docker_command();
            command
                .args(["volume", "rm", "--force"])
                .arg(format!("{}_{}-state", self.project_name, node.id));
            self.run_command(
                command,
                "discard state of a node outside the restored topology",
                "environment_snapshot_restore_failed",
                COMPOSE_DELETE_TIMEOUT,
            )
            .await?;
        }
        Ok(())
    }

    pub(crate) async fn delete_snapshot(&self, id: &str) -> Result<(), Error> {
        storage::validate_id(id)?;
        let path = self.snapshot_path(id);
        let bundle: Bundle = storage::read_json(&path).await?;
        self.delete_archives(&bundle).await?;
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| Error::storage(&path, e))
    }

    async fn delete_archives(&self, bundle: &Bundle) -> Result<(), Error> {
        for (service, archive) in &bundle.archives {
            // Deletion can be retried after some node archives were already removed.
            let remaining: Vec<Snapshot> = self
                .snapshot_json(service, &bundle.directory(service), &["list"])
                .await?;
            if !remaining.iter().any(|saved| saved.id == archive.id) {
                continue;
            }

            let _: serde_json::Value = self
                .snapshot_json(
                    service,
                    &bundle.directory(service),
                    &["delete", &archive.id],
                )
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn reset_indexer(&self) -> Result<(), Error> {
        self.run_compose(
            ["down", "--remove-orphans"],
            "prepare to rebuild the index",
            "environment_snapshot_restore_failed",
            COMPOSE_DELETE_TIMEOUT,
        )
        .await?;
        // A scanner checkpoint is valid only for the database it populated.
        // Keeping it after dropping Postgres silently skips unchanged accounts.
        for volume in ["postgres-data", "ton-index-workdir", "ton-account-scan"] {
            let mut command = self.docker_command();
            command
                .arg("volume")
                .arg("rm")
                .arg("--force")
                .arg(format!("{}_{volume}", self.project_name));
            self.run_command(
                command,
                "remove derived index data",
                "environment_snapshot_restore_failed",
                COMPOSE_DELETE_TIMEOUT,
            )
            .await?;
        }

        Ok(())
    }

    async fn snapshot_json<T: serde::de::DeserializeOwned>(
        &self,
        service: &str,
        directory: &str,
        args: &[&str],
    ) -> Result<T, Error> {
        let started = Instant::now();
        let action = args.first().copied().unwrap_or("unknown");
        log::info!("operation=snapshot_{action} node={service} phase=started");
        // Inventory and deletion only need the archive volume. Reusing the
        // genesis mount avoids recreating a node volume removed by restore.
        let mounted_service = if action == "create" || action == "restore" {
            service
        } else {
            "localton"
        };
        let mut command = self.offline_command(mounted_service);
        command.arg("snapshot").args(args).args([
            "--state-dir",
            LOCALTON_STATE_DIR,
            "--snapshot-dir",
            directory,
        ]);
        let result = async {
            let output = self
                .command_output(
                    command,
                    "manage snapshots",
                    "environment_snapshot_failed",
                    SNAPSHOT_TIMEOUT,
                )
                .await?;
            serde_json::from_slice(&output.stdout).map_err(|error| Error::Internal {
                code: "environment_snapshot_failed",
                message: format!("{service}: Localton returned invalid snapshot data: {error}"),
            })
        }
        .await;
        log::info!(
            "operation=snapshot_{action} node={service} duration_ms={} outcome={}",
            started.elapsed().as_millis(),
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            }
        );
        result.map_err(|error| Error::Internal {
            code: "environment_snapshot_failed",
            message: format!("{service}: snapshot {action}: {error}"),
        })
    }
}

#[cfg(test)]
mod tests;
