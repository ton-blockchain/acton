//! Direct `LiteAPI` source for canonical, decoded TON batches.
//!
//! [`TonutilsLiteClient`] owns the ADNL/LiteAPI transport. The generic
//! [`CanonicalBlockSource`] contains the deterministic masterchain-frontier
//! algorithm and can be tested without a network.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration,
};

use async_trait::async_trait;
use futures::{StreamExt, future::join_all, stream::FuturesUnordered};
use thiserror::Error;
use ton_indexer_core::{Batch, BlockData, BlockId, BlockSource, Error as IndexerError, Hash256};
use tonutils::{
    liteclient::client::LiteClient,
    network_config::{ConfigGlobal, ConfigLiteServer},
    tl::common::{BlockId as LiteBlockId, BlockIdExt as LiteBlockIdExt, Int256},
};

/// Errors produced by the `LiteAPI` source and canonical traversal.
#[derive(Debug, Error)]
pub enum SourceError {
    /// A global config could not be read or parsed.
    #[error("invalid global config: {0}")]
    GlobalConfig(String),
    /// A `LiteAPI` request failed.
    #[error("LiteAPI request failed: {0}")]
    LiteApi(String),
    /// A numeric field cannot be represented by the `LiteAPI` TL type.
    #[error("invalid LiteAPI block id: {0}")]
    InvalidBlockId(String),
    /// TON model decoding failed while inspecting a block.
    #[error(transparent)]
    Decode(#[from] ton_indexer_core::DecodeError),
    /// A loaded block differs from the full id committed by its successor.
    #[error("requested block {expected}, transport returned {actual}")]
    UnexpectedBlock {
        /// Full block id expected by the traversal.
        expected: Box<BlockId>,
        /// Full block id returned by the transport.
        actual: Box<BlockId>,
    },
    /// Two consecutive masterchain lookups do not form one chain.
    #[error("masterchain block {next} does not directly follow {previous}; references {actual:?}")]
    MasterchainDiscontinuity {
        /// Previous masterchain block loaded by sequence number.
        previous: Box<BlockId>,
        /// Next masterchain block loaded by sequence number.
        next: Box<BlockId>,
        /// Predecessors declared by the next block.
        actual: Vec<BlockId>,
    },
    /// A frontier walk exceeded its configured safety bound.
    #[error("shard delta exceeded the {limit}-block traversal limit")]
    TraversalLimit {
        /// Configured maximum.
        limit: usize,
    },
    /// The current and previous masterchain frontiers do not form one connected delta.
    #[error("shard frontier did not reach previous blocks: {missing:?}")]
    DisconnectedFrontier {
        /// Previous frontier ids not reached from the current frontier.
        missing: Vec<BlockId>,
    },
    /// A canonical batch invariant was violated.
    #[error(transparent)]
    Indexer(#[from] IndexerError),
    /// Raw transport blocks did not form one canonical batch.
    #[error("invalid raw batch: {0}")]
    InvalidBatch(String),
}

/// `LiteAPI` block coordinates without representation hashes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockIdShort {
    /// Workchain identifier.
    pub workchain: i32,
    /// Shard prefix.
    pub shard: u64,
    /// Block sequence number.
    pub seqno: u32,
}

impl From<BlockId> for BlockIdShort {
    fn from(id: BlockId) -> Self {
        Self {
            workchain: id.workchain,
            shard: id.shard,
            seqno: id.seqno,
        }
    }
}

/// Exact block payload returned by `LiteAPI`.
#[derive(Clone, Debug)]
pub struct RawBlock {
    id: BlockId,
    boc: Vec<u8>,
}

impl RawBlock {
    /// Creates a raw block with its full transport id.
    pub fn new(id: BlockId, boc: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            boc: boc.into(),
        }
    }

    /// Returns the full block id.
    #[must_use]
    pub const fn id(&self) -> BlockId {
        self.id
    }

    /// Returns the serialized block `BoC`.
    #[must_use]
    pub fn boc(&self) -> &[u8] {
        &self.boc
    }
}

/// Block access required by the canonical traversal.
///
/// Implement this trait to reuse the canonicalizer with another transport,
/// cache, archive, or test fixture.
#[async_trait]
pub trait BlockGraphClient: Send {
    /// Returns the latest known masterchain block.
    async fn latest_masterchain_block(&mut self) -> Result<BlockId, SourceError>;

    /// Resolves a short id and downloads the exact block `BoC`.
    async fn load_block(&mut self, id: BlockIdShort) -> Result<RawBlock, SourceError>;

    /// Downloads a block whose full id is already known.
    ///
    /// Clients may override this to avoid resolving the short id. The fallback
    /// preserves compatibility with transports that only support short lookups.
    async fn load_block_exact(&mut self, id: BlockId) -> Result<RawBlock, SourceError> {
        self.load_block(id.into()).await
    }

    /// Downloads multiple blocks whose full ids are already known.
    ///
    /// The default implementation is sequential. Network clients may override
    /// it to issue independent requests concurrently while preserving input
    /// order in the returned blocks.
    async fn load_blocks_exact(&mut self, ids: &[BlockId]) -> Result<Vec<RawBlock>, SourceError> {
        let mut blocks = Vec::with_capacity(ids.len());
        for &id in ids {
            blocks.push(self.load_block_exact(id).await?);
        }
        Ok(blocks)
    }

    /// Reads the shard frontier committed by a masterchain block.
    async fn shard_frontier(&mut self, mc_block: &RawBlock) -> Result<Vec<BlockId>, SourceError>;

    /// Reads the direct predecessor ids committed inside a block.
    async fn predecessors(&mut self, block: &RawBlock) -> Result<Vec<BlockId>, SourceError>;

    /// Decodes an exact raw block after the traversal has made it canonical.
    async fn decode_block(&mut self, block: RawBlock) -> Result<BlockData, SourceError> {
        Ok(BlockData::decode(block.id, &block.boc)?)
    }
}

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

/// Direct ADNL/LiteAPI client backed by `tonutils`.
pub struct TonutilsLiteClient {
    inner: LiteClient,
    exact_clients: Vec<LiteClient>,
    decoded: HashMap<BlockId, BlockData>,
    request_stats: LiteRequestStats,
}

impl TonutilsLiteClient {
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
    const WORKER_CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
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

    /// Connects with a bounded number of clients for concurrent exact block loads.
    ///
    /// Values above 16 are capped to protect public liteservers. Zero is treated
    /// as one client.
    ///
    /// # Errors
    ///
    /// Returns an error when the config has no liteservers or none of them
    /// accepts an ADNL connection and answers a `getMasterchainInfo` probe.
    pub async fn connect_with_parallelism(
        config: &ConfigGlobal,
        parallelism: usize,
    ) -> Result<Self, SourceError> {
        if config.liteservers.is_empty() {
            return Err(SourceError::GlobalConfig(
                "network config has no liteservers".into(),
            ));
        }

        let mut failures = Vec::with_capacity(config.liteservers.len());
        let mut request_stats = LiteRequestStats::default();
        let mut attempts = FuturesUnordered::new();
        for (index, liteserver) in config.liteservers.iter().enumerate() {
            attempts.push(async move {
                let (probed, result) = connect_liteserver(liteserver, Self::CONNECT_TIMEOUT).await;
                (index, liteserver.clone(), probed, result)
            });
        }

        let mut selected = None;
        while let Some((index, liteserver, probed, result)) = attempts.next().await {
            request_stats.get_masterchain_info += u64::from(probed);
            match result {
                Ok(client) => {
                    selected = Some((liteserver, client));
                    break;
                }
                Err(error) => failures.push(format!("#{index}: {error}")),
            }
        }
        drop(attempts);

        if let Some((liteserver, inner)) = selected {
            let parallelism = parallelism.clamp(1, Self::MAX_PARALLEL_CLIENTS);
            let worker_attempts = (1..parallelism).map(|_| async {
                connect_liteserver(&liteserver, Self::WORKER_CONNECT_TIMEOUT).await
            });
            let mut exact_clients = Vec::with_capacity(parallelism - 1);
            for (probed, result) in join_all(worker_attempts).await {
                request_stats.get_masterchain_info += u64::from(probed);
                if let Ok(client) = result {
                    exact_clients.push(client);
                }
            }
            return Ok(Self {
                inner,
                exact_clients,
                decoded: HashMap::new(),
                request_stats,
            });
        }

        Err(SourceError::LiteApi(format!(
            "none of {} configured liteservers is responsive ({})",
            config.liteservers.len(),
            failures.join("; ")
        )))
    }

    /// Reads a global config and connects to its first responsive liteserver.
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
        let path = path.as_ref();
        let source = tokio::fs::read_to_string(path).await.map_err(|error| {
            SourceError::GlobalConfig(format!("failed to read {}: {error}", path.display()))
        })?;
        let config = source.parse::<ConfigGlobal>().map_err(|error| {
            SourceError::GlobalConfig(format!("failed to parse {}: {error}", path.display()))
        })?;
        Self::connect_with_parallelism(&config, parallelism).await
    }

    /// Returns the latest masterchain id without constructing a source.
    ///
    /// # Errors
    ///
    /// Returns an error when the `LiteAPI` request or id conversion fails.
    pub async fn latest(&mut self) -> Result<BlockId, SourceError> {
        self.latest_masterchain_block().await
    }

    /// Returns a snapshot of the requests issued by this client.
    #[must_use]
    pub const fn request_stats(&self) -> LiteRequestStats {
        self.request_stats
    }

    /// Returns the maximum number of exact block downloads issued concurrently.
    #[must_use]
    pub const fn exact_block_parallelism(&self) -> usize {
        1 + self.exact_clients.len()
    }

    fn decode_cached(&mut self, raw: &RawBlock) -> Result<&BlockData, SourceError> {
        Ok(match self.decoded.entry(raw.id) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(BlockData::decode(raw.id, &raw.boc)?)
            }
        })
    }
}

async fn connect_liteserver(
    liteserver: &ConfigLiteServer,
    connect_timeout: Duration,
) -> (bool, Result<LiteClient, String>) {
    let connection = LiteClient::connect_with_timeout(
        liteserver.socket_addr(),
        liteserver.public_key(),
        connect_timeout,
    )
    .await;
    let mut client = match connection {
        Ok(client) => client.with_request_timeout(TonutilsLiteClient::REQUEST_TIMEOUT),
        Err(error) => return (false, Err(format!("connect failed: {error}"))),
    };

    match client.get_masterchain_info().await {
        Ok(_) => (true, Ok(client)),
        Err(error) => (true, Err(format!("probe failed: {error}"))),
    }
}

#[async_trait]
impl BlockGraphClient for TonutilsLiteClient {
    async fn latest_masterchain_block(&mut self) -> Result<BlockId, SourceError> {
        self.request_stats.get_masterchain_info += 1;
        let info = self
            .inner
            .get_masterchain_info()
            .await
            .map_err(|error| SourceError::LiteApi(error.to_string()))?;
        from_lite_block_id(&info.last)
    }

    async fn load_block(&mut self, id: BlockIdShort) -> Result<RawBlock, SourceError> {
        let lite_id = to_lite_short_id(id)?;
        self.request_stats.lookup_block += 1;
        let header = self
            .inner
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
            .map_err(|error| SourceError::LiteApi(error.to_string()))?;
        let full_id = from_lite_block_id(&header.id)?;
        self.request_stats.get_block += 1;
        let boc = self
            .inner
            .get_block(header.id)
            .await
            .map_err(|error| SourceError::LiteApi(error.to_string()))?;
        Ok(RawBlock::new(full_id, boc))
    }

    async fn load_block_exact(&mut self, id: BlockId) -> Result<RawBlock, SourceError> {
        self.load_blocks_exact(&[id])
            .await?
            .pop()
            .ok_or_else(|| SourceError::InvalidBatch("exact block load returned no block".into()))
    }

    async fn load_blocks_exact(&mut self, ids: &[BlockId]) -> Result<Vec<RawBlock>, SourceError> {
        let request_count = u64::try_from(ids.len()).unwrap_or(u64::MAX);
        self.request_stats.get_block = self.request_stats.get_block.saturating_add(request_count);

        let mut blocks = Vec::with_capacity(ids.len());
        let parallelism = 1 + self.exact_clients.len();
        for ids in ids.chunks(parallelism) {
            let Some((&first, rest)) = ids.split_first() else {
                continue;
            };
            let mut requests = Vec::with_capacity(ids.len());
            requests.push(download_exact_block(&mut self.inner, first));
            requests.extend(
                self.exact_clients
                    .iter_mut()
                    .zip(rest)
                    .map(|(client, &id)| download_exact_block(client, id)),
            );

            for result in join_all(requests).await {
                let (id, boc) = result?;
                // `tonutils::get_block` returns only the response payload. Decode
                // it now to verify both hashes and all block coordinates. Caching
                // also prevents a second decode during traversal.
                self.decoded.insert(id, BlockData::decode(id, &boc)?);
                blocks.push(RawBlock::new(id, boc));
            }
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
        match self.decoded.remove(&block.id) {
            Some(decoded) => Ok(decoded),
            None => Ok(BlockData::decode(block.id, &block.boc)?),
        }
    }
}

/// Produces one canonical batch for each masterchain block.
pub struct CanonicalBlockSource<C> {
    client: C,
    start_seqno: u32,
    max_shard_blocks: usize,
    known_tip_seqno: Option<u32>,
    last_emitted_masterchain: Option<CachedMasterchainState>,
}

impl<C> CanonicalBlockSource<C> {
    /// Default maximum number of shard blocks visited for one masterchain step.
    pub const DEFAULT_MAX_SHARD_BLOCKS: usize = 10_000;

    /// Starts at the provided masterchain sequence number when no checkpoint exists.
    pub const fn new(client: C, start_seqno: u32) -> Self {
        Self {
            client,
            start_seqno,
            max_shard_blocks: Self::DEFAULT_MAX_SHARD_BLOCKS,
            known_tip_seqno: None,
            last_emitted_masterchain: None,
        }
    }

    /// Changes the per-batch shard traversal safety bound.
    #[must_use]
    pub const fn with_max_shard_blocks(mut self, limit: usize) -> Self {
        self.max_shard_blocks = limit;
        self
    }

    /// Returns a shared reference to the underlying client.
    pub const fn client(&self) -> &C {
        &self.client
    }

    /// Returns a mutable reference to the underlying client.
    pub const fn client_mut(&mut self) -> &mut C {
        &mut self.client
    }

    /// Consumes the source and returns its client.
    pub fn into_client(self) -> C {
        self.client
    }
}

#[async_trait]
impl<C> BlockSource for CanonicalBlockSource<C>
where
    C: BlockGraphClient,
{
    async fn next_batch(
        &mut self,
        after: Option<&BlockId>,
    ) -> ton_indexer_core::Result<Option<Batch>> {
        CanonicalBlockSource::next_batch(self, after)
            .await
            .map_err(IndexerError::source)
    }
}

impl<C> CanonicalBlockSource<C>
where
    C: BlockGraphClient,
{
    /// Loads, canonicalizes, and decodes the next full batch.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, decoding, checkpoint-continuity, or
    /// shard-frontier traversal failures.
    pub async fn next_batch(
        &mut self,
        after: Option<&BlockId>,
    ) -> Result<Option<Batch>, SourceError> {
        let Some(raw) = self.next_raw_batch(after).await? else {
            return Ok(None);
        };

        let masterchain = self.client.decode_block(raw.masterchain).await?;
        let mut shards = Vec::with_capacity(raw.shards.len());
        for block in raw.shards {
            shards.push(self.client.decode_block(block).await?);
        }
        Ok(Some(Batch::try_new(masterchain, shards)?))
    }

    async fn next_raw_batch(
        &mut self,
        after: Option<&BlockId>,
    ) -> Result<Option<RawBatch>, SourceError> {
        let next_seqno = match after {
            Some(checkpoint) => checkpoint.seqno.saturating_add(1),
            None => self.start_seqno,
        };
        if !self.tip_covers(next_seqno).await? {
            return Ok(None);
        }

        let mc_block = self.load_masterchain(next_seqno).await?;
        let previous = if next_seqno == 0 {
            None
        } else if let Some(cached) = self.cached_previous(after, next_seqno) {
            Some(cached)
        } else {
            let block = self.load_masterchain(next_seqno - 1).await?;
            let frontier = self.client.shard_frontier(&block).await?;
            Some(CachedMasterchainState {
                id: block.id,
                frontier,
            })
        };

        if let (Some(checkpoint), Some(previous)) = (after, previous.as_ref()) {
            Self::verify_block_id(checkpoint, previous.id)?;
        }
        if let Some(previous) = previous.as_ref() {
            self.verify_masterchain_link(previous.id, &mc_block).await?;
        }

        let previous_frontier = previous.map_or_else(Vec::new, |previous| previous.frontier);
        let current_frontier = self.client.shard_frontier(&mc_block).await?;
        let shard_blocks = self
            .collect_shard_delta(previous_frontier, current_frontier.clone())
            .await?;
        let batch = RawBatch::try_new(mc_block, shard_blocks)?;

        self.last_emitted_masterchain = Some(CachedMasterchainState {
            id: batch.masterchain.id,
            frontier: current_frontier,
        });
        Ok(Some(batch))
    }

    async fn tip_covers(&mut self, seqno: u32) -> Result<bool, SourceError> {
        if self
            .known_tip_seqno
            .is_some_and(|known_tip| seqno <= known_tip)
        {
            return Ok(true);
        }

        let tip = self.client.latest_masterchain_block().await?;
        self.known_tip_seqno = Some(tip.seqno);
        Ok(seqno <= tip.seqno)
    }

    fn cached_previous(
        &self,
        after: Option<&BlockId>,
        next_seqno: u32,
    ) -> Option<CachedMasterchainState> {
        let checkpoint = after?;
        let cached = self.last_emitted_masterchain.as_ref()?;
        if cached.id.seqno.checked_add(1) != Some(next_seqno) || cached.id != *checkpoint {
            return None;
        }
        Some(cached.clone())
    }

    async fn load_masterchain(&mut self, seqno: u32) -> Result<RawBlock, SourceError> {
        let block = self
            .client
            .load_block(BlockIdShort {
                workchain: BlockId::MASTERCHAIN_WORKCHAIN,
                shard: BlockId::FULL_SHARD,
                seqno,
            })
            .await?;
        if !block.id.is_masterchain()
            || block.id.shard != BlockId::FULL_SHARD
            || block.id.seqno != seqno
        {
            return Err(SourceError::InvalidBlockId(format!(
                "expected masterchain seqno {seqno}, got {}",
                block.id
            )));
        }
        Ok(block)
    }

    fn verify_block_id(expected: &BlockId, actual: BlockId) -> Result<(), SourceError> {
        if actual != *expected {
            return Err(SourceError::UnexpectedBlock {
                expected: Box::new(*expected),
                actual: Box::new(actual),
            });
        }
        Ok(())
    }

    async fn verify_masterchain_link(
        &mut self,
        previous: BlockId,
        next: &RawBlock,
    ) -> Result<(), SourceError> {
        let predecessors = self.client.predecessors(next).await?;
        if predecessors.as_slice() != [previous] {
            return Err(SourceError::MasterchainDiscontinuity {
                previous: Box::new(previous),
                next: Box::new(next.id),
                actual: predecessors,
            });
        }
        Ok(())
    }

    async fn collect_shard_delta(
        &mut self,
        previous_frontier: Vec<BlockId>,
        mut current_frontier: Vec<BlockId>,
    ) -> Result<Vec<RawBlock>, SourceError> {
        current_frontier.sort_unstable();
        let stop = previous_frontier.into_iter().collect::<HashSet<_>>();

        if stop.is_empty() {
            if current_frontier.len() > self.max_shard_blocks {
                return Err(SourceError::TraversalLimit {
                    limit: self.max_shard_blocks,
                });
            }
            let blocks = self.client.load_blocks_exact(&current_frontier).await?;
            if blocks.len() != current_frontier.len() {
                return Err(SourceError::InvalidBatch(format!(
                    "requested {} exact shard blocks, transport returned {}",
                    current_frontier.len(),
                    blocks.len()
                )));
            }
            for (&expected, block) in current_frontier.iter().zip(&blocks) {
                Self::verify_block_id(&expected, block.id)?;
            }
            return Ok(blocks);
        }

        let roots = current_frontier.clone();
        let mut pending = current_frontier;
        let mut discovered = HashSet::new();
        let mut reached = HashSet::new();
        let mut loaded = HashMap::<BlockId, (RawBlock, Vec<BlockId>)>::new();

        while !pending.is_empty() {
            pending.sort_unstable();
            pending.dedup();
            let mut wave = Vec::with_capacity(pending.len());
            for id in std::mem::take(&mut pending) {
                if stop.contains(&id) {
                    reached.insert(id);
                } else if discovered.insert(id) {
                    wave.push(id);
                }
            }
            if discovered.len() > self.max_shard_blocks {
                return Err(SourceError::TraversalLimit {
                    limit: self.max_shard_blocks,
                });
            }
            if wave.is_empty() {
                break;
            }

            let blocks = self.client.load_blocks_exact(&wave).await?;
            if blocks.len() != wave.len() {
                return Err(SourceError::InvalidBatch(format!(
                    "requested {} exact shard blocks, transport returned {}",
                    wave.len(),
                    blocks.len()
                )));
            }
            for (&expected, block) in wave.iter().zip(blocks) {
                Self::verify_block_id(&expected, block.id)?;
                let mut predecessors = self.client.predecessors(&block).await?;
                predecessors.sort_unstable();
                pending.extend(predecessors.iter().copied());
                loaded.insert(expected, (block, predecessors));
            }
        }

        if reached != stop {
            let mut missing = stop.difference(&reached).copied().collect::<Vec<_>>();
            missing.sort_unstable();
            return Err(SourceError::DisconnectedFrontier { missing });
        }

        // Network discovery runs breadth-first to expose parallel requests. Build
        // the result afterwards in the same deterministic predecessor-first order
        // as the former depth-first traversal.
        let mut frames = roots
            .into_iter()
            .rev()
            .map(TraversalFrame::Enter)
            .collect::<Vec<_>>();
        let mut ordered = HashSet::new();
        let mut blocks = Vec::with_capacity(loaded.len());
        while let Some(frame) = frames.pop() {
            match frame {
                TraversalFrame::Enter(id) if stop.contains(&id) => {}
                TraversalFrame::Enter(id) if !ordered.insert(id) => {}
                TraversalFrame::Enter(id) => {
                    let (_, predecessors) = loaded.get(&id).ok_or_else(|| {
                        SourceError::InvalidBatch(format!(
                            "shard traversal did not load discovered block {id}"
                        ))
                    })?;
                    frames.push(TraversalFrame::Exit(id));
                    frames.extend(
                        predecessors
                            .iter()
                            .rev()
                            .copied()
                            .map(TraversalFrame::Enter),
                    );
                }
                TraversalFrame::Exit(id) => {
                    let (block, _) = loaded.remove(&id).ok_or_else(|| {
                        SourceError::InvalidBatch(format!(
                            "shard traversal emitted missing block {id}"
                        ))
                    })?;
                    blocks.push(block);
                }
            }
        }
        Ok(blocks)
    }
}

#[derive(Debug)]
struct RawBatch {
    masterchain: RawBlock,
    shards: Vec<RawBlock>,
}

impl RawBatch {
    fn try_new(masterchain: RawBlock, shards: Vec<RawBlock>) -> Result<Self, SourceError> {
        if !masterchain.id.is_masterchain() || masterchain.id.shard != BlockId::FULL_SHARD {
            return Err(SourceError::InvalidBatch(format!(
                "{} is not a masterchain block",
                masterchain.id
            )));
        }

        let mut ids = HashSet::with_capacity(shards.len());
        for block in &shards {
            if block.id.is_masterchain() {
                return Err(SourceError::InvalidBatch(format!(
                    "masterchain block {} appeared in the shard delta",
                    block.id
                )));
            }
            if !ids.insert(block.id) {
                return Err(SourceError::InvalidBatch(format!(
                    "duplicate shard block {}",
                    block.id
                )));
            }
        }
        Ok(Self {
            masterchain,
            shards,
        })
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

async fn download_exact_block(
    client: &mut LiteClient,
    id: BlockId,
) -> Result<(BlockId, Vec<u8>), SourceError> {
    let boc = client
        .get_block(to_lite_block_id_ext(id)?)
        .await
        .map_err(|error| SourceError::LiteApi(error.to_string()))?;
    Ok((id, boc))
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

enum TraversalFrame {
    Enter(BlockId),
    Exit(BlockId),
}

#[derive(Clone)]
struct CachedMasterchainState {
    id: BlockId,
    frontier: Vec<BlockId>,
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[derive(Default)]
    struct FakeGraph {
        tip: Option<BlockId>,
        latest_calls: usize,
        blocks: HashMap<BlockIdShort, RawBlock>,
        load_calls: HashMap<BlockIdShort, usize>,
        exact_load_calls: HashMap<BlockId, usize>,
        exact_load_batches: Vec<Vec<BlockId>>,
        frontiers: HashMap<BlockId, Vec<BlockId>>,
        predecessors: HashMap<BlockId, Vec<BlockId>>,
    }

    impl FakeGraph {
        fn insert(&mut self, id: BlockId) {
            self.blocks.insert(id.into(), RawBlock::new(id, []));
        }
    }

    #[async_trait]
    impl BlockGraphClient for FakeGraph {
        async fn latest_masterchain_block(&mut self) -> Result<BlockId, SourceError> {
            self.latest_calls += 1;
            self.tip
                .ok_or_else(|| SourceError::LiteApi("fake tip is missing".into()))
        }

        async fn load_block(&mut self, id: BlockIdShort) -> Result<RawBlock, SourceError> {
            *self.load_calls.entry(id).or_default() += 1;
            self.blocks
                .get(&id)
                .cloned()
                .ok_or_else(|| SourceError::LiteApi(format!("fake block {id:?} is missing")))
        }

        async fn load_block_exact(&mut self, id: BlockId) -> Result<RawBlock, SourceError> {
            *self.exact_load_calls.entry(id).or_default() += 1;
            self.load_block(id.into()).await
        }

        async fn load_blocks_exact(
            &mut self,
            ids: &[BlockId],
        ) -> Result<Vec<RawBlock>, SourceError> {
            self.exact_load_batches.push(ids.to_vec());
            let mut blocks = Vec::with_capacity(ids.len());
            for &id in ids {
                blocks.push(self.load_block_exact(id).await?);
            }
            Ok(blocks)
        }

        async fn shard_frontier(
            &mut self,
            mc_block: &RawBlock,
        ) -> Result<Vec<BlockId>, SourceError> {
            Ok(self
                .frontiers
                .get(&mc_block.id)
                .cloned()
                .unwrap_or_default())
        }

        async fn predecessors(&mut self, block: &RawBlock) -> Result<Vec<BlockId>, SourceError> {
            Ok(self
                .predecessors
                .get(&block.id)
                .cloned()
                .unwrap_or_default())
        }
    }

    fn id(workchain: i32, shard: u64, seqno: u32, marker: u8) -> BlockId {
        BlockId {
            workchain,
            shard,
            seqno,
            root_hash: Hash256::new([marker; 32]),
            file_hash: Hash256::new([marker.wrapping_add(1); 32]),
        }
    }

    fn graph_with_masterchain() -> (FakeGraph, BlockId, BlockId) {
        let previous_mc = id(-1, BlockId::FULL_SHARD, 9, 90);
        let current_mc = id(-1, BlockId::FULL_SHARD, 10, 100);
        let mut graph = FakeGraph {
            tip: Some(current_mc),
            ..FakeGraph::default()
        };
        graph.insert(previous_mc);
        graph.insert(current_mc);
        graph.predecessors.insert(current_mc, vec![previous_mc]);
        (graph, previous_mc, current_mc)
    }

    #[tokio::test]
    async fn rejects_config_without_liteservers() {
        let config = r#"{"liteservers":[]}"#.parse::<ConfigGlobal>().unwrap();
        let error = TonutilsLiteClient::connect(&config).await.err().unwrap();
        assert!(matches!(error, SourceError::GlobalConfig(_)));
    }

    #[tokio::test]
    async fn reuses_tip_and_checkpoint_confirmed_previous_frontier() {
        let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
        let next_mc = id(-1, BlockId::FULL_SHARD, 11, 110);
        graph.tip = Some(next_mc);
        graph.insert(next_mc);
        graph.predecessors.insert(next_mc, vec![current_mc]);

        let mut source = CanonicalBlockSource::new(graph, current_mc.seqno);
        let current = source.next_raw_batch(None).await.unwrap().unwrap();
        let next = source
            .next_raw_batch(Some(&current.masterchain.id))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(next.masterchain.id, next_mc);
        assert_eq!(source.client().latest_calls, 1);
        assert_eq!(
            source.client().load_calls,
            HashMap::from([
                (previous_mc.into(), 1),
                (current_mc.into(), 1),
                (next_mc.into(), 1),
            ])
        );
    }

    #[tokio::test]
    async fn does_not_reuse_cached_frontier_for_mismatching_checkpoint() {
        let (mut graph, _, current_mc) = graph_with_masterchain();
        let next_mc = id(-1, BlockId::FULL_SHARD, 11, 110);
        graph.tip = Some(next_mc);
        graph.insert(next_mc);
        graph.predecessors.insert(next_mc, vec![current_mc]);

        let mut source = CanonicalBlockSource::new(graph, current_mc.seqno);
        source.next_raw_batch(None).await.unwrap().unwrap();
        let invalid_checkpoint = BlockId {
            workchain: current_mc.workchain,
            shard: current_mc.shard,
            seqno: current_mc.seqno,
            root_hash: Hash256::new([0xee; 32]),
            file_hash: current_mc.file_hash,
        };

        let error = source
            .next_raw_batch(Some(&invalid_checkpoint))
            .await
            .unwrap_err();
        assert!(matches!(error, SourceError::UnexpectedBlock { .. }));
        assert_eq!(source.client().load_calls.get(&current_mc.into()), Some(&2));
    }

    #[tokio::test]
    async fn refreshes_tip_after_reaching_cached_boundary() {
        let (graph, _, current_mc) = graph_with_masterchain();
        let mut source = CanonicalBlockSource::new(graph, current_mc.seqno);
        let current = source.next_raw_batch(None).await.unwrap().unwrap();

        assert!(
            source
                .next_raw_batch(Some(&current.masterchain.id))
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(source.client().latest_calls, 2);
    }

    #[tokio::test]
    async fn walks_normal_chain_predecessor_first() {
        let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
        let one = id(0, BlockId::FULL_SHARD, 1, 1);
        let two = id(0, BlockId::FULL_SHARD, 2, 2);
        let three = id(0, BlockId::FULL_SHARD, 3, 3);
        for block in [one, two, three] {
            graph.insert(block);
        }
        graph.frontiers.insert(previous_mc, vec![one]);
        graph.frontiers.insert(current_mc, vec![three]);
        graph.predecessors.insert(two, vec![one]);
        graph.predecessors.insert(three, vec![two]);

        let mut source = CanonicalBlockSource::new(graph, 10);
        let batch = source.next_raw_batch(None).await.unwrap().unwrap();
        assert_eq!(
            batch
                .shards
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            vec![two, three]
        );
        assert_eq!(
            source.client().exact_load_calls,
            HashMap::from([(two, 1), (three, 1)])
        );
        assert_eq!(
            source.client().exact_load_batches,
            vec![vec![three], vec![two]]
        );
    }

    #[tokio::test]
    async fn handles_split_without_duplicating_parent() {
        let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
        let parent = id(0, BlockId::FULL_SHARD, 1, 1);
        let (left_shard, right_shard) =
            ton_indexer_core::tycho_types::models::ShardIdent::new(0, BlockId::FULL_SHARD)
                .unwrap()
                .split()
                .unwrap();
        let left = id(0, left_shard.prefix(), 2, 2);
        let right = id(0, right_shard.prefix(), 2, 3);
        for block in [parent, left, right] {
            graph.insert(block);
        }
        graph.frontiers.insert(previous_mc, vec![parent]);
        graph.frontiers.insert(current_mc, vec![right, left]);
        graph.predecessors.insert(left, vec![parent]);
        graph.predecessors.insert(right, vec![parent]);

        let mut source = CanonicalBlockSource::new(graph, 10);
        let batch = source.next_raw_batch(None).await.unwrap().unwrap();
        let ids = batch
            .shards
            .iter()
            .map(|block| block.id)
            .collect::<HashSet<_>>();
        assert_eq!(ids, HashSet::from([left, right]));
        assert_eq!(source.client().exact_load_batches, vec![vec![left, right]]);
    }

    #[tokio::test]
    async fn handles_merge_from_two_frontier_blocks() {
        let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
        let parent_shard =
            ton_indexer_core::tycho_types::models::ShardIdent::new(0, BlockId::FULL_SHARD).unwrap();
        let (left_shard, right_shard) = parent_shard.split().unwrap();
        let left = id(0, left_shard.prefix(), 2, 2);
        let right = id(0, right_shard.prefix(), 2, 3);
        let merged = id(0, BlockId::FULL_SHARD, 3, 4);
        for block in [left, right, merged] {
            graph.insert(block);
        }
        graph.frontiers.insert(previous_mc, vec![left, right]);
        graph.frontiers.insert(current_mc, vec![merged]);
        graph.predecessors.insert(merged, vec![right, left]);

        let batch = CanonicalBlockSource::new(graph, 10)
            .next_raw_batch(None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            batch
                .shards
                .iter()
                .map(|block| block.id)
                .collect::<Vec<_>>(),
            vec![merged]
        );
    }

    #[tokio::test]
    async fn rejects_disconnected_midchain_bootstrap() {
        let (mut graph, previous_mc, current_mc) = graph_with_masterchain();
        let unrelated = id(-1, BlockId::FULL_SHARD, 9, 91);
        graph.predecessors.insert(current_mc, vec![unrelated]);

        let error = CanonicalBlockSource::new(graph, 10)
            .next_raw_batch(None)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SourceError::MasterchainDiscontinuity { previous, next, .. }
                if *previous == previous_mc && *next == current_mc
        ));
    }

    #[tokio::test]
    async fn rejects_loaded_predecessor_that_differs_from_checkpoint() {
        let (graph, previous_mc, _) = graph_with_masterchain();
        let checkpoint = BlockId {
            workchain: previous_mc.workchain,
            shard: previous_mc.shard,
            seqno: previous_mc.seqno,
            root_hash: Hash256::new([0xee; 32]),
            file_hash: previous_mc.file_hash,
        };

        let error = CanonicalBlockSource::new(graph, 0)
            .next_raw_batch(Some(&checkpoint))
            .await
            .unwrap_err();
        assert!(matches!(error, SourceError::UnexpectedBlock { .. }));
    }
}
