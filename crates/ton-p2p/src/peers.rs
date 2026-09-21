//! Restores ADNL connection descriptors alongside the shared pool's measurements.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use service_pool::{Endpoint, Options, Pool, Snapshot};
use tokio::time::{Instant, sleep, timeout};
use tracing::{info, warn};
use tycho_types::models::BlockId;

use crate::{
    ClientOptions,
    network::{Network, Peer, PeerDescriptor},
    storage::atomic_write,
};

pub(super) struct PeerEndpoint {
    id: String,
    pub(super) peer: Peer,
}

impl From<Peer> for PeerEndpoint {
    fn from(peer: Peer) -> Self {
        Self {
            id: peer.id.to_string(),
            peer,
        }
    }
}

impl Endpoint for PeerEndpoint {
    fn id(&self) -> &str {
        &self.id
    }

    fn address(&self) -> String {
        self.peer.address.to_string()
    }
}

#[derive(Serialize, Deserialize)]
struct SavedPeers {
    version: u32,
    overlay: String,
    peers: Vec<PeerDescriptor>,
}

#[derive(Serialize, Deserialize)]
struct PeerProfile {
    #[serde(flatten)]
    nodes: SavedPeers,
    statistics: Snapshot,
    masterchain_block: BlockId,
    shard_block: Option<BlockId>,
    samples: usize,
}

pub(super) fn import_profile(
    network: &Network,
    pool: &Pool<PeerEndpoint>,
    path: &Path,
) -> Result<()> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("cannot read peer profile {}", path.display()))?;
    let profile: PeerProfile = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid peer profile {}", path.display()))?;
    ensure!(
        profile.nodes.version == 1 && profile.nodes.overlay == network.overlay_id.to_string(),
        "peer profile {} belongs to another network or schema version",
        path.display()
    );
    pool.restore_snapshot(profile.statistics)?;
    for descriptor in profile.nodes.peers {
        if let Some(peer) = network
            .restore_peer(&descriptor)
            .with_context(|| format!("invalid descriptor in {}", path.display()))?
        {
            pool.upsert(peer.into());
        }
    }
    info!(
        operation = "p2p_peer_profile",
        target = %path.display(),
        peers = pool.len(),
        outcome = "imported",
        "imported calibrated peer profile",
    );
    Ok(())
}

pub(super) async fn export_profile(
    network: &Network,
    pool: &Pool<PeerEndpoint>,
    path: &Path,
    masterchain_block: BlockId,
    shard_block: Option<BlockId>,
    samples: usize,
) -> Result<()> {
    let profile = PeerProfile {
        nodes: SavedPeers {
            version: 1,
            overlay: network.overlay_id.to_string(),
            peers: pool
                .endpoints()
                .iter()
                .map(|endpoint| endpoint.peer.descriptor.clone())
                .collect(),
        },
        statistics: pool.snapshot(),
        masterchain_block,
        shard_block,
        samples,
    };
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(directory)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("peer profile requires a UTF-8 filename")?;
        atomic_write(directory, name, &serde_json::to_vec_pretty(&profile)?)
    })
    .await
    .context("peer profile writer failed")?
}

/// Measurements alone cannot establish ADNL sessions. Restore validated public
/// keys and addresses so the first download does not have to wait for DHT discovery.
pub(super) fn open(network: &Network, options: &ClientOptions) -> Result<Pool<PeerEndpoint>> {
    let mut pool = Pool::new(
        format!("ton-p2p:{}", network.overlay_id),
        Options {
            max_in_flight: options.parallelism,
            // A block operation sends metadata, block and proof queries. Space
            // operations to leave room under TON's public per-peer RPC limits,
            // including when masterchain and shard downloads share a peer.
            min_request_interval: Duration::from_millis(35),
            attempt_timeout: options.network.timeout,
            request_timeout: options.network.timeout,
            ..Options::default()
        },
    )?;
    if let Some(path) = &options.peers_file {
        import_profile(network, &pool, path)?;
    }
    pool.persist_to(options.data_dir.join("peers.json"))?;

    let path = options.data_dir.join("peer-nodes.json");
    let started = Instant::now();
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(pool),
        Err(error) => return Err(error).with_context(|| format!("cannot read {}", path.display())),
    };
    let saved: SavedPeers = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid peer cache {}", path.display()))?;

    if saved.version != 1 || saved.overlay != network.overlay_id.to_string() {
        warn!(
            operation = "p2p_peer_cache",
            target = %path.display(),
            outcome = "ignored",
            "saved peer descriptors belong to another overlay or schema version",
        );
        return Ok(pool);
    }

    for descriptor in saved.peers {
        match network.restore_peer(&descriptor) {
            Ok(Some(peer)) => pool.upsert(peer.into()),
            Ok(None) => {}
            Err(error) => warn!(
                operation = "p2p_peer_cache",
                target = %descriptor.address,
                outcome = "rejected",
                error = %format!("{error:#}"),
                "invalid saved peer descriptor",
            ),
        }
    }

    info!(
        operation = "p2p_peer_cache",
        target = %path.display(),
        peers = pool.len(),
        duration_ms = started.elapsed().as_millis(),
        outcome = "restored",
        "restored block download peers",
    );
    Ok(pool)
}

/// Save the accumulated address book because a DHT refresh returns only a subset.
/// The snapshot contains public overlay descriptors, never the client's secret key.
async fn save(network: &Network, pool: &Pool<PeerEndpoint>, directory: PathBuf) -> Result<()> {
    let saved = SavedPeers {
        version: 1,
        overlay: network.overlay_id.to_string(),
        peers: pool
            .endpoints()
            .iter()
            .map(|endpoint| endpoint.peer.descriptor.clone())
            .collect(),
    };
    tokio::task::spawn_blocking(move || {
        let bytes = serde_json::to_vec_pretty(&saved)?;
        atomic_write(&directory, "peer-nodes.json", &bytes)
    })
    .await
    .context("peer cache writer failed")?
}

/// Publish new endpoints directly into the pool so pending requests can use them
/// before their existing attempts finish. The owning client's task cancels discovery.
pub(super) async fn discover(
    network: Arc<Network>,
    pool: Pool<PeerEndpoint>,
    directory: PathBuf,
    deadline: Duration,
    minimum_peers: usize,
) {
    loop {
        let started = Instant::now();
        let result = timeout(deadline, network.find_peers())
            .await
            .context("block peer discovery timed out")
            .and_then(std::convert::identity);

        match result {
            Ok(peers) => {
                for peer in peers {
                    pool.upsert(peer.into());
                }

                if let Err(error) = save(&network, &pool, directory.clone()).await {
                    warn!(
                        operation = "p2p_peer_cache",
                        target = %directory.display(),
                        outcome = "failed",
                        error = %format!("{error:#}"),
                        "could not save peer descriptors",
                    );
                }

                info!(
                    operation = "p2p_discovery",
                    node = %network.dht.key().id(),
                    target = %network.overlay_id,
                    peers = pool.len(),
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "refreshed",
                    "refreshed block download peers",
                );
            }
            Err(error) => warn!(
                operation = "p2p_discovery",
                target = %network.overlay_id,
                duration_ms = started.elapsed().as_millis(),
                outcome = "retry",
                error = %format!("{error:#}"),
                "could not refresh block download peers",
            ),
        }

        let interval = if pool.len() < minimum_peers { 2 } else { 30 };
        sleep(Duration::from_secs(interval)).await;
    }
}
