//! Direct `LiteAPI` source for canonical, decoded TON batches.
//!
//! [`TonutilsLiteClient`] owns the ADNL/LiteAPI transport. Canonical traversal
//! lives in [`ton_indexer_core::CanonicalBlockSource`] and is shared with P2P.

#[cfg(test)]
mod tests;

mod transport;

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use async_trait::async_trait;
use futures::{StreamExt, stream};
use service_pool::{Endpoint, Failure, Options, Pool, Snapshot};
use sha2::{Digest, Sha256};
use ton_indexer_core::{
    BlockData, BlockGraphClient, BlockId, BlockIdShort, Hash256, RawBlock, SourceError,
};
use tonutils::{
    network_config::ConfigGlobal,
    tl::common::{BlockId as LiteBlockId, BlockIdExt as LiteBlockIdExt, Int256},
};
use transport::{Counters, Server, classify};

/// Counts `LiteServer` TL requests issued by [`TonutilsLiteClient`].
///
/// The counters are incremented immediately before a request is sent, so failed
/// requests are included. Establishing the ADNL connection itself is a transport
/// operation and is not included.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiteRequestStats {
    get_masterchain_info: u64,
    lookup_block: u64,
    get_block: u64,
}

impl LiteRequestStats {
    /// Returns the number of `liteServer.getMasterchainInfo` requests.
    #[must_use]
    pub const fn get_masterchain_info(self) -> u64 {
        self.get_masterchain_info
    }

    /// Returns the number of `liteServer.lookupBlock` requests.
    #[must_use]
    pub const fn lookup_block(self) -> u64 {
        self.lookup_block
    }

    /// Returns the number of `liteServer.getBlock` requests.
    #[must_use]
    pub const fn get_block(self) -> u64 {
        self.get_block
    }

    /// Returns the total number of counted `LiteServer` requests.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.get_masterchain_info + self.lookup_block + self.get_block
    }

    /// Returns requests made since an earlier snapshot.
    #[must_use]
    pub const fn since(self, earlier: Self) -> Self {
        Self {
            get_masterchain_info: self
                .get_masterchain_info
                .saturating_sub(earlier.get_masterchain_info),
            lookup_block: self.lookup_block.saturating_sub(earlier.lookup_block),
            get_block: self.get_block.saturating_sub(earlier.get_block),
        }
    }
}

/// ADNL/LiteAPI client with shared endpoint admission, failover, and measurements.
/// Responses are validated before accepting a speculative attempt. Available since trunk.
pub struct TonutilsLiteClient {
    pool: Pool<Server>,
    parallelism: usize,
    decoded: HashMap<BlockId, BlockData>,
    counters: Arc<Counters>,
}

impl TonutilsLiteClient {
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
    const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
    const DEFAULT_PARALLEL_CLIENTS: usize = 4;
    const MAX_PARALLEL_CLIENTS: usize = 16;

    /// Connects to a responsive liteserver pool from a parsed global config.
    ///
    /// # Errors
    ///
    /// Returns an error when the config has no liteservers or none of them
    /// accepts an ADNL connection and answers a `getMasterchainInfo` probe.
    pub async fn connect(config: &ConfigGlobal) -> Result<Self, SourceError> {
        Self::connect_with_parallelism(config, Self::DEFAULT_PARALLEL_CLIENTS).await
    }

    /// Bounds simultaneous upstream operations, including speculative attempts.
    /// Available since trunk.
    ///
    /// Values above 16 are capped to protect public liteservers. Zero is treated
    /// as one active operation. Idle connections can remain on several servers.
    ///
    /// # Errors
    ///
    /// Returns an error when the config has no liteservers or none of them
    /// accepts an ADNL connection and answers a `getMasterchainInfo` probe.
    pub async fn connect_with_parallelism(
        config: &ConfigGlobal,
        parallelism: usize,
    ) -> Result<Self, SourceError> {
        Self::connect_pool(config, parallelism, None).await
    }

    async fn connect_pool(
        config: &ConfigGlobal,
        parallelism: usize,
        stats_path: Option<&Path>,
    ) -> Result<Self, SourceError> {
        if config.liteservers.is_empty() {
            return Err(SourceError::GlobalConfig(
                "network config has no liteservers".into(),
            ));
        }

        let parallelism = parallelism.clamp(1, Self::MAX_PARALLEL_CLIENTS);
        let counters = Arc::new(Counters::default());
        let servers: Vec<_> = config
            .liteservers
            .iter()
            .map(|settings| Server::new(settings.clone(), Arc::clone(&counters)))
            .collect();
        let mut identities: Vec<_> = servers.iter().map(Endpoint::id).collect();
        identities.sort_unstable();
        identities.dedup();
        // ConfigGlobal contains only server keys. Scope saved measurements to this
        // trusted inventory so a different config cannot inherit its ranking.
        let namespace = format!(
            "ton-liteserver:{}",
            hex::encode(Sha256::digest(identities.join(",")))
        );
        let mut pool = Pool::new(
            namespace,
            Options {
                max_in_flight: parallelism,
                max_in_flight_per_endpoint: parallelism.min(4),
                attempt_timeout: Self::REQUEST_TIMEOUT,
                request_timeout: Self::REQUEST_TIMEOUT,
                ..Options::default()
            },
        )
        .map_err(source_failure)?;

        if let Some(path) = stats_path {
            pool.persist_to(path).map_err(|error| {
                SourceError::Transport(format!("cannot restore {}: {error}", path.display()))
            })?;
        }
        for server in servers {
            pool.upsert(server);
        }

        let mut client = Self {
            pool,
            parallelism,
            decoded: HashMap::new(),
            counters,
        };
        client.latest().await?;
        Ok(client)
    }

    /// Reads a global config and opens a pool of its liteservers.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or parsed, or when the
    /// ADNL connection cannot be established.
    pub async fn connect_path(path: impl AsRef<Path>) -> Result<Self, SourceError> {
        Self::connect_path_with_parallelism(path, Self::DEFAULT_PARALLEL_CLIENTS).await
    }

    /// Reads a global config and connects with configurable exact-load parallelism.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or parsed, or when the
    /// ADNL connection cannot be established.
    pub async fn connect_path_with_parallelism(
        path: impl AsRef<Path>,
        parallelism: usize,
    ) -> Result<Self, SourceError> {
        Self::connect_pool(&read_config(path.as_ref()).await?, parallelism, None).await
    }

    /// Restores endpoint measurements before selecting the first server, and saves
    /// updates atomically beside caller-owned data. Available since trunk.
    ///
    /// # Errors
    /// Returns an error for an unreadable config or malformed statistics file,
    /// or when no configured server answers within the request deadline.
    pub async fn connect_path_with_stats(
        path: impl AsRef<Path>,
        parallelism: usize,
        stats_path: impl AsRef<Path>,
    ) -> Result<Self, SourceError> {
        Self::connect_pool(
            &read_config(path.as_ref()).await?,
            parallelism,
            Some(stats_path.as_ref()),
        )
        .await
    }

    /// Captures endpoint outcomes and latency estimates for monitoring. Available since trunk.
    #[must_use]
    pub fn peer_stats(&self) -> Snapshot {
        self.pool.snapshot()
    }

    /// Saves measurements before a controlled shutdown. Available since trunk.
    ///
    /// # Errors
    /// Returns an error when the configured statistics file cannot be written.
    pub async fn flush_peer_stats(&self) -> std::io::Result<()> {
        self.pool.flush().await
    }

    /// Returns the latest masterchain id without constructing a source.
    ///
    /// # Errors
    ///
    /// Returns an error when the `LiteAPI` request or id conversion fails.
    pub async fn latest(&mut self) -> Result<BlockId, SourceError> {
        self.latest_masterchain_block().await
    }

    /// Resolves a bootstrap anchor while checking the server's network identity.
    /// The returned ID is trusted metadata, not a verified chain of block proofs.
    /// No block bodies or account states are downloaded by this request.
    pub async fn latest_for_network(
        &mut self,
        zero_state: BlockId,
    ) -> Result<BlockId, SourceError> {
        if !zero_state.is_masterchain()
            || zero_state.shard != BlockId::FULL_SHARD
            || zero_state.seqno != 0
        {
            return Err(SourceError::InvalidBlockId(
                "network anchor must be a masterchain zerostate".into(),
            ));
        }
        self.head(Some(zero_state)).await
    }

    async fn head(&self, zero_state: Option<BlockId>) -> Result<BlockId, SourceError> {
        self.pool
            .execute_with_probes("metadata", move |server| async move {
                let counters = Arc::clone(&server.counters);
                let request = server.request(move |client| {
                    Box::pin(async move {
                        counters.head.fetch_add(1, Ordering::Relaxed);
                        let info = client.get_masterchain_info().await.map_err(classify)?;
                        if zero_state.is_some_and(|zero| {
                            info.init.workchain != zero.workchain
                                || info.init.root_hash.0 != zero.root_hash.into_bytes()
                                || info.init.file_hash.0 != zero.file_hash.into_bytes()
                        }) {
                            return Err(Failure::Invalid(
                                "liteserver zerostate does not match the requested network".into(),
                            ));
                        }

                        let last = from_lite_block_id(&info.last).map_err(invalid_response)?;
                        if !last.is_masterchain() || last.shard != BlockId::FULL_SHARD {
                            return Err(Failure::Invalid(
                                "liteserver head must belong to the masterchain".into(),
                            ));
                        }
                        Ok(last)
                    })
                });
                tokio::time::timeout(Duration::from_secs(3), request)
                    .await
                    .map_err(|_| {
                        Failure::Retryable("masterchain metadata timed out after 3s".into())
                    })?
            })
            .await
            .map_err(source_failure)
    }

    /// Returns the number of messages waiting in all shard outbound queues.
    ///
    /// The liteserver reports one queue size per shard. Summing them produces
    /// the network-wide backlog expected by monitoring consumers.
    ///
    /// # Errors
    ///
    /// Returns an error when the `LiteAPI` request fails.
    pub async fn out_msg_queue_size(&mut self) -> Result<u64, SourceError> {
        self.pool
            .execute_with_probes("queue_sizes", |server| async move {
                server
                    .request(|client| {
                        Box::pin(async move {
                            let sizes = client
                                .get_out_msg_queue_sizes(None)
                                .await
                                .map_err(classify)?;
                            Ok(sizes.shards.into_iter().fold(0_u64, |total, shard| {
                                total.saturating_add(u64::from(shard.size))
                            }))
                        })
                    })
                    .await
            })
            .await
            .map_err(source_failure)
    }

    /// Returns physical TL requests, including retries and speculative attempts.
    #[must_use]
    pub fn request_stats(&self) -> LiteRequestStats {
        self.counters.snapshot()
    }

    /// Returns the shared admission limit, including speculative block downloads.
    #[must_use]
    pub const fn exact_block_parallelism(&self) -> usize {
        self.parallelism
    }

    fn decode_cached(&mut self, raw: &RawBlock) -> Result<&BlockData, SourceError> {
        Ok(match self.decoded.entry(raw.id()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(BlockData::decode(raw.id(), raw.boc())?)
            }
        })
    }
}

#[async_trait]
impl BlockGraphClient for TonutilsLiteClient {
    async fn latest_masterchain_block(&mut self) -> Result<BlockId, SourceError> {
        self.head(None).await
    }

    async fn load_block(&mut self, id: BlockIdShort) -> Result<RawBlock, SourceError> {
        let lite_id = to_lite_short_id(id)?;
        let (raw, decoded) = self
            .pool
            .execute_with_probes("lookup_block", move |server| {
                let lite_id = lite_id.clone();
                async move {
                    let counters = Arc::clone(&server.counters);
                    server
                        .request(move |client| {
                            Box::pin(async move {
                                counters.lookup.fetch_add(1, Ordering::Relaxed);
                                let header = client
                                    .lookup_block(
                                        (),
                                        lite_id,
                                        Some(()),
                                        None,
                                        None,
                                        false,
                                        false,
                                        false,
                                        false,
                                        false,
                                    )
                                    .await
                                    .map_err(classify)?;
                                let full_id =
                                    from_lite_block_id(&header.id).map_err(invalid_response)?;
                                if BlockIdShort::from(full_id) != id {
                                    return Err(Failure::Invalid(
                                        "lookup returned different block coordinates".into(),
                                    ));
                                }

                                counters.block.fetch_add(1, Ordering::Relaxed);
                                let boc = client.get_block(header.id).await.map_err(classify)?;
                                let decoded =
                                    BlockData::decode(full_id, &boc).map_err(invalid_response)?;
                                Ok((RawBlock::new(full_id, boc), decoded))
                            })
                        })
                        .await
                }
            })
            .await
            .map_err(source_failure)?;
        self.decoded.insert(raw.id(), decoded);
        Ok(raw)
    }

    async fn load_block_exact(&mut self, id: BlockId) -> Result<RawBlock, SourceError> {
        self.load_blocks_exact(&[id])
            .await?
            .pop()
            .ok_or_else(|| SourceError::InvalidBatch("exact block load returned no block".into()))
    }

    async fn load_blocks_exact(&mut self, ids: &[BlockId]) -> Result<Vec<RawBlock>, SourceError> {
        let requests = ids.iter().copied().map(|id| {
            let pool = &self.pool;
            async move {
                let lite_id = to_lite_block_id_ext(id)?;
                pool.execute_with_probes("block", move |server| {
                    let lite_id = lite_id.clone();
                    async move {
                        let counters = Arc::clone(&server.counters);
                        server
                            .request(move |client| {
                                Box::pin(async move {
                                    counters.block.fetch_add(1, Ordering::Relaxed);
                                    let boc = client.get_block(lite_id).await.map_err(classify)?;
                                    let decoded =
                                        BlockData::decode(id, &boc).map_err(invalid_response)?;
                                    Ok((RawBlock::new(id, boc), decoded))
                                })
                            })
                            .await
                    }
                })
                .await
                .map_err(source_failure)
            }
        });
        let results = stream::iter(requests)
            .buffered(self.parallelism)
            .collect::<Vec<_>>()
            .await;
        let mut blocks = Vec::with_capacity(ids.len());
        for result in results {
            let (raw, decoded) = result?;
            self.decoded.insert(raw.id(), decoded);
            blocks.push(raw);
        }
        Ok(blocks)
    }

    async fn shard_frontier(&mut self, mc_block: &RawBlock) -> Result<Vec<BlockId>, SourceError> {
        Ok(self.decode_cached(mc_block)?.shard_frontier()?)
    }

    async fn predecessors(&mut self, block: &RawBlock) -> Result<Vec<BlockId>, SourceError> {
        Ok(self.decode_cached(block)?.predecessors()?)
    }

    async fn decode_block(&mut self, block: RawBlock) -> Result<BlockData, SourceError> {
        match self.decoded.remove(&block.id()) {
            Some(decoded) => Ok(decoded),
            None => Ok(BlockData::decode(block.id(), block.boc())?),
        }
    }
}

fn to_lite_short_id(id: BlockIdShort) -> Result<LiteBlockId, SourceError> {
    Ok(LiteBlockId {
        workchain: id.workchain,
        shard: i64::from_ne_bytes(id.shard.to_ne_bytes()),
        seqno: i32::try_from(id.seqno).map_err(|_| {
            SourceError::InvalidBlockId(format!("seqno {} exceeds signed TL range", id.seqno))
        })?,
    })
}

fn to_lite_block_id_ext(id: BlockId) -> Result<LiteBlockIdExt, SourceError> {
    let short = to_lite_short_id(id.into())?;
    Ok(LiteBlockIdExt {
        workchain: short.workchain,
        shard: short.shard,
        seqno: short.seqno,
        root_hash: Int256(id.root_hash.into_bytes()),
        file_hash: Int256(id.file_hash.into_bytes()),
    })
}

async fn read_config(path: &Path) -> Result<ConfigGlobal, SourceError> {
    let source = tokio::fs::read_to_string(path).await.map_err(|error| {
        SourceError::GlobalConfig(format!("failed to read {}: {error}", path.display()))
    })?;
    source.parse().map_err(|error| {
        SourceError::GlobalConfig(format!("failed to parse {}: {error}", path.display()))
    })
}

fn invalid_response(error: impl std::fmt::Display) -> Failure {
    Failure::Invalid(error.to_string())
}

fn source_failure(error: Failure) -> SourceError {
    SourceError::Transport(error.to_string())
}

fn from_lite_block_id(id: &LiteBlockIdExt) -> Result<BlockId, SourceError> {
    Ok(BlockId {
        workchain: id.workchain,
        shard: u64::from_ne_bytes(id.shard.to_ne_bytes()),
        seqno: u32::try_from(id.seqno)
            .map_err(|_| SourceError::InvalidBlockId(format!("negative seqno {}", id.seqno)))?,
        root_hash: Hash256::new(id.root_hash.0),
        file_hash: Hash256::new(id.file_hash.0),
    })
}
