//! Descriptor support for the localnet Docker runtime.

use super::{DOCKER_METADATA_TIMEOUT, DockerTarget, RUNTIME_DESCRIPTOR_VERSION, RuntimeDescriptor};
use crate::Error;
use std::{path::Path, process::Stdio};
use tokio::{process::Command, time::timeout};
use uuid::Uuid;
use xxhash_rust::xxh3::xxh3_64;

pub(super) async fn load_runtime_descriptor(
    path: &Path,
) -> Result<Option<RuntimeDescriptor>, Error> {
    let contents = match tokio::fs::read(path).await {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(Error::Internal {
                code: "environment_storage_failed",
                message: format!(
                    "Failed to read the full TON network runtime descriptor at {}: {error}",
                    path.display()
                ),
            });
        }
    };

    let runtime: RuntimeDescriptor =
        serde_json::from_slice(&contents).map_err(|error| Error::Internal {
            code: "environment_storage_failed",
            message: format!(
                "Failed to parse the full TON network runtime descriptor at {}: {error}",
                path.display()
            ),
        })?;
    if runtime.version != RUNTIME_DESCRIPTOR_VERSION {
        return Err(Error::Internal {
            code: "environment_storage_failed",
            message: format!(
                "The full TON network runtime descriptor at {} uses unsupported version {}",
                path.display(),
                runtime.version
            ),
        });
    }

    Ok(Some(runtime))
}

pub(super) async fn write_runtime_descriptor(
    path: &Path,
    runtime: &RuntimeDescriptor,
) -> Result<(), Error> {
    let mut contents = serde_json::to_vec_pretty(runtime).map_err(|error| Error::Internal {
        code: "environment_storage_failed",
        message: format!("Failed to serialize the full TON network runtime: {error}"),
    })?;
    contents.push(b'\n');
    let temp_path = path.with_extension(format!("json.{}.tmp", std::process::id()));
    tokio::fs::write(&temp_path, contents)
        .await
        .map_err(|error| Error::Internal {
            code: "environment_storage_failed",
            message: format!(
                "Failed to write the full TON network runtime descriptor at {}: {error}",
                temp_path.display()
            ),
        })?;
    if let Err(error) = tokio::fs::rename(&temp_path, path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(Error::Internal {
            code: "environment_storage_failed",
            message: format!(
                "Failed to publish the full TON network runtime descriptor at {}: {error}",
                path.display()
            ),
        });
    }

    Ok(())
}

pub(super) async fn resolve_docker_target() -> Result<DockerTarget, Error> {
    match std::env::var("DOCKER_CONTEXT") {
        Ok(context) if !context.is_empty() => return Ok(DockerTarget::Context(context)),
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(Error::InvalidRequest {
                code: "environment_docker_context_invalid",
                message: "DOCKER_CONTEXT must contain valid UTF-8".to_owned(),
            });
        }
    }

    match std::env::var("DOCKER_HOST") {
        Ok(host) if !host.is_empty() => return Ok(DockerTarget::Host(host)),
        Ok(_) | Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(Error::InvalidRequest {
                code: "environment_docker_host_invalid",
                message: "DOCKER_HOST must contain valid UTF-8".to_owned(),
            });
        }
    }

    let mut command = Command::new("docker");
    command.args(["context", "show"]);
    docker_text(command).await.map(DockerTarget::Context)
}

pub(super) async fn docker_text(mut command: Command) -> Result<String, Error> {
    command.stdin(Stdio::null()).kill_on_drop(true);
    let output = timeout(DOCKER_METADATA_TIMEOUT, command.output())
        .await
        .map_err(|_| Error::Internal {
            code: "environment_start_failed",
            message: "Timed out while inspecting the active Docker context".to_owned(),
        })?
        .map_err(|error| Error::Internal {
            code: "environment_start_failed",
            message: format!("Failed to inspect the active Docker context: {error}"),
        })?;
    if !output.status.success() {
        let details = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Internal {
            code: "environment_start_failed",
            message: format!("Docker context inspection failed: {}", details.trim()),
        });
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if value.is_empty() {
        return Err(Error::Internal {
            code: "environment_start_failed",
            message: "Docker context inspection returned an empty value".to_owned(),
        });
    }

    Ok(value)
}

pub(super) fn compose_project_name(
    workspace_root: &Path,
    environment_id: &str,
    deployment_id: Uuid,
) -> String {
    let workspace_hash = xxh3_64(workspace_root.as_os_str().as_encoded_bytes());
    format!(
        "acton-localnet-{workspace_hash:016x}-{environment_id}-{}",
        deployment_id.as_simple()
    )
}

pub(super) fn validate_image_reference(image: &str) -> Result<(), Error> {
    if image.is_empty()
        || !image
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/:@-".contains(&byte))
    {
        return Err(Error::InvalidRequest {
            code: "environment_image_invalid",
            message: "The full TON network image is not a valid container image reference"
                .to_owned(),
        });
    }

    Ok(())
}
