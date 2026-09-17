//! Nodes support for the localnet Docker runtime.

#[cfg(test)]
mod tests;

use super::{
    DockerNetwork, LOCALTON_STATE_DIR, NODE_COMMAND_TIMEOUT, NODE_REMOVE_TIMEOUT,
    SERVICE_START_TIMEOUT_SECONDS, compose::render_compose,
};
use crate::{Error, Node};
use std::time::Duration;

impl DockerNetwork {
    /// Starts or gracefully stops one service without removing its container or state volume.
    /// Explicit service selection leaves the network owner and peer nodes alone.
    pub(crate) async fn node_running(
        &self,
        nodes: &[Node],
        id: &str,
        running: bool,
    ) -> Result<(), Error> {
        self.write_compose(nodes).await?;
        self.operation(
            if running { "start_node" } else { "stop_node" },
            if running {
                "environment_node_start_failed"
            } else {
                "environment_node_stop_failed"
            },
            Duration::from_secs(u64::from(SERVICE_START_TIMEOUT_SECONDS) + 10),
            async {
                if running {
                    self.start_services(Some(&[id.to_owned()]), false, true)
                        .await
                } else {
                    self.stop_service(id).await
                }
            },
        )
        .await
    }

    /// Adds one validator-engine instance without duplicating the shared HTTP APIs or indexer.
    ///
    /// The compose definition is persisted before Docker starts the service so a localnet service restart
    /// can reconstruct the same topology. A failed start restores the previous definition.
    pub(crate) async fn add_node(&self, existing_nodes: &[Node], node: &Node) -> Result<(), Error> {
        let mut nodes = existing_nodes.to_vec();
        nodes.push(node.clone());
        self.write_compose(&nodes).await?;

        let result = self
            .operation(
                "join_node",
                "environment_node_start_failed",
                Duration::from_secs(u64::from(SERVICE_START_TIMEOUT_SECONDS)),
                self.start_services(Some(std::slice::from_ref(&node.id)), true, true),
            )
            .await;

        if result.is_err() {
            let _ = self.write_compose(existing_nodes).await;
        }
        result
    }

    /// Disables future election participation inside a joined validator's persistent state.
    ///
    /// Localton keeps the validator engine online until TON replaces the elected set. The setting
    /// survives container restarts and observability reports the intermediate `leaving` state.
    pub(crate) async fn leave_validation(&self, node: &Node) -> Result<(), Error> {
        self.exec(
            &node.id,
            &[
                "/usr/local/bin/localton",
                "validator",
                "disable",
                "--state-dir",
                LOCALTON_STATE_DIR,
            ],
            None,
            NODE_COMMAND_TIMEOUT,
        )
        .await
        .map(|_| ())
    }

    /// Restores election participation in the joined validator's durable state.
    pub(crate) async fn enter_validation(&self, node: &Node) -> Result<(), Error> {
        self.exec(
            &node.id,
            &[
                "/usr/local/bin/localton",
                "validator",
                "enable",
                "--state-dir",
                LOCALTON_STATE_DIR,
            ],
            None,
            NODE_COMMAND_TIMEOUT,
        )
        .await
        .map(|_| ())
    }

    /// Stops one joined service, removes its private state volume, and persists the new topology.
    ///
    /// The bootstrap service is not represented by `Node`, so callers cannot remove the
    /// network owner through this operation.
    pub(crate) async fn remove_node(
        &self,
        existing_nodes: &[Node],
        node: &Node,
    ) -> Result<(), Error> {
        let remaining = existing_nodes
            .iter()
            .filter(|candidate| candidate.id != node.id)
            .cloned()
            .collect::<Vec<_>>();

        // Publish the new topology before deleting runtime state so the removed service cannot
        // return on the next environment restart if the Compose definition cannot be updated.
        self.write_compose(&remaining).await?;

        if let Err(error) = self
            .operation(
                "remove_node",
                "environment_node_remove_failed",
                NODE_REMOVE_TIMEOUT,
                self.remove_service(&node.id),
            )
            .await
        {
            let _ = self.write_compose(existing_nodes).await;
            return Err(error);
        }
        let volume = format!("{}-state", node.id);
        if let Err(error) = self
            .operation(
                "remove_node_volume",
                "environment_node_remove_failed",
                NODE_REMOVE_TIMEOUT,
                self.remove_volume(&volume),
            )
            .await
        {
            tracing::warn!(operation = "remove_node_volume", node = %node.name, target = %volume,
                outcome = "error", %error, "Node container was removed but its state volume could not be deleted");
        }

        Ok(())
    }

    pub(crate) async fn write_compose(&self, nodes: &[Node]) -> Result<(), Error> {
        let temp_path = self
            .compose_file
            .with_extension(format!("yaml.{}.tmp", std::process::id()));
        tokio::fs::write(
            &temp_path,
            render_compose(&self.image, &self.compose_config, nodes),
        )
        .await
        .map_err(|error| Error::Internal {
            code: "environment_storage_failed",
            message: format!(
                "Failed to write the full TON network definition at {}: {error}",
                temp_path.display()
            ),
        })?;
        if let Err(error) = tokio::fs::rename(&temp_path, &self.compose_file).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(Error::Internal {
                code: "environment_storage_failed",
                message: format!(
                    "Failed to publish the full TON network definition at {}: {error}",
                    self.compose_file.display()
                ),
            });
        }

        Ok(())
    }
}
