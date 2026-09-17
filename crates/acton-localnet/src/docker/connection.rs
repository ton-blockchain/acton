//! Resolves Docker's on-disk context configuration without invoking its CLI.
//! The selected target is persisted by the runtime; polling never changes it.

use super::{DOCKER_METADATA_TIMEOUT, DockerTarget};
use crate::Error;
use bollard::{API_DEFAULT_VERSION, Docker};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub(super) fn config_directory() -> Result<PathBuf, Error> {
    if let Some(path) = std::env::var_os("DOCKER_CONFIG") {
        return Ok(path.into());
    }
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(".docker"))
        .ok_or_else(|| Error::invalid("Cannot locate Docker configuration; set DOCKER_CONFIG"))
}

pub(super) async fn configuration() -> Result<Value, Error> {
    let path = config_directory()?.join("config.json");
    match tokio::fs::read(&path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| Error::storage(&path, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
        Err(e) => Err(Error::storage(&path, e)),
    }
}

impl DockerTarget {
    /// Opens the pinned engine and negotiates its API before sending mutations.
    /// Context TLS material is read locally; secrets never enter command arguments.
    pub(super) async fn connect(&self) -> Result<Docker, Error> {
        let mut tls = None;
        let host = match self {
            Self::Host(host) => host.clone(),
            Self::Context(name) if name == "default" => {
                if cfg!(windows) {
                    "npipe:////./pipe/docker_engine".to_owned()
                } else {
                    "unix:///var/run/docker.sock".to_owned()
                }
            }
            Self::Context(name) => {
                let hash = format!("{:x}", Sha256::digest(name.as_bytes()));
                let directory = config_directory()?;
                let path = directory
                    .join("contexts/meta")
                    .join(&hash)
                    .join("meta.json");
                let bytes = tokio::fs::read(&path).await.map_err(|e| Error::Internal {
                    code: "docker_context_unavailable",
                    message: format!("Cannot read Docker context at {}: {e}", path.display()),
                })?;
                let metadata: Value =
                    serde_json::from_slice(&bytes).map_err(|e| Error::storage(&path, e))?;
                let endpoint = &metadata["Endpoints"]["docker"];
                if endpoint["SkipTLSVerify"].as_bool() == Some(true) {
                    return Err(Error::invalid(
                        "Docker contexts with SkipTLSVerify are unsupported; configure a trusted CA",
                    ));
                }
                let host = endpoint["Host"]
                    .as_str()
                    .ok_or_else(|| Error::invalid("Docker context has no engine endpoint"))?;
                let certs = directory.join("contexts/tls").join(hash).join("docker");
                if certs.join("ca.pem").exists() {
                    tls = Some(certs);
                }
                host.to_owned()
            }
        };

        if host.starts_with("ssh://") {
            return Err(Error::invalid(
                "SSH Docker contexts are unsupported; select a Unix socket, Windows named pipe, or TCP/TLS endpoint",
            ));
        }
        if (host.starts_with("tcp://")
            || host.starts_with("http://")
            || host.starts_with("https://"))
            && host.contains('@')
        {
            return Err(Error::invalid(
                "Docker endpoints containing credentials are unsupported; use TLS certificates",
            ));
        }

        // Transport messages can contain endpoint credentials. Never include the
        // endpoint verbatim in progress logs or errors.
        let connect = match tls {
            Some(certs) => Docker::connect_with_ssl(
                &host,
                &certs.join("key.pem"),
                &certs.join("cert.pem"),
                &certs.join("ca.pem"),
                120,
                API_DEFAULT_VERSION,
            ),
            None => Docker::connect_with_host(&host),
        };
        let client = connect.map_err(super::prerequisites::api_error)?;
        tokio::time::timeout(DOCKER_METADATA_TIMEOUT, client.negotiate_version())
            .await
            .map_err(|_| Error::Internal {
                code: "docker_check_failed",
                message: "Docker did not respond within 10 seconds\nCheck the selected Docker context and connection, then retry"
                    .into(),
            })?
            .map_err(super::prerequisites::api_error)
    }
}
