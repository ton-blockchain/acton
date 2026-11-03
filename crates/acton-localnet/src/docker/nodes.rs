//! Nodes support for the localnet Docker runtime.

use super::{
    COMPOSE_NODE_COMMAND_TIMEOUT, COMPOSE_NODE_REMOVE_TIMEOUT, COMPOSE_WAIT_TIMEOUT_SECONDS,
    DockerNetwork, LOCALTON_STATE_DIR, compose::render_compose,
};
use crate::{Error, Node};
use std::time::{Duration, Instant};
use tokio::time::timeout;

impl DockerNetwork {
    /// Seeds only the new node volume from the stopped genesis volume.
    /// The runtime owns stopping and restarting the network. The helper takes
    /// filesystem locks too, so an unexpected live source cannot be copied.
    pub(crate) async fn copy_node_state(&self, node: &Node) -> Result<(), Error> {
        let started = Instant::now();
        let container = format!("{}-copy-{}", self.project_name, uuid::Uuid::new_v4());
        let mut command = self.docker_command();
        command
            .args([
                "run",
                "--rm",
                "--name",
                &container,
                "--network",
                "none",
                "--volume",
            ])
            .arg(format!(
                "{}_localton-state:/var/lib/localton-source:ro",
                self.project_name
            ))
            .arg("--volume")
            .arg(format!(
                "{}_{}-state:{LOCALTON_STATE_DIR}",
                self.project_name, node.id
            ))
            .args(["--entrypoint", "/usr/local/bin/localton", &self.image])
            .args([
                "node",
                "copy-state",
                "--from-state-dir",
                "/var/lib/localton-source",
                "--state-dir",
                LOCALTON_STATE_DIR,
            ]);

        tracing::info!(
            operation = "copy_node_state", node = %node.name, target = %node.id,
            duration_ms = 0, outcome = "pending", "Copying genesis blockchain data"
        );
        let mut child = self.spawn_logged(&mut command, false, "copy genesis blockchain data")?;
        let result = match timeout(Duration::from_secs(3600), child.wait()).await {
            Ok(Ok(status)) if status.success() => Ok(()),
            Ok(Ok(status)) => Err(format!("Blockchain data copy exited with {status}")),
            Ok(Err(error)) => Err(format!("Failed to wait for blockchain data copy: {error}")),
            Err(_) => Err("Blockchain data copy did not finish within one hour".to_owned()),
        };

        // Killing the Docker client does not stop its container. Release the
        // source lock before the runtime attempts to restart the network.
        let result = if let Err(error) = result {
            let _ = child.kill().await;
            let mut cleanup = self.docker_command();
            cleanup.args(["rm", "--force", &container]);
            let cleanup = self
                .run_command(
                    cleanup,
                    "remove the blockchain copy helper",
                    "node_copy_cleanup_failed",
                    COMPOSE_NODE_REMOVE_TIMEOUT,
                )
                .await;
            let message = match cleanup {
                Err(cleanup) if !cleanup.to_string().contains("No such container") => {
                    format!("{error}; helper cleanup also failed: {cleanup}")
                }
                _ => error,
            };
            Err(Error::Internal {
                code: "node_copy_failed",
                message: format!("{message}\nFull log: {}", self.startup_log_file.display()),
            })
        } else {
            Ok(())
        };

        tracing::info!(
            operation = "copy_node_state", node = %node.name, target = %node.id,
            duration_ms = started.elapsed().as_millis() as u64,
            outcome = if result.is_ok() { "success" } else { "failure" },
            "Blockchain data copy finished"
        );
        result
    }

    /// Starts or gracefully stops one service without removing its container or state volume.
    /// Explicit service selection and --no-deps leave the network owner and peer nodes alone.
    pub(crate) async fn node_running(
        &self,
        nodes: &[Node],
        id: &str,
        running: bool,
    ) -> Result<(), Error> {
        self.write_compose(nodes).await?;
        let mut command = self.compose_command();
        if running {
            command
                .args(["up", "-d", "--no-deps", "--wait", "--wait-timeout"])
                .arg(COMPOSE_WAIT_TIMEOUT_SECONDS.to_string())
                .arg(id);
        } else {
            command.args(["stop", "--timeout", "30", id]);
        }
        self.run_command(
            command,
            if running {
                "start the node"
            } else {
                "stop the node"
            },
            if running {
                "environment_node_start_failed"
            } else {
                "environment_node_stop_failed"
            },
            if running {
                Duration::from_secs(u64::from(COMPOSE_WAIT_TIMEOUT_SECONDS) + 10)
            } else {
                COMPOSE_NODE_REMOVE_TIMEOUT
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
        if let Err(error) = self.remove_node_state(node).await {
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

    /// Removes a detached node volume after deletion or an abandoned cold copy.
    /// Docker refuses removal if a container still uses the volume.
    pub(crate) async fn remove_node_state(&self, node: &Node) -> Result<(), Error> {
        let volume = format!("{}_{}-state", self.project_name, node.id);
        let mut command = self.docker_command();
        command.args(["volume", "rm", "--force", &volume]);
        self.run_command(
            command,
            "remove the node state volume",
            "environment_node_remove_failed",
            COMPOSE_NODE_REMOVE_TIMEOUT,
        )
        .await
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

#[cfg(test)]
mod tests;
