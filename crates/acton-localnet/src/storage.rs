//! Private service discovery, deployment records, and atomic JSON writes.

mod networks;

pub(crate) use networks::{network_directory, prepare_network_directory};

use crate::Error;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// Local discovery data is private because the token authorizes Docker mutations.
/// A descriptor alone does not prove liveness; clients verify `/v1/health`.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDescriptor {
    pub protocol_version: u32,
    pub url: String,
    pub token: String,
    pub pid: u32,
}

/// Returns the descriptor inside an explicitly selected localnet state directory.
/// This namespace is independent from Studio's daemon and environment data.
#[must_use]
pub fn service_descriptor_path(root: &Path) -> PathBuf {
    root.join("service.json")
}

pub(crate) fn lock(root: &Path) -> Result<File, Error> {
    std::fs::create_dir_all(root).map_err(|e| Error::storage(root, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| Error::storage(root, e))?;
    }

    let path = root.join("service.lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| Error::storage(&path, e))?;
    file.try_lock_exclusive().map_err(|_| Error::Conflict {
        code: "service_already_running",
        message: format!("A localnet service already owns {}", root.display()),
    })?;
    Ok(file)
}

/// Tests ownership without creating files or changing permissions. A vanished
/// listener alone is insufficient: graceful Docker cleanup runs after HTTP stops.
pub(crate) fn service_is_locked(root: &Path) -> Result<bool, Error> {
    let path = root.join("service.lock");
    let file = match OpenOptions::new().read(true).write(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(Error::storage(&path, error)),
    };
    match file.try_lock_exclusive() {
        Ok(()) => Ok(false),
        Err(error) if error.kind() == fs2::lock_contended_error().kind() => Ok(true),
        Err(error) => Err(Error::storage(&path, error)),
    }
}

/// Catalog writes hold this lock only while reserving a name/port range or moving
/// a selected directory. Concurrent CLI commands wait briefly for that work;
/// network services still fail immediately when their own directory is owned.
pub(crate) async fn catalog_lock(root: &Path) -> Result<File, Error> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match lock(root) {
            Err(Error::Conflict { .. }) if tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            result => return result,
        }
    }
}

pub(crate) async fn write_json(path: &Path, value: &(impl Serialize + Sync)) -> Result<(), Error> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| Error::storage(path, e))?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    tokio::fs::write(&temp, bytes)
        .await
        .map_err(|e| Error::storage(&temp, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|e| Error::storage(&temp, e))?;
    }
    tokio::fs::rename(&temp, path)
        .await
        .map_err(|e| Error::storage(path, e))
}

pub(crate) async fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| Error::storage(path, e))?;
    serde_json::from_slice(&bytes).map_err(|e| Error::storage(path, e))
}

pub(crate) fn validate_id(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(Error::invalid(
            "Identifiers must contain lowercase letters, digits, or hyphens",
        ));
    }

    Ok(())
}
