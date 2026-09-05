//! Human-readable directory names do not participate in Docker deployment identity.

use super::{read_json, write_json};
use crate::{Error, Network, Operation};
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

/// Names are display text and may contain separators or long Unicode sequences.
/// Bound the prefix so the directory is portable; the ID hash distinguishes names
/// that become identical after sanitizing or deleting and recreating a network.
pub(crate) fn network_directory(root: &Path, network: &Network) -> PathBuf {
    let prefix: String = network
        .name
        .chars()
        .take(40)
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let prefix = prefix.trim_matches('-');
    let prefix = if prefix.is_empty() {
        "localnet"
    } else {
        prefix
    };
    let hash = xxh3_64(network.id.as_bytes());

    root.join("networks").join(format!("{prefix}-{hash:016x}"))
}

/// Moves an existing ID-only directory under the service lock, retaining its
/// runtime descriptor and therefore its Docker project and volumes. Log references
/// are repaired even after a previous rename succeeded but updating JSON failed.
pub(crate) async fn prepare_network_directory(
    root: &Path,
    current: &Path,
    network: &mut Network,
) -> Result<PathBuf, Error> {
    let destination = network_directory(root, network);
    let previous = root.join("networks").join(&network.id);
    if current != destination && current != previous {
        return Err(Error::invalid(
            "Network directory does not match its definition",
        ));
    }

    if current != destination {
        if tokio::fs::try_exists(&destination)
            .await
            .map_err(|e| Error::storage(&destination, e))?
        {
            return Err(Error::invalid(format!(
                "Network directory already exists: {}",
                destination.display()
            )));
        }

        tokio::fs::rename(current, &destination)
            .await
            .map_err(|e| Error::storage(current, e))?;
    }

    // Persisted failures embed the full log path as well as keeping a structured
    // log_path field. Repair both so old operation IDs still lead to real logs.
    let old_prefix = previous.to_string_lossy();
    let new_prefix = destination.to_string_lossy();
    if let Some(error) = &mut network.error {
        *error = error.replace(old_prefix.as_ref(), new_prefix.as_ref());
    }

    if let Some(operation) = &mut network.operation {
        operation.log_path = operation
            .log_path
            .replace(old_prefix.as_ref(), new_prefix.as_ref());
        if let Some(error) = &mut operation.error {
            *error = error.replace(old_prefix.as_ref(), new_prefix.as_ref());
        }
    }

    let operation_dir = root.join("operations");
    let local_operations = destination.join("operations");
    tokio::fs::create_dir_all(&local_operations)
        .await
        .map_err(|e| Error::storage(&local_operations, e))?;
    let mut files = match tokio::fs::read_dir(&operation_dir).await {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(destination),
        Err(error) => return Err(Error::storage(&operation_dir, error)),
    };
    while let Some(file) = files
        .next_entry()
        .await
        .map_err(|e| Error::storage(&operation_dir, e))?
    {
        let path = file.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }

        let mut operation: Operation = read_json(&path).await?;
        if ![Some(previous.as_path()), Some(destination.as_path())]
            .contains(&Path::new(&operation.log_path).parent())
        {
            continue;
        }

        operation.log_path = destination.join("startup.log").display().to_string();
        if let Some(error) = &mut operation.error {
            *error = error.replace(old_prefix.as_ref(), new_prefix.as_ref());
        }
        write_json(&local_operations.join(file.file_name()), &operation).await?;
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| Error::storage(&path, e))?;
    }

    Ok(destination)
}
