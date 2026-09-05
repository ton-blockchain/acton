//! Docker deployment identity shared by the compose and process modules.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Network, NetworkConfig};

const COMPOSE_TEMPLATE: &str = include_str!("../../assets/localton.compose.yaml");
const DEFAULT_LOCALTON_IMAGE: &str =
    "ghcr.io/ton-blockchain/localton:sha-fb807b4286149081379b337cc1f50165972384d7";
const COMPOSE_WAIT_TIMEOUT_SECONDS: u16 = 600;
const DOCKER_CONFIG_DIRECTORY: &str = "docker-pull-config";
const RUNTIME_DESCRIPTOR_FILE: &str = "runtime.json";
const RUNTIME_DESCRIPTOR_VERSION: u16 = 2;
const STARTUP_LOG_FILE: &str = "startup.log";
const STARTUP_ERROR_LINES: usize = 12;
const FAILED_CONTAINER_LOG_LINES: usize = 80;
const DOCKER_METADATA_TIMEOUT: Duration = Duration::from_secs(10);
const DOCKER_DIAGNOSTICS_TIMEOUT: Duration = Duration::from_secs(15);
const COMPOSE_STOP_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const COMPOSE_DELETE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const COMPOSE_NODE_REMOVE_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const COMPOSE_NODE_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const LOCALTON_STATE_DIR: &str = "/var/lib/localton";
const LOCALTON_SNAPSHOT_DIR: &str = "/var/lib/localton-snapshots";

/// Docker deployment identity and process commands, independent of localnet service.
#[derive(Clone)]
pub(crate) struct DockerNetwork {
    compose_file: PathBuf,
    compose_config: NetworkConfig,
    docker_target: DockerTarget,
    isolated_docker_config_dir: Option<PathBuf>,
    image: String,
    project_name: String,
    startup_log_file: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
enum DockerTarget {
    Context(String),
    Host(String),
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeDescriptor {
    version: u16,
    image: String,
    docker_target: DockerTarget,
    project_name: String,
}

mod admin;
mod compose;
mod descriptor;
mod diagnostics;
mod nodes;
mod process;
mod progress;
mod snapshots;

use compose::render_compose;
use descriptor::{
    compose_project_name, load_runtime_descriptor, resolve_docker_target, validate_image_reference,
    write_runtime_descriptor,
};

impl DockerNetwork {
    /// Loads only an existing deployment for read-only observation. In particular,
    /// status must not create a descriptor, render Compose, or change Docker context.
    pub(crate) async fn load(data_dir: &Path, network: &Network) -> Result<Option<Self>, Error> {
        let Some(runtime) =
            load_runtime_descriptor(&data_dir.join(RUNTIME_DESCRIPTOR_FILE)).await?
        else {
            return Ok(None);
        };

        Ok(Some(Self {
            compose_file: data_dir.join("compose.yaml"),
            compose_config: network.config.clone(),
            docker_target: runtime.docker_target,
            isolated_docker_config_dir: None,
            image: runtime.image,
            project_name: runtime.project_name,
            startup_log_file: data_dir.join(STARTUP_LOG_FILE),
        }))
    }

    pub(crate) fn state_location(&self) -> crate::NetworkState {
        crate::NetworkState {
            directory: LOCALTON_STATE_DIR.to_owned(),
            volume: format!("{}_localton-state", self.project_name),
        }
    }

    /// Pins Docker identity on first use and renders the persisted network definition.
    /// Reading the existing descriptor keeps restarts attached to the same volumes.
    pub(crate) async fn materialize(
        data_dir: &Path,
        workspace_root: &Path,
        network: &Network,
    ) -> Result<Self, Error> {
        let runtime_file = data_dir.join(RUNTIME_DESCRIPTOR_FILE);
        let runtime = match load_runtime_descriptor(&runtime_file).await? {
            Some(runtime) => runtime,
            None => {
                let image = std::env::var("ACTON_LOCALNET_IMAGE")
                    .unwrap_or_else(|_| DEFAULT_LOCALTON_IMAGE.to_owned());
                validate_image_reference(&image)?;
                let runtime = RuntimeDescriptor {
                    version: RUNTIME_DESCRIPTOR_VERSION,
                    image,
                    docker_target: resolve_docker_target().await?,
                    project_name: compose_project_name(workspace_root, &network.id, Uuid::new_v4()),
                };
                write_runtime_descriptor(&runtime_file, &runtime).await?;
                runtime
            }
        };
        validate_image_reference(&runtime.image)?;
        let RuntimeDescriptor {
            image,
            docker_target,
            project_name,
            ..
        } = runtime;
        let compose_file = data_dir.join("compose.yaml");
        let isolated_docker_config_dir = if image == DEFAULT_LOCALTON_IMAGE {
            let path = data_dir.join(DOCKER_CONFIG_DIRECTORY);
            tokio::fs::create_dir_all(&path)
                .await
                .map_err(|error| Error::Internal {
                    code: "environment_storage_failed",
                    message: format!(
                        "Failed to create isolated Docker pull configuration at {}: {error}",
                        path.display()
                    ),
                })?;
            Some(path)
        } else {
            None
        };

        let compose = render_compose(&image, &network.config, &network.nodes);
        tokio::fs::write(&compose_file, compose)
            .await
            .map_err(|error| Error::Internal {
                code: "environment_storage_failed",
                message: format!(
                    "Failed to write the full TON network definition at {}: {error}",
                    compose_file.display()
                ),
            })?;

        Ok(Self {
            compose_file,
            compose_config: network.config.clone(),
            docker_target,
            isolated_docker_config_dir,
            image,
            project_name,
            startup_log_file: data_dir.join(STARTUP_LOG_FILE),
        })
    }
}
