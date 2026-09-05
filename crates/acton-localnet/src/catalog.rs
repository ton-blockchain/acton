//! Filesystem discovery reserves names and ports without owning network services.

use crate::{CreateNetwork, Error, Network, NetworkConfig, Status, storage};
use std::path::{Path, PathBuf};

/// A persisted network and its own service directory. Discovery is read-only;
/// selecting one directory never opens, stops, or resumes the other networks.
#[derive(Clone)]
pub struct NetworkDirectory {
    pub network: Network,
    pub path: PathBuf,
}

impl NetworkDirectory {
    /// Prepares only the selected network for the current directory layout.
    /// The catalog lock prevents concurrent renames and rejects an old catalog-wide
    /// service that is still using the previous layout.
    pub async fn prepare(mut self, root: &Path) -> Result<Self, Error> {
        let _lock = storage::catalog_lock(root).await?;
        // CLI and application roots can reach the same directory through /tmp
        // or another symlink. Compare canonical paths before validating ownership.
        let root = dunce::canonicalize(root).map_err(|error| Error::storage(root, error))?;
        self.path =
            dunce::canonicalize(&self.path).map_err(|error| Error::storage(&self.path, error))?;
        let _network_lock = storage::lock(&self.path)?;
        // Reload under the network lock: discovery may have raced the last
        // operation of a service that has since stopped.
        self.network = storage::read_json(&self.path.join("network.json")).await?;
        self.path =
            storage::prepare_network_directory(&root, &self.path, &mut self.network).await?;
        storage::write_json(&self.path.join("network.json"), &self.network).await?;
        Ok(self)
    }
}

/// Lists persisted definitions without contacting Docker or starting any service.
/// Each running service is the sole writer of its own atomically updated record.
pub async fn list(root: &Path) -> Result<Vec<NetworkDirectory>, Error> {
    let parent = root.join("networks");
    let mut files = match tokio::fs::read_dir(&parent).await {
        Ok(files) => files,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(Error::storage(&parent, error)),
    };
    let mut networks = Vec::new();

    while let Some(file) = files
        .next_entry()
        .await
        .map_err(|e| Error::storage(&parent, e))?
    {
        if !file
            .file_type()
            .await
            .map_err(|e| Error::storage(&file.path(), e))?
            .is_dir()
        {
            continue;
        }
        let path = file.path().join("network.json");
        if !path.exists() {
            continue;
        }
        let network: Network = storage::read_json(&path).await?;
        storage::validate_id(&network.id)?;
        if network.status != Status::Deleted {
            networks.push(NetworkDirectory {
                network,
                path: file.path(),
            });
        }
    }
    networks.sort_by(|a, b| a.network.name.cmp(&b.network.name));
    Ok(networks)
}

/// Creates a stopped network definition. Genesis work begins only on `start`.
/// Names are unique among live definitions; ports are reserved in this catalog.
pub async fn create(root: &Path, request: CreateNetwork) -> Result<NetworkDirectory, Error> {
    let _lock = storage::catalog_lock(root).await?;
    let root = dunce::canonicalize(root).map_err(|e| Error::storage(root, e))?;
    let parent = root.join("networks");
    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(|e| Error::storage(&parent, e))?;

    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(Error::invalid(
            "A network name must contain 1 to 80 characters",
        ));
    }

    if request.block_time_ms == Some(0) {
        return Err(Error::invalid("Block time must be greater than zero"));
    }

    if request.election_time_seconds.is_some_and(|v| v < 4) {
        return Err(Error::invalid("Election time must be at least 4 seconds"));
    }

    for boc in &request.imported_account_bocs {
        if boc.is_empty() || boc.len() % 2 != 0 || !boc.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::invalid(
                "Imported ShardAccount BoCs must be nonempty hexadecimal strings",
            ));
        }
    }

    let existing = list(&root).await?;
    if existing.iter().any(|n| n.network.name == name) {
        return Err(Error::Conflict {
            code: "network_name_exists",
            message: format!("Network {name} already exists"),
        });
    }

    let requested = [
        request.ports.config,
        request.ports.admin,
        request.ports.api_v2,
        request.ports.api_v3,
        request.ports.observability,
    ];
    let available_port = |port: u16| {
        port > 0
            && !request.reserved_ports.contains(&port)
            && existing
                .iter()
                .all(|n| !n.network.config.ports().all().contains(&port))
            && std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).is_ok()
    };
    let mut explicit = Vec::new();
    for port in requested.into_iter().flatten() {
        if explicit.contains(&port) || !available_port(port) {
            return Err(Error::invalid(format!(
                "Port {port} is unavailable or used by another endpoint"
            )));
        }
        explicit.push(port);
    }

    // Allocate only unspecified endpoints. An explicit port can occupy a slot
    // in the first default range, so that range must be skipped, not rejected.
    let available = |base: u16| {
        base > 0
            && base <= 65531
            && (base..=base + 4)
                .zip(requested)
                .all(|(port, explicit_port)| {
                    explicit_port.is_some() || (!explicit.contains(&port) && available_port(port))
                })
    };

    let port_base = match request.port_base {
        Some(base) if available(base) => base,
        Some(_) => {
            return Err(Error::invalid(
                "The requested five-port range is unavailable",
            ));
        }
        None => (19000..=65531)
            .step_by(5)
            .find(|base| available(*base))
            .ok_or_else(|| Error::invalid("No port range is available"))?,
    };

    let defaults = [
        port_base,
        port_base + 1,
        port_base + 2,
        port_base + 3,
        port_base + 4,
    ];
    let selected: Vec<_> = requested
        .into_iter()
        .zip(defaults)
        .map(|(requested, default)| requested.unwrap_or(default))
        .collect();
    let ports = crate::NetworkPorts {
        config: selected[0],
        admin: selected[1],
        api_v2: selected[2],
        api_v3: selected[3],
        observability: selected[4],
    };

    let config = NetworkConfig {
        port_base,
        ports: Some(ports),
        block_time_ms: request.block_time_ms,
        election_time_seconds: request.election_time_seconds,
        imported_account_bocs: request.imported_account_bocs,
    };

    let id = uuid::Uuid::new_v4().to_string();
    let record = Network {
        id: id.clone(),
        name: name.to_owned(),
        endpoints: config.endpoints(),
        config,
        nodes: Vec::new(),
        state: None,
        status: Status::Stopped,
        operation: None,
        snapshot_operation: None,
        startup_timings: None,
        error: None,
    };

    let data_dir = storage::network_directory(&root, &record);
    tokio::fs::create_dir(&data_dir)
        .await
        .map_err(|e| Error::storage(&data_dir, e))?;
    storage::write_json(&data_dir.join("network.json"), &record).await?;
    Ok(NetworkDirectory {
        network: record,
        path: data_dir,
    })
}
