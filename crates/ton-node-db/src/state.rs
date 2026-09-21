use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, ensure};
use rocksdb::DB;
use sha2::{Digest, Sha256};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, Lazy, Load};
use tycho_types::models::{
    Block, BlockId, PrevBlockRef, ShardAccount, ShardDescription, ShardIdent, ShardStateUnsplit,
    StdAddr,
};

use crate::lazy::Reader;
use crate::{ReadStats, StateRecord};

/// An account and the exact masterchain/shard blocks that locate it.
/// `None` means the account dictionary has no entry for the requested address.
pub struct AccountSnapshot {
    pub masterchain_block: BlockId,
    pub shard_block: BlockId,
    pub account: Option<ShardAccount>,
    pub reads: ReadStats,
}

/// A retained shard state with cells fetched only when traversed.
/// Lazy references never escape the view: returned accounts are fully owned.
/// An I/O or cell-integrity error poisons the view; open another to retry.
pub struct StateView {
    id: BlockId,
    root: Cell,
    reader: Arc<Reader>,
}

impl StateView {
    pub(crate) fn open(db: Arc<DB>, record: StateRecord, limit: usize) -> Result<Self> {
        let reader = Reader::new(db, limit);
        let root = reader.load(record.root_hash)?;
        reader.run(|| validate_state(&root, &record.block_id))?;

        Ok(Self {
            id: record.block_id,
            root,
            reader,
        })
    }

    /// Identifies the state, including successfully applied in-memory updates.
    pub fn block_id(&self) -> BlockId {
        self.id
    }

    /// Reports cumulative database reads for this view. Reusing loaded cell
    /// references does not increment the counters.
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
        let root = self.reader.run(|| {
            ensure!(
                self.id.shard.is_masterchain() && id.shard == self.id.shard,
                "state updates require masterchain blocks"
            );
            ensure!(
                self.id.seqno.checked_add(1) == Some(id.seqno),
                "block is not the next masterchain block"
            );
            ensure!(
                Sha256::digest(bytes)[..] == id.file_hash.0,
                "block file hash mismatch"
            );
            let cell = Boc::decode(bytes)?;
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
            let PrevBlockRef::Single(previous) = info.load_prev_ref()? else {
                anyhow::bail!("masterchain block has multiple predecessors");
            };
            ensure!(
                previous.as_block_id(id.shard) == self.id,
                "block predecessor mismatch"
            );
            let root = block.load_state_update()?.apply(&self.root)?;
            validate_state(&root, id)?;

            Ok(root)
        })?;

        self.root = root;
        self.id = *id;
        Ok(())
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
