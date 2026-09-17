//! Checks Docker before persisting a deployment identity or starting containers.

use super::DockerTarget;
use crate::Error;

/// Classifies daemon errors while retaining the Engine's diagnostic message.
/// Request bodies (which can contain imported account state) are never formatted.
pub(super) fn api_error(error: bollard::errors::Error) -> Error {
    let details = match error {
        bollard::errors::Error::DockerResponseServerError { message, .. } => message,
        bollard::errors::Error::JsonDataError { .. } => "Docker returned an invalid API response".to_owned(),
        bollard::errors::Error::UnsupportedURISchemeError { .. } => "Unsupported Docker transport; use a Unix socket, Windows named pipe, or TCP/TLS endpoint".to_owned(),
        other => {
            let mut details = other.to_string();
            let mut source = std::error::Error::source(&other);
            while let Some(cause) = source {
                details.push_str(": ");
                details.push_str(&cause.to_string());
                source = cause.source();
            }
            details
        },
    };
    runtime_failure(&details).unwrap_or_else(|| {
        failure(
            "docker_api_failed",
            "Docker Engine API request failed",
            &details,
        )
    })
}

fn failure(code: &'static str, message: &str, details: &str) -> Error {
    Error::Internal {
        code,
        message: format!("{message}\n\n{details}"),
    }
}

/// Recognizes specific prerequisite failures without relabeling remote transport,
/// TLS, context configuration, or arbitrary container errors as a stopped engine.
pub(super) fn runtime_failure(details: &str) -> Option<Error> {
    let text = details.to_ascii_lowercase();
    let local_endpoint =
        text.contains("unix://") || text.contains("docker.sock") || text.contains("//./pipe/");

    let (code, message) = if (text.contains("permission denied")
        || text.contains("access is denied"))
        && (local_endpoint || text.contains("connect"))
    {
        (
            "docker_permission_denied",
            "Access to Docker was denied\nGrant your user access to the selected Docker engine, then retry",
        )
    } else if text.contains("x509:")
        || text.contains("tls handshake")
        || text.contains("certificate verify failed")
        || text.contains("invalid peer certificate")
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
            "Docker could not access the image registry\nCheck the image name and configure inline registry credentials in Docker config.json if it requires authentication",
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
        || text.starts_with("socket not found:")
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

/// Probes the selected engine before persisting a new deployment identity.
pub(super) async fn check(target: &DockerTarget) -> Result<(), Error> {
    let client = target.connect().await?;
    tokio::time::timeout(super::DOCKER_METADATA_TIMEOUT, client.ping())
        .await
        .map_err(|_| {
            failure(
                "docker_check_failed",
                "Docker did not respond within 10 seconds",
                "Check the selected Docker engine, then retry",
            )
        })?
        .map_err(api_error)?;
    Ok(())
}
