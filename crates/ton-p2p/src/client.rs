//! Downloads blocks and maintains a resumable cache independently of consumers.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anyhow::{Context, Result, ensure};
use futures::{StreamExt, stream};
use service_pool::{Failure, Pool};
use tokio::{task::JoinSet, time::Instant};
use tracing::{debug, info};
use tycho_types::models::{BlockId, ShardIdent};

use crate::{
    NetworkConfig, NetworkOptions,
    download::{download_from_peer, download_shard_from_peer, validate_block},
    network::Network,
    peers::{self, PeerEndpoint},
    storage::{Storage, atomic_write},
};

/// Connection settings and cache location for one client.
/// The data directory remains exclusively locked until the client is dropped.
pub struct ClientOptions {
    /// Advertised UDP address, identity, and per-request deadline.
    pub network: NetworkOptions,
    /// Original block BOCs and the masterchain download checkpoint.
    pub data_dir: PathBuf,
    /// Optional read-only peer profile produced by calibration. Runtime updates
    /// remain in the data directory. Available since trunk.
    pub peers_file: Option<PathBuf>,
    /// Shared limit for masterchain and shard download attempts, including competing peers.
    /// Accepted values are 1..=128. Available since trunk.
    pub parallelism: usize,
}

/// P2P block client with a persistent cache and a single masterchain download stream.
///
/// Dropping it cancels background discovery and prefetch, then releases the cache lock.
/// Callers decide which shard IDs to fetch and own any application-level progress.
pub struct Client {
    network: Arc<Network>,
    storage: Storage,
    ids: BTreeMap<u32, BlockId>,
    peers: Pool<PeerEndpoint>,
    _discovery: JoinSet<()>,
    masterchain_download: JoinSet<Option<DownloadedMasterchain>>,
    options: ClientOptions,
}

/// A downloaded block awaiting commit. The background task cannot update storage.
struct DownloadedMasterchain {
    id: BlockId,
    block: Vec<u8>,
    proof: Vec<u8>,
}

impl Client {
    /// Opens the cache and UDP transport, restores peers, and starts discovery.
    /// Fails if settings, the saved head, or the directory lock are invalid.
    pub fn open(config: &NetworkConfig, options: ClientOptions) -> Result<Self> {
        ensure!(
            !options.network.timeout.is_zero(),
            "P2P timeout must be positive"
        );
        ensure!(
            (1..=128).contains(&options.parallelism),
            "P2P parallelism must be between 1 and 128"
        );

        let storage = Storage::open(&options.data_dir, config)?;
        storage.restore()?;
        let ids = storage.masterchain_ids()?;
        let network = Arc::new(Network::new(config, &options.network)?);
        let peers = peers::open(&network, &options)?;
        let mut discovery = JoinSet::new();
        discovery.spawn(peers::discover(
            Arc::clone(&network),
            peers.clone(),
            options.data_dir.clone(),
            options.network.timeout,
            options.parallelism,
        ));

        info!(
            operation = "p2p_client",
            node = %network.dht.key().id(),
            target = %storage.head(),
            data_dir = %options.data_dir.display(),
            parallelism = options.parallelism,
            verification = "hashes_and_links",
            outcome = "opened",
            "opened P2P block client",
        );

        Ok(Self {
            network,
            storage,
            ids,
            peers,
            _discovery: discovery,
            masterchain_download: JoinSet::new(),
            options,
        })
    }

    /// Returns the trusted starting block recorded in this cache.
    #[must_use]
    pub const fn anchor(&self) -> BlockId {
        self.storage.anchor()
    }

    /// Returns the persisted head, or `None` while the starting block is missing.
    /// A zerostate counts as a starting point without a block BOC.
    #[must_use]
    pub fn head(&self) -> Option<BlockId> {
        (!self.storage.needs_anchor()).then(|| self.storage.head())
    }

    /// Checks a consumer's position against the cached chain. A position ahead
    /// of the cache is allowed and must be checked again when downloads reach it.
    pub fn validate_checkpoint(&self, checkpoint: &BlockId) -> Result<()> {
        ensure!(
            checkpoint.shard == ShardIdent::MASTERCHAIN && checkpoint.seqno >= self.anchor().seqno,
            "checkpoint {checkpoint} precedes or conflicts with P2P anchor {}",
            self.anchor()
        );

        if checkpoint.seqno <= self.storage.head().seqno {
            ensure!(
                self.ids.get(&checkpoint.seqno) == Some(checkpoint),
                "checkpoint {checkpoint} conflicts with the masterchain cache"
            );
        }

        Ok(())
    }

    /// Downloads and commits the starting block or the successor of the saved head.
    /// Returns `None` when the current peers cannot supply it. The next block is
    /// prefetched in the background, but remains uncommitted until the next call.
    pub async fn next_masterchain(&mut self) -> Result<Option<BlockId>> {
        let downloaded = self.advance_masterchain().await?;

        Ok(downloaded.then(|| self.storage.head()))
    }

    /// Reads a committed masterchain block by sequence number. Returns `None`
    /// outside the cached range or for seqno zero, which identifies a state.
    pub async fn masterchain_block(&self, seqno: u32) -> Result<Option<(BlockId, Vec<u8>)>> {
        let Some(&id) = self.ids.get(&seqno).filter(|id| id.seqno != 0) else {
            return Ok(None);
        };
        let path = self.storage.block_path(&id);
        let boc = tokio::fs::read(&path)
            .await
            .with_context(|| format!("cannot read masterchain block {}", path.display()))?;

        Ok(Some((id, boc)))
    }

    /// Loads shard BOCs from cache or peers in input order. Each ID must come
    /// from a trusted block reference. Missing blocks produce `None` entries;
    /// cache failures stop the operation. Valid downloads remain cached for retries.
    pub async fn download_shards(&mut self, ids: &[BlockId]) -> Result<Vec<Option<Vec<u8>>>> {
        ensure!(
            ids.iter()
                .all(|id| !id.shard.is_masterchain() && id.seqno > 0),
            "shard downloads require nonzero shard block IDs"
        );

        stream::iter(ids.iter().copied())
            .map(|id| download_shard(&self.network, &self.peers, &self.options, id))
            .buffered(self.options.parallelism)
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect()
    }

    /// Downloads one successor while the caller processes the committed block.
    /// The task returns bytes; only `advance_masterchain` may commit them.
    fn prefetch_masterchain(&mut self) {
        if !self.masterchain_download.is_empty() || self.peers.is_empty() {
            return;
        }

        let head = self.storage.head();
        let previous = (!self.storage.needs_anchor()).then_some(head);
        let network = Arc::clone(&self.network);
        let deadline = self.options.network.timeout;
        let peers = self.peers.clone();

        self.masterchain_download.spawn(async move {
            let result = peers
                .execute_with_probes("masterchain", move |endpoint| {
                    let network = Arc::clone(&network);
                    async move {
                        download_from_peer(
                            &network,
                            &endpoint.peer,
                            &head,
                            previous.as_ref(),
                            deadline,
                        )
                        .await
                        .map_err(crate::download::pool_failure)?
                        .ok_or_else(|| {
                            Failure::Unavailable(format!("masterchain successor of {head}"))
                        })
                    }
                })
                .await;

            match result {
                Ok((id, block, proof)) => Some(DownloadedMasterchain { id, block, proof }),
                Err(error) => {
                    debug!(
                        operation = "block_download",
                        target = %head,
                        outcome = "retry",
                        error = %error,
                        "masterchain download exhausted available peers",
                    );
                    None
                }
            }
        });
    }

    /// Commits one verified download before exposing its ID to the caller.
    async fn advance_masterchain(&mut self) -> Result<bool> {
        self.prefetch_masterchain();
        let Some(result) = self.masterchain_download.join_next().await else {
            return Ok(false);
        };

        let Some(DownloadedMasterchain { id, block, proof }) =
            result.context("masterchain download task failed")?
        else {
            return Ok(false);
        };

        let started = Instant::now();
        self.storage.commit(id, &block, &proof)?;
        self.ids.insert(id.seqno, id);
        debug!(
            operation = "masterchain_cache",
            target = %id,
            duration_ms = started.elapsed().as_millis(),
            outcome = "committed",
            "masterchain block and proof committed to cache",
        );

        self.prefetch_masterchain();
        Ok(true)
    }
}

/// Requests a shard by the full ID obtained from a masterchain block or a shard
/// predecessor reference. Peers on the masterchain overlay can serve shard data.
async fn download_shard(
    network: &Arc<Network>,
    peers: &Pool<PeerEndpoint>,
    options: &ClientOptions,
    id: BlockId,
) -> Result<Option<Vec<u8>>> {
    let directory = options
        .data_dir
        .join("shards")
        .join(id.shard.workchain().to_string())
        .join(format!("{:016x}", id.shard.prefix()));
    let name = format!("{}-{}-{}.boc", id.seqno, id.root_hash, id.file_hash);
    let path = directory.join(&name);

    match tokio::fs::read(&path).await {
        Ok(boc) => {
            validate_block(&id, &boc)?;
            return Ok(Some(boc));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("cannot read {}", path.display()));
        }
    }

    let started = Instant::now();
    let network = Arc::clone(network);
    let deadline = options.network.timeout;
    let result = peers
        .execute_with_probes("shard", move |endpoint| {
            let network = Arc::clone(&network);
            async move { download_shard_from_peer(&network, &endpoint.peer, id, deadline).await }
        })
        .await;

    let boc = match result {
        Ok(boc) => boc,
        Err(error) => {
            debug!(
                operation = "shard_download",
                target = %id,
                duration_ms = started.elapsed().as_millis(),
                outcome = "retry",
                error = %error,
                "shard download exhausted available peers",
            );
            return Ok(None);
        }
    };

    // Preserve the downloaded serialization because file_hash covers original bytes.
    let boc = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&directory)?;
        atomic_write(&directory, &name, &boc)?;
        Ok::<_, anyhow::Error>(boc)
    })
    .await
    .context("shard cache write task failed")??;

    debug!(
        operation = "shard_download",
        target = %id,
        bytes = boc.len(),
        duration_ms = started.elapsed().as_millis(),
        outcome = "stored",
        "shard block saved",
    );
    Ok(Some(boc))
}
