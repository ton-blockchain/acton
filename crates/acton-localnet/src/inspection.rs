//! Read-only access remains available after a network's service has exited.

use crate::{Error, Network, Operation, Status, docker::DockerNetwork, storage};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

/// Reads the same bounded log tail used by HTTP clients. Missing logs mean the
/// deployment has not produced output; no service or Docker process is started.
pub async fn logs(root: &Path, lines: usize) -> Result<String, Error> {
    let path = root.join("startup.log");
    let mut file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(error) => return Err(Error::storage(&path, error)),
    };

    let length = file
        .metadata()
        .await
        .map_err(|e| Error::storage(&path, e))?
        .len();
    file.seek(std::io::SeekFrom::Start(length.saturating_sub(256 * 1024)))
        .await
        .map_err(|e| Error::storage(&path, e))?;
    let mut bytes = Vec::new();
    file.take(256 * 1024)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| Error::storage(&path, e))?;

    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .rev()
        .take(lines.min(2000))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Operation IDs are scoped to one network directory. Reading a durable record
/// never replays an interrupted mutation or acquires the service's ownership lock.
pub async fn operation(root: &Path, id: &str) -> Result<Operation, Error> {
    storage::validate_id(id)?;
    storage::read_json(&root.join("operations").join(format!("{id}.json"))).await
}

/// Observes Docker through the pinned deployment descriptor without rewriting
/// Compose or opening a runtime. An unavailable daemon cannot confirm old status.
pub async fn status(root: &Path) -> Result<Network, Error> {
    let mut network: Network = storage::read_json(&root.join("network.json")).await?;
    if network.status == Status::Deleted {
        return Ok(network);
    }

    let started = std::time::Instant::now();
    log::info!(
        "operation=inspect_status target={} duration_ms=0 outcome=running",
        network.id
    );
    let observed = async {
        match DockerNetwork::load(root, &network).await? {
            Some(driver) => driver.status().await,
            None => Ok(Status::Stopped),
        }
    }
    .await;
    match observed {
        Ok(status) => network.status = status,
        Err(error) => {
            network.status = Status::Unknown;
            network.error = Some(format!("Could not inspect network state: {error}"));
        }
    }
    log::info!(
        "operation=inspect_status target={} duration_ms={} outcome={}",
        network.id,
        started.elapsed().as_millis(),
        if network.status == Status::Unknown {
            "failed"
        } else {
            "success"
        }
    );

    Ok(network)
}
