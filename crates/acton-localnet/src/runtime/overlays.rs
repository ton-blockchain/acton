//! Serializes overlay changes with node membership, shutdown, and snapshot restore.

use super::{Runtime, operations::Context};
use crate::{Error, Network, OverlayConfig, Status, docker::DockerNetwork};

pub(super) fn validate_update(network: &Network, config: &OverlayConfig) -> Result<(), Error> {
    config.validate(&network.nodes)?;
    if network.status != Status::Running {
        return Err(Error::Conflict {
            code: "network_not_running",
            message: "Start the network before configuring overlays".to_owned(),
        });
    }
    if network.nodes.iter().any(|node| node.stopped) {
        return Err(Error::Conflict {
            code: "node_stopped",
            message: "Start all nodes before replacing the overlay configuration".to_owned(),
        });
    }
    Ok(())
}

impl Context {
    pub(super) async fn configure_overlays(
        &mut self,
        driver: &DockerNetwork,
        config: OverlayConfig,
    ) -> Result<serde_json::Value, Error> {
        let network = self.entry.record.read().await.clone();
        validate_update(&network, &config)?;
        self.phase("configuringOverlays").await?;
        // The adapter validates every node before writing, and rolls back to its
        // captured engine state if any console mutation fails.
        driver.configure_overlays(&network.nodes, &config).await?;
        self.entry.record.write().await.overlay_config = config.clone();
        if let Err(error) = Runtime::save(&self.entry).await {
            self.entry.record.write().await.overlay_config = network.overlay_config.clone();
            let rollback = async {
                driver
                    .configure_overlays(&network.nodes, &network.overlay_config)
                    .await?;
                Runtime::save(&self.entry).await
            }
            .await;
            return match rollback {
                Ok(()) => Err(error),
                Err(rollback) => Err(Error::Internal {
                    code: "overlay_rollback_failed",
                    message: format!(
                        "{error}; restoring the previous overlay configuration also failed: {rollback}; retry the overlay configuration to reconcile all nodes"
                    ),
                }),
            };
        }
        self.entry.record.write().await.error = None;
        serde_json::to_value(config).map_err(|error| Error::invalid(error.to_string()))
    }

    /// The node persists its console settings. Reconcile after startup as well,
    /// including recovery from an interrupted multi-node update.
    pub(super) async fn reconcile_overlays(&mut self, driver: &DockerNetwork) -> Result<(), Error> {
        let network = self.entry.record.read().await.clone();
        if network.overlay_config.is_empty()
            && !self.entry.data_dir.join("overlays-managed").exists()
            && !self.entry.data_dir.join("overlays-recovery").exists()
        {
            return Ok(());
        }
        self.phase("configuringOverlays").await?;
        network.overlay_config.validate(&network.nodes)?;
        driver
            .configure_overlays(&network.nodes, &network.overlay_config)
            .await?;
        let recovery = self.entry.data_dir.join("overlays-recovery");
        match tokio::fs::remove_file(&recovery).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::storage(&recovery, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CreateNetwork, Node, PrivateOverlay, catalog, runtime::Action};

    #[tokio::test]
    async fn invalid_overlay_updates_do_not_publish_an_operation_or_materialize_docker()
    -> anyhow::Result<()> {
        let directory = tempfile::tempdir()?;
        let location = catalog::create(
            directory.path(),
            CreateNetwork {
                name: "overlay-admission".into(),
                ..Default::default()
            },
        )
        .await?;
        let runtime = Runtime::open(&location.path).await?;
        let config = OverlayConfig {
            overlays: vec![PrivateOverlay {
                name: "private".into(),
                nodes: vec!["genesis".into(), "node-1".into()],
            }],
        };
        // Invalid membership is rejected before checking lifecycle state.
        assert!(
            runtime
                .submit(Action::ConfigureOverlays(config.clone()))
                .await
                .unwrap_err()
                .to_string()
                .contains("Unknown node")
        );
        {
            let mut network = runtime.inner.entry.record.write().await;
            network.nodes.push(Node {
                id: "node-1".into(),
                name: "replica".into(),
                validator: false,
                port_base: 20010,
                stopped: false,
            });
        }
        assert_eq!(
            runtime
                .submit(Action::ConfigureOverlays(config.clone()))
                .await
                .unwrap_err()
                .code(),
            "network_not_running"
        );
        {
            let mut network = runtime.inner.entry.record.write().await;
            network.status = Status::Running;
            network.nodes[0].stopped = true;
        }
        assert_eq!(
            runtime
                .submit(Action::ConfigureOverlays(config))
                .await
                .unwrap_err()
                .code(),
            "node_stopped"
        );
        assert!(runtime.get().await.operation.is_none());
        assert!(!location.path.join("runtime.json").exists());
        assert!(!location.path.join("compose.yaml").exists());
        Ok(())
    }
}
