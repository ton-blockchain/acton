use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use rocksdb::DB;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, Lazy, Load};
use rston::models::{
    Block, BlockId, BlockInfo, BlockchainConfig, PrevBlockRef, ShardAccount, ShardDescription,
    ShardIdent, ShardStateSplit, ShardStateUnsplit, StdAddr,
};
use sha2::{Digest, Sha256};

use crate::lazy::Reader;
use crate::{ReadStats, StateRecord};

/// An account and the exact masterchain/shard blocks that locate it.
/// `None` means the account dictionary has no entry for the requested address.
pub struct AccountSnapshot {
    pub masterchain_block: BlockId,
    pub shard_block: BlockId,
    /// Generation time of the account's shard state, in Unix seconds.
    /// This can precede the masterchain checkpoint's time.
    pub gen_utime: u32,
    pub account: Option<ShardAccount>,
    pub reads: ReadStats,
}

/// Owned network configuration and block references for off-chain TVM execution.
/// All fields belong to the same masterchain state; no lazy database cells escape.
pub struct MasterchainContext {
    /// Complete configuration dictionary, including the global TVM version.
    pub config: BlockchainConfig,
    /// Current block followed by up to fifteen predecessors, newest first.
    pub last_mc_blocks: Vec<BlockId>,
    /// Includes the current block when the state follows a key block.
    pub last_key_block: BlockId,
    /// Up to sixteen blocks at multiples of 100, newest first, for TVM 9+.
    pub last_mc_blocks_100: Vec<BlockId>,
}

/// A retained shard state with cells fetched only when traversed.
///
/// Lazy references never escape the view: returned accounts are fully owned.
/// An I/O or cell-integrity error poisons the view; open another to retry.
pub struct StateView {
    id: BlockId,
    pub(crate) root: Cell,
    pub(crate) reader: Arc<Reader>,
}

impl StateView {
    pub(crate) fn open(db: Arc<DB>, record: StateRecord, limit: usize) -> Result<Self> {
        let reader = Reader::new(db, limit);
        Self::load(reader, record.block_id, record.root_hash)
    }

    pub(crate) fn load(
        reader: Arc<Reader>,
        id: BlockId,
        hash: rston::cell::HashBytes,
    ) -> Result<Self> {
        let root = reader.load(hash)?;
        reader.run(|| validate_state(&root, &id))?;

        Ok(Self { id, root, reader })
    }

    /// Identifies the state, including successfully applied in-memory updates.
    #[must_use]
    pub const fn block_id(&self) -> BlockId {
        self.id
    }

    /// Hash of this state root, distinct from the block's root hash.
    #[must_use]
    pub fn root_hash(&self) -> rston::cell::HashBytes {
        *self.root.repr_hash()
    }

    /// Block generation time from the state header, in Unix seconds.
    /// This is chain time, not the time the state was downloaded or opened.
    pub fn gen_utime(&self) -> Result<u32> {
        self.reader
            .run(|| Ok(self.root.parse::<ShardStateUnsplit>()?.gen_utime))
    }

    /// Logical time at the end of this state. Off-chain execution must start
    /// after this frontier as well as after the account's previous transaction.
    pub fn gen_lt(&self) -> Result<u64> {
        self.reader
            .run(|| Ok(self.root.parse::<ShardStateUnsplit>()?.gen_lt))
    }

    /// Reports cumulative database reads for this view. Reusing loaded cell
    /// references does not increment the counters.
    #[must_use]
    pub fn read_stats(&self) -> ReadStats {
        self.reader.stats()
    }

    /// Reads config parameter 1 in a masterchain state. Custom networks can
    /// use an elector address different from the public networks.
    pub fn elector_address(&self) -> Result<StdAddr> {
        self.reader.run(|| {
            ensure!(
                self.id.shard.is_masterchain(),
                "elector config requires masterchain state"
            );
            let extra = self
                .root
                .parse::<ShardStateUnsplit>()?
                .load_custom()?
                .context("missing masterchain state extra")?;

            Ok(StdAddr::new(-1, extra.config.get_elector_address()?))
        })
    }

    /// Materializes configuration and the block windows used by `PREVBLOCKS*`.
    /// Reads only the required paths in the previous-block dictionary. Missing
    /// history is an error, rather than an invented execution context.
    pub fn execution_context(&self) -> Result<MasterchainContext> {
        self.reader.run(|| {
            ensure!(
                self.id.shard.is_masterchain(),
                "execution context requires masterchain state"
            );
            let extra = self
                .root
                .parse::<ShardStateUnsplit>()?
                .load_custom()?
                .context("missing masterchain state extra")?;
            let block_at = |seqno: u32| -> Result<BlockId> {
                if seqno == self.id.seqno {
                    return Ok(self.id);
                }
                let (_, entry) = extra
                    .prev_blocks
                    .get(seqno)?
                    .with_context(|| format!("missing previous masterchain block {seqno}"))?;
                Ok(entry.block_ref.as_block_id(ShardIdent::MASTERCHAIN))
            };
            let mut last_mc_blocks = Vec::new();
            for offset in 0..16 {
                let Some(seqno) = self.id.seqno.checked_sub(offset) else {
                    break;
                };
                last_mc_blocks.push(block_at(seqno)?);
            }
            let last_key_block = if extra.after_key_block || self.id.seqno == 0 {
                self.id
            } else if let Some(block) = extra.last_key_block {
                block.as_block_id(ShardIdent::MASTERCHAIN)
            } else {
                block_at(0)?
            };
            let mut last_mc_blocks_100 = Vec::new();
            if extra.config.get_global_version()?.version >= 9 {
                let start = self.id.seqno / 100 * 100;
                for offset in 0..16 {
                    let Some(seqno) = start.checked_sub(offset * 100) else {
                        break;
                    };
                    last_mc_blocks_100.push(block_at(seqno)?);
                }
            }
            let config = self
                .materialize(&CellBuilder::build_from(extra.config)?)?
                .parse()?;
            Ok(MasterchainContext {
                config,
                last_mc_blocks,
                last_key_block,
                last_mc_blocks_100,
            })
        })
    }

    /// Resolves a published library at this checkpoint and returns owned cells.
    /// An unpublished hash returns `None`; database failures remain errors.
    pub fn get_library(&self, hash: &rston::cell::HashBytes) -> Result<Option<Cell>> {
        self.reader.run(|| {
            ensure!(
                self.id.shard.is_masterchain(),
                "libraries require masterchain state"
            );
            let state = self.root.parse::<ShardStateUnsplit>()?;
            let Some(library) = state.libraries.get(hash)? else {
                return Ok(None);
            };
            ensure!(library.lib.repr_hash() == hash, "library hash mismatch");
            Ok(Some(self.materialize(&library.lib)?))
        })
    }

    /// Selects the shard containing this address from the masterchain's own
    /// frontier. Traverses only the requested workchain and address prefix.
    pub fn account_shard(&self, address: &StdAddr) -> Result<BlockId> {
        self.reader.run(|| {
            ensure!(
                address.anycast.is_none(),
                "anycast addresses are not supported"
            );
            ensure!(
                self.id.shard.is_masterchain(),
                "shard selection requires masterchain state"
            );
            if address.workchain == -1 {
                return Ok(self.id);
            }

            let extra = self
                .root
                .parse::<ShardStateUnsplit>()?
                .load_custom()?
                .context("missing masterchain state extra")?;
            let workchain = i32::from(address.workchain);
            let mut tree = extra
                .shards
                .as_dict()
                .get(workchain)?
                .context("workchain is absent from the masterchain shard frontier")?;
            let mut shard = ShardIdent::new(workchain, 1_u64 << 63).context("invalid workchain")?;

            loop {
                let mut slice = tree.as_slice()?;
                if !slice.load_bit()? {
                    let description = ShardDescription::load_from(&mut slice)?;
                    return Ok(BlockId {
                        shard,
                        seqno: description.seqno,
                        root_hash: description.root_hash,
                        file_hash: description.file_hash,
                    });
                }

                let (left, right) = shard.split().context("shard tree exceeds maximum depth")?;
                let branch = u8::from(!left.contains_address(address));
                shard = if branch == 0 { left } else { right };
                tree = slice.get_reference_cloned(branch)?;
            }
        })
    }

    /// Returns the exact shard frontier whose states complete this masterchain
    /// checkpoint. A downloader must also fetch intermediate shard blocks.
    pub fn shard_blocks(&self) -> Result<Vec<BlockId>> {
        self.reader.run(|| {
            ensure!(
                self.id.shard.is_masterchain(),
                "shard frontier requires masterchain state"
            );
            let extra = self
                .root
                .parse::<ShardStateUnsplit>()?
                .load_custom()?
                .context("missing masterchain state extra")?;

            Ok(extra
                .shards
                .latest_blocks()
                .collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Reads one dictionary path and materializes only that account's cells.
    /// The address must belong to this view's shard. No entry yields `None`;
    /// malformed state and missing database cells yield errors instead.
    pub fn get_account(&self, address: &StdAddr) -> Result<Option<ShardAccount>> {
        self.reader.run(|| {
            ensure!(
                address.anycast.is_none(),
                "anycast addresses are not supported"
            );
            ensure!(
                self.id.shard.contains_address(address),
                "address is outside this state's shard"
            );
            let accounts = self.root.parse::<ShardStateUnsplit>()?.load_accounts()?;
            let Some((_, mut account)) = accounts.get(address.address)? else {
                return Ok(None);
            };

            let owned = self.materialize(account.account.inner())?;
            account.account = Lazy::from_raw(owned)?;
            account.load_account().context("invalid account TL-B")?;

            Ok(Some(account))
        })
    }

    /// Copies the selected subgraph into ordinary cells. Iterative postorder
    /// traversal bounds stack use and shares repeated code/data references.
    fn materialize(&self, root: &Cell) -> Result<Cell> {
        let mut loaded = HashMap::new();
        let mut pending = vec![(root.clone(), false)];

        while let Some((cell, expanded)) = pending.pop() {
            let hash = *cell.repr_hash();
            if loaded.contains_key(&hash) {
                continue;
            }
            let descriptor = cell.descriptor();
            self.reader.check()?;

            if !expanded {
                pending.push((cell.clone(), true));
                for index in 0..descriptor.reference_count() {
                    let child = cell
                        .reference_cloned(index)
                        .context("missing account cell reference")?;
                    pending.push((child, false));
                }
                continue;
            }

            let mut builder = CellBuilder::new();
            builder.set_exotic(descriptor.is_exotic());
            builder.store_raw(cell.data(), cell.bit_len())?;
            for index in 0..descriptor.reference_count() {
                let child = cell
                    .reference(index)
                    .context("missing account cell reference")?;
                let owned: &Cell = loaded
                    .get(child.repr_hash())
                    .context("account child was not loaded")?;
                builder.store_reference(owned.clone())?;
            }
            let owned = builder.build()?;
            ensure!(
                owned.repr_hash() == &hash,
                "materialized account cell hash mismatch"
            );
            loaded.insert(hash, owned);
        }

        loaded
            .remove(root.repr_hash())
            .context("account root was not loaded")
    }

    /// Applies an adjacent masterchain block's Merkle update in memory.
    /// Checks the full block ID, predecessor and old/new state hashes. This
    /// does not verify validator signatures or execute transactions in the TVM.
    /// The database stays read-only; the caller owns persistence and trust.
    pub fn apply_masterchain_block(&mut self, id: &BlockId, bytes: &[u8]) -> Result<()> {
        ensure!(self.id.shard.is_masterchain(), "expected masterchain state");
        self.apply_block(id, bytes)
    }

    /// Applies a block with one predecessor, including a child after a split.
    /// Merge blocks require both predecessor states; use `StateStore::apply_batch`.
    /// Changes remain in memory; block authenticity is the caller's responsibility.
    pub fn apply_block(&mut self, id: &BlockId, bytes: &[u8]) -> Result<()> {
        *self = StateUpdate::parse(*id, bytes)?.apply(&[self], None)?;
        Ok(())
    }
}

/// A checked block and its exact state dependencies. Parsing precedes database
/// lookup so split/merge ancestry can select the required retained states.
pub(crate) struct StateUpdate {
    pub(crate) id: BlockId,
    block: Block,
    info: BlockInfo,
    pub(crate) predecessors: Vec<BlockId>,
}

impl StateUpdate {
    pub(crate) fn parse(id: BlockId, bytes: &[u8]) -> Result<Self> {
        ensure!(
            Sha256::digest(bytes)[..] == id.file_hash.0,
            "block file hash mismatch"
        );
        let cell = Boc::decode(bytes)?;
        Self::from_root(id, &cell)
    }

    pub(crate) fn from_root(id: BlockId, cell: &Cell) -> Result<Self> {
        ensure!(
            cell.repr_hash() == &id.root_hash,
            "block root hash mismatch"
        );
        let block = cell.parse::<Block>()?;
        let info = block.load_info()?;
        ensure!(
            info.shard == id.shard && info.seqno == id.seqno,
            "block header mismatch"
        );
        ensure!(
            !(info.after_split && info.after_merge),
            "block is both after split and after merge"
        );
        ensure!(
            !id.shard.is_masterchain()
                || !(info.after_split || info.after_merge || info.before_split),
            "masterchain cannot split or merge"
        );

        let predecessors = match info.load_prev_ref()? {
            PrevBlockRef::Single(previous) => {
                let shard = if info.after_split {
                    id.shard
                        .merge()
                        .context("split block has no parent shard")?
                } else {
                    id.shard
                };
                vec![previous.as_block_id(shard)]
            }
            PrevBlockRef::AfterMerge { left, right } => {
                let (left_shard, right_shard) = id
                    .shard
                    .split()
                    .context("merge block has no child shards")?;
                vec![left.as_block_id(left_shard), right.as_block_id(right_shard)]
            }
        };
        ensure!(
            predecessors
                .iter()
                .map(|id| id.seqno)
                .max()
                .and_then(|seqno| seqno.checked_add(1))
                == Some(id.seqno),
            "block sequence number does not follow its predecessors"
        );

        Ok(Self {
            id,
            block,
            info,
            predecessors,
        })
    }

    /// All inputs share a reader so deferred errors in either merge branch are
    /// reported by the resulting view. No previous state is mutated.
    pub(crate) fn apply(
        &self,
        previous: &[&StateView],
        workers: Option<&rayon::ThreadPool>,
    ) -> Result<StateView> {
        ensure!(
            previous
                .iter()
                .map(|state| state.id)
                .eq(self.predecessors.iter().copied()),
            "block predecessor mismatch"
        );
        let reader = &previous[0].reader;
        ensure!(
            previous
                .iter()
                .all(|state| Arc::ptr_eq(reader, &state.reader)),
            "predecessor states must share a database reader"
        );

        reader.run(|| {
            for state in previous {
                ensure!(
                    state.root.parse::<ShardStateUnsplit>()?.before_split == self.info.after_split,
                    "predecessor before_split flag does not match block transition"
                );
            }

            let old = if self.info.after_merge {
                // TON hashes the ordered pair of child states as the old state
                // of a merge block. Their dictionaries stay separate and lazy.
                CellBuilder::build_from(ShardStateSplit {
                    left: Lazy::from_raw(previous[0].root.clone())?,
                    right: Lazy::from_raw(previous[1].root.clone())?,
                })?
            } else {
                // Both children after a split reference the original parent
                // state; each block's Merkle update produces its own subtree.
                previous[0].root.clone()
            };
            let update = self.block.load_state_update()?;
            let root = if let Some(workers) = workers {
                // Partition the proof near its root, without reading the old
                // database tree. Independent branches can then load cells in
                // parallel while rston checks the full Merkle update.
                let mut frontier = vec![update.old.as_ref()];
                for _ in 0..6 {
                    let next = frontier
                        .iter()
                        .flat_map(|cell| cell.references())
                        .filter(|cell| !cell.descriptor().is_pruned_branch())
                        .collect::<Vec<_>>();
                    if next.is_empty() || next.len() > 32 {
                        break;
                    }
                    frontier = next;
                }
                let split_at = frontier.iter().map(|cell| *cell.hash(0)).collect();
                workers.install(|| update.par_apply(&old, &split_at))?
            } else {
                update.apply(&old)?
            };
            validate_state(&root, &self.id)?;
            ensure!(
                root.parse::<ShardStateUnsplit>()?.before_split == self.info.before_split,
                "new state before_split flag does not match block header"
            );

            Ok(StateView {
                id: self.id,
                root,
                reader: Arc::clone(reader),
            })
        })
    }
}

fn validate_state(root: &Cell, id: &BlockId) -> Result<()> {
    let state = root
        .parse::<ShardStateUnsplit>()
        .context("invalid shard state TL-B")?;
    ensure!(state.shard_ident == id.shard, "state shard mismatch");
    ensure!(state.seqno == id.seqno, "state sequence number mismatch");

    Ok(())
}
