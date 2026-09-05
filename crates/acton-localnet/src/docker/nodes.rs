//! Nodes support for the localnet Docker runtime.

use super::{
    COMPOSE_NODE_COMMAND_TIMEOUT, COMPOSE_NODE_REMOVE_TIMEOUT, COMPOSE_WAIT_TIMEOUT_SECONDS,
    DockerNetwork, LOCALTON_STATE_DIR, compose::render_compose,
};
use crate::{Error, Node};
use std::time::Duration;
use tokio::time::timeout;

impl DockerNetwork {
    /// Adds one validator-engine instance without duplicating the shared HTTP APIs or indexer.
    ///
    /// The compose definition is persisted before Docker starts the service so a localnet service restart
    /// can reconstruct the same topology. A failed start restores the previous definition.
    pub(crate) async fn add_node(&self, existing_nodes: &[Node], node: &Node) -> Result<(), Error> {
        let mut nodes = existing_nodes.to_vec();
        nodes.push(node.clone());
        self.write_compose(&nodes).await?;

        let mut command = self.compose_command();
        command
            .arg("up")
            .arg("-d")
            .arg("--wait")
            .arg("--wait-timeout")
            .arg(COMPOSE_WAIT_TIMEOUT_SECONDS.to_string())
            .arg(&node.id);
        let mut child = match self.spawn_logged(
            &mut command,
            false,
            "join a node to the full TON network with Docker Compose",
        ) {
            Ok(child) => child,
            Err(error) => {
                let _ = self.write_compose(existing_nodes).await;
                return Err(error);
            }
        };

        let result = match timeout(
            Duration::from_secs(u64::from(COMPOSE_WAIT_TIMEOUT_SECONDS)),
            child.wait(),
        )
        .await
        {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(status)) => Err(Error::Internal {
                code: "environment_node_start_failed",
                message: self.startup_failure_message("join the node", status).await,
            }),
            Ok(Err(error)) => Err(Error::Internal {
                code: "environment_node_start_failed",
                message: format!("Failed to wait for the joining node: {error}"),
            }),
            Err(_) => {
                let _ = child.kill().await;
                Err(Error::Internal {
                    code: "environment_node_start_timeout",
                    message: format!(
                        "The joining node did not start within {COMPOSE_WAIT_TIMEOUT_SECONDS} seconds"
                    ),
                })
            }
        };

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
        let mut command = self.compose_command();
        command.args([
            "exec",
            "--no-TTY",
            &node.id,
            "/usr/local/bin/localton",
            "validator",
            "disable",
            "--state-dir",
            LOCALTON_STATE_DIR,
        ]);
        self.run_command(
            command,
            "disable future validator elections",
            "environment_node_validation_leave_failed",
            COMPOSE_NODE_COMMAND_TIMEOUT,
        )
        .await
    }

    pub(crate) async fn enter_validation(&self, node: &Node) -> Result<(), Error> {
        let mut command = self.compose_command();
        command.args([
            "exec",
            "--no-TTY",
            &node.id,
            "/usr/local/bin/localton",
            "validator",
            "enable",
            "--state-dir",
            LOCALTON_STATE_DIR,
        ]);
        self.run_command(
            command,
            "enable future validator elections",
            "environment_node_validation_enter_failed",
            COMPOSE_NODE_COMMAND_TIMEOUT,
        )
        .await
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
        let mut command = self.compose_command();
        command.args(["ps", "--all", "--quiet", &node.id]);
        let output = self
            .command_output(
                command,
                "locate the node container",
                "environment_node_remove_failed",
                COMPOSE_NODE_REMOVE_TIMEOUT,
            )
            .await?;
        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let remaining = existing_nodes
            .iter()
            .filter(|candidate| candidate.id != node.id)
            .cloned()
            .collect::<Vec<_>>();

        // Publish the new topology before deleting runtime state so the removed service cannot
        // return on the next environment restart if the Compose definition cannot be updated.
        self.write_compose(&remaining).await?;

        if !container_id.is_empty() {
            let mut command = self.docker_command();
            command.args(["rm", "--force", &container_id]);
            if let Err(error) = self
                .run_command(
                    command,
                    "remove the node container",
                    "environment_node_remove_failed",
                    COMPOSE_NODE_REMOVE_TIMEOUT,
                )
                .await
            {
                let _ = self.write_compose(existing_nodes).await;
                return Err(error);
            }
        }

        let volume = format!("{}_{}-state", self.project_name, node.id);
        let mut command = self.docker_command();
        command.args(["volume", "rm", "--force", &volume]);
        if let Err(error) = self
            .run_command(
                command,
                "remove the node state volume",
                "environment_node_remove_failed",
                COMPOSE_NODE_REMOVE_TIMEOUT,
            )
            .await
        {
            tracing::warn!(
                operation = "remove_full_ton_node_state",
                node = %node.name,
                target = %volume,
                outcome = "error",
                %error,
                "Node container was removed but its state volume could not be deleted"
            );
        }

        Ok(())
    }

    pub(super) async fn write_compose(&self, nodes: &[Node]) -> Result<(), Error> {
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
