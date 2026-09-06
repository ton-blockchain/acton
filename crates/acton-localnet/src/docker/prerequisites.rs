//! Bounded, read-only Docker checks shared by Studio and the localnet CLI.

use std::{io, process::Stdio};
use tokio::{process::Command, time::timeout};

use super::{DOCKER_METADATA_TIMEOUT, DockerTarget};
use crate::Error;

const INSTALL_DOCKER: &str = "Install Docker Desktop on macOS/Windows, or Docker Engine with the Compose plugin on Linux, then restart Acton from a terminal where `docker info` works";
const START_DOCKER: &str = "Start Docker Desktop or your Docker Engine service and wait until `docker info` succeeds, then retry this operation";
const DOCKER_ACCESS: &str = "Grant your user access to Docker, verify `docker info` from the terminal that runs Acton, then retry this operation";
const INSTALL_COMPOSE: &str = "Install or enable Docker Compose v2, verify `docker compose version`, then retry this operation";

pub(super) fn failure(code: &'static str, title: &str, next_step: &str, details: &str) -> Error {
    Error::Internal {
        code,
        message: format!("{title}\nNext step: {next_step}\n\n{details}"),
    }
}

/// Spawn errors are distinct from daemon errors: starting Docker cannot fix a missing CLI.
pub(super) fn spawn_error(error: &io::Error, operation: &str) -> Error {
    let (code, title, next_step) = match error.kind() {
        io::ErrorKind::NotFound => (
            "docker_not_found",
            "Docker CLI was not found on PATH",
            INSTALL_DOCKER,
        ),
        io::ErrorKind::PermissionDenied => (
            "docker_permission_denied",
            "Permission to run Docker was denied",
            "Allow your user to execute the Docker CLI, then retry this operation",
        ),
        _ => (
            "docker_command_failed",
            "Docker CLI could not be started",
            "Verify `docker --version` from the terminal that runs Acton, then retry this operation",
        ),
    };

    failure(
        code,
        title,
        next_step,
        &format!("Failed to {operation}: {error}"),
    )
}

/// Recognizes infrastructure failures during later operations without relabeling image,
/// container, or registry errors as daemon failures.
pub(super) fn runtime_failure(details: &str) -> Option<Error> {
    let lower = details.to_ascii_lowercase();
    if lower.contains("compose")
        && (lower.contains("is not a docker command") || lower.contains("unknown command"))
    {
        return Some(failure(
            "docker_compose_unavailable",
            "Docker Compose is not available",
            INSTALL_COMPOSE,
            details,
        ));
    }

    if (lower.contains("permission denied") || lower.contains("access is denied"))
        && (lower.contains("connect") || lower.contains("docker.sock") || lower.contains("pipe"))
    {
        return Some(failure(
            "docker_permission_denied",
            "Access to Docker was denied",
            DOCKER_ACCESS,
            details,
        ));
    }

    if lower.contains("cannot connect to the docker daemon")
        || lower.contains("is the docker daemon running")
        || lower.contains("docker daemon is unavailable")
        || lower.contains("error during connect")
    {
        return Some(failure(
            "docker_engine_unavailable",
            "Docker Engine is not reachable",
            START_DOCKER,
            details,
        ));
    }

    None
}

/// Checks the pinned deployment target, not whatever context became active since its creation.
/// Nothing is persisted or pulled until both the Engine and Compose respond successfully.
pub(super) async fn check(target: &DockerTarget) -> Result<(), Error> {
    for (args, code, title, next_step) in [
        (
            ["info", "--format", "{{.ServerVersion}}"],
            "docker_engine_unavailable",
            "Docker Engine is not reachable",
            START_DOCKER,
        ),
        (
            ["compose", "version", "--short"],
            "docker_compose_unavailable",
            "Docker Compose is not available",
            INSTALL_COMPOSE,
        ),
    ] {
        let mut command = target.command();
        command.args(args).stdin(Stdio::null()).kill_on_drop(true);
        let output = timeout(DOCKER_METADATA_TIMEOUT, command.output())
            .await
            .map_err(|_| {
                failure(
                    code,
                    title,
                    next_step,
                    "Docker did not respond within 10 seconds",
                )
            })?
            .map_err(|error| spawn_error(&error, "check Docker prerequisites"))?;

        if output.status.success() && !output.stdout.trim_ascii().is_empty() {
            continue;
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let target_label = match target {
            DockerTarget::Context(context) => format!("Saved Docker context: {context}"),
            // A remote endpoint can contain credentials; do not repeat it in UI errors or logs.
            DockerTarget::Host(_) => "Saved Docker host from DOCKER_HOST".to_owned(),
        };
        let details = format!(
            "{target_label}\nCheck that this deployment's Docker target is available\nDocker check exited with {}\n{}",
            output.status,
            stderr.trim()
        );
        return Err(
            runtime_failure(&details).unwrap_or_else(|| failure(code, title, next_step, &details))
        );
    }

    Ok(())
}

impl DockerTarget {
    pub(super) fn command(&self) -> Command {
        let mut command = Command::new("docker");
        match self {
            Self::Context(context) => command.arg("--context").arg(context),
            Self::Host(host) => command.arg("--host").arg(host),
        };
        command
    }
}
