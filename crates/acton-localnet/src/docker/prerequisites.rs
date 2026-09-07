//! Checks Docker before persisting a deployment identity or starting containers.

use std::{io, process::Stdio};

use tokio::{process::Command, time::timeout};

use super::{DOCKER_METADATA_TIMEOUT, DockerTarget};
use crate::Error;

fn failure(code: &'static str, message: &str, details: &str) -> Error {
    Error::Internal {
        code,
        message: format!("{message}\n\n{details}"),
    }
}

/// Keeps a missing executable distinct from an engine or registry failure.
pub(super) fn spawn_error(error: &io::Error, operation: &str) -> Error {
    let details = format!("Failed to {operation}: {error}");
    match error.kind() {
        io::ErrorKind::NotFound => failure(
            "docker_not_found",
            "Docker CLI was not found on PATH\nInstall Docker Desktop or Docker Engine with Compose v2 and make `docker` available to Acton",
            &details,
        ),
        io::ErrorKind::PermissionDenied => failure(
            "docker_permission_denied",
            "Permission to run Docker was denied\nCheck the Docker executable permissions, then retry",
            &details,
        ),
        _ => failure(
            "docker_command_failed",
            "Docker could not be started",
            &details,
        ),
    }
}

/// Recognizes specific prerequisite failures without relabeling remote transport,
/// TLS, context configuration, or arbitrary container errors as a stopped engine.
pub(super) fn runtime_failure(details: &str) -> Option<Error> {
    let text = details.to_ascii_lowercase();
    let local_endpoint =
        text.contains("unix://") || text.contains("docker.sock") || text.contains("//./pipe/");

    let (code, message) = if text.contains("compose")
        && (text.contains("is not a docker command") || text.contains("unknown command"))
    {
        (
            "docker_compose_unavailable",
            "Docker Compose is not available\nInstall or enable Compose v2, then retry",
        )
    } else if (text.contains("permission denied") || text.contains("access is denied"))
        && (local_endpoint || text.contains("connect"))
    {
        (
            "docker_permission_denied",
            "Access to Docker was denied\nGrant your user access to the selected Docker engine, then retry",
        )
    } else if text.contains("x509:")
        || text.contains("tls handshake")
        || text.contains("certificate verify failed")
    {
        (
            "docker_tls_failed",
            "Docker TLS verification failed\nCheck the certificates and TLS settings for the selected Docker context",
        )
    } else if text.contains("context")
        && (text.contains("not found") || text.contains("does not exist"))
    {
        (
            "docker_context_unavailable",
            "The selected Docker context is unavailable\nCheck `docker context ls` and restore or repair the selected context",
        )
    } else if text.contains("no space left on device") {
        (
            "docker_storage_full",
            "Docker has run out of disk space\nFree space in Docker's storage or increase its disk limit, then retry",
        )
    } else if text.contains("port is already allocated") || text.contains("address already in use")
    {
        (
            "docker_port_in_use",
            "A network port is already in use\nStop the conflicting service or choose unused ports for this localnet",
        )
    } else if text.contains("pull access denied")
        || text.contains("unauthorized:")
        || text.contains("authentication required")
    {
        (
            "docker_registry_access_denied",
            "Docker could not access the image registry\nCheck the image name and sign in to its registry with `docker login` if it requires authentication",
        )
    } else if text.contains("manifest unknown") || text.contains("no matching manifest") {
        (
            "docker_image_unavailable",
            "The Docker image is unavailable for this platform\nCheck the image tag and that it supports your machine's architecture",
        )
    } else if text.contains("toomanyrequests") || text.contains("pull rate limit") {
        (
            "docker_registry_rate_limited",
            "The image registry rate limit was reached\nSign in to the registry or wait before retrying the image download",
        )
    } else if text.contains("no such host")
        || text.contains("network is unreachable")
        || text.contains("i/o timeout")
        || text.contains("connection timed out")
        || text.contains("proxyconnect tcp")
    {
        (
            "docker_connection_failed",
            "Docker could not reach the requested endpoint\nCheck the host, network, VPN and proxy settings shown in the diagnostic details, then retry",
        )
    } else if text.trim() == "docker daemon is unavailable"
        || (local_endpoint
            && (text.contains("cannot connect to the docker daemon")
                || text.contains("connection refused")
                || text.contains("no such file or directory")
                || text.contains("the system cannot find the file specified")))
    {
        (
            "docker_engine_unavailable",
            "Docker is not running\nStart Docker Desktop or your Docker Engine service, wait until it is ready, then retry",
        )
    } else if text.contains("cannot connect to the docker daemon")
        || text.contains("error during connect")
        || text.contains("connection refused")
    {
        (
            "docker_connection_failed",
            "The selected Docker endpoint could not be reached\nCheck the Docker context or DOCKER_HOST and verify that the target engine is reachable",
        )
    } else {
        return None;
    };

    Some(failure(code, message, details))
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

/// Probes the selected target on every start, so fixing Docker never requires
/// restarting Studio. These read-only checks precede deployment persistence.
pub(super) async fn check(target: &DockerTarget) -> Result<(), Error> {
    for args in [
        &["info", "--format", "{{.ServerVersion}}"] as &[&str],
        &["compose", "version", "--short"],
    ] {
        let operation = format!("docker {}", args.join(" "));
        let mut command = target.command();
        command.args(args).stdin(Stdio::null()).kill_on_drop(true);
        let output = timeout(DOCKER_METADATA_TIMEOUT, command.output())
            .await
            .map_err(|_| Error::Internal {
                code: "docker_check_failed",
                message: format!(
                    "Docker did not respond within {} seconds\nCheck the selected Docker context and connection, then retry\n\nCheck: {operation}",
                    DOCKER_METADATA_TIMEOUT.as_secs()
                ),
            })?
            .map_err(|error| spawn_error(&error, &operation))?;

        if output.status.success() && !output.stdout.trim_ascii().is_empty() {
            continue;
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        // Keep the original diagnostic in the operation log. Do not print DOCKER_HOST:
        // it can contain credentials, unlike this fixed command description.
        return Err(runtime_failure(stderr.trim()).unwrap_or_else(|| Error::Internal {
            code: "docker_check_failed",
            message: format!("Docker could not complete its availability check\nRun `{operation}` with the selected Docker context to inspect the failure\n\nExit status: {}\n{}", output.status, stderr.trim()),
        }));
    }

    Ok(())
}
