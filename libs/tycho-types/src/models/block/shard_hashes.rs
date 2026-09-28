use crate::cell::*;
use crate::dict::{self, Dict, LoadDictKey};
use crate::error::Error;
use crate::models::block::block_id::{BlockId, ShardIdent};
use crate::models::currency::CurrencyCollection;
use crate::util::*;

/// A tree of the most recent descriptions for all currently existing shards
/// for all workchains except the masterchain.
#[derive(Debug, Default, Clone, Eq, PartialEq, Store, Load)]
pub struct ShardHashes(Dict<i32, Cell>);

impl ShardHashes {
    /// Wraps the shards dictionary.
    pub const fn from_dict(dict: Dict<i32, Cell>) -> Self {
        Self(dict)
    }

    /// Returns the underlying dictionary.
    pub fn as_dict(&self) -> &Dict<i32, Cell> {
        &self.0
    }

    /// Returns the underlying dictionary.
    pub fn into_dict(self) -> Dict<i32, Cell> {
        self.0
    }

    /// Tries to construct a [`ShardHashes`] from an iterator over the shards.
    /// The iterator must contain a list of all shards for each workchain.
    pub fn from_shards<'a, I>(iter: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = (&'a ShardIdent, &'a ShardDescription)>,
    {
        let mut groups = ahash::HashMap::<i32, Vec<_>>::default();
        for (ident, descr) in iter {
            groups
                .entry(ident.workchain())
                .or_default()
                .push((ident, descr));
        }

        for shards in groups.values_mut() {
            shards.sort_unstable_by_key(|(a, _)| *a);
        }

        let mut result = Dict::new();
        for (wc, shards) in groups {
            let cell = ok!(WorkchainShardHashes::try_build_raw(&shards));
            ok!(result.set(wc, cell));
        }

        Ok(Self(result))
    }

    /// Gets an iterator over the entries of the shard description trees, sorted by
    /// shard ident. The iterator element is `Result<(ShardIdent, ShardDescription)>`.
    ///
    /// If the dict or tree is invalid, finishes after the first invalid element.
    /// returning an error.
    pub fn iter(&self) -> ShardHashesIter<'_> {
        ShardHashesIter::new(self.0.root())
    }

    /// Gets an iterator over the raw entries of the shard description trees, sorted by
    /// shard ident. The iterator element is `Result<(ShardIdent, CellSlice)>`.
    ///
    /// If the dict or tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn raw_iter(&self) -> ShardsTreeRawIter<'_> {
        ShardsTreeRawIter::new(self.0.root())
    }

    /// Gets an iterator over the latest blocks in all shards, sorted by
    /// shard ident. The iterator element is `Result<BlockId>`.
    ///
    /// If the dict or tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn latest_blocks(&self) -> LatestBlocksIter<'_> {
        LatestBlocksIter::new(self.0.root())
    }

    /// Returns a shards description tree root for the specified workchain.
    pub fn get_workchain_shards(
        &self,
        workchain: i32,
    ) -> Result<Option<WorkchainShardHashes>, Error> {
        match self.0.get(workchain) {
            Ok(Some(root)) => Ok(Some(WorkchainShardHashes { workchain, root })),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Returns `true` if the dictionary contains a workchain for the specified id.
    pub fn contains_workchain<Q>(&self, workchain: i32) -> Result<bool, Error> {
        self.0.contains_key(workchain)
    }
}

/// A tree of the most recent descriptions for all currently existing shards
/// for a single workchain.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WorkchainShardHashes {
    workchain: i32,
    root: Cell,
}

impl WorkchainShardHashes {
    /// Returns the workchain of this tree.
    pub fn workchain(&self) -> i32 {
        self.workchain
    }

    /// Returns the root cell of this tree.
    pub fn root(&'_ self) -> &'_ DynCell {
        self.root.as_ref()
    }

    /// Returns the root cell of this tree.
    pub fn into_root(self) -> Cell {
        self.root
    }

    /// Gets an iterator over the keys of the shard descriptions tree, sorted by key.
    /// The iterator element type is `Result<ShardIdent>`.
    ///
    /// If the tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn keys(&self) -> WorkchainShardsTreeKeysIter<'_> {
        WorkchainShardsTreeKeysIter::new(self.workchain, self.root.as_ref())
    }

    /// Gets an iterator over the entries of the shard descriptions tree, sorted by key.
    /// The iterator element type is `Result<(ShardIdent, ShardDescription)>`.
    ///
    /// If the tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn iter(&self) -> WorkchainShardHashesIter<'_> {
        WorkchainShardHashesIter::new(self.workchain, self.root.as_ref())
    }

    /// Gets an iterator over the latest block in the current workchain, sorted by key.
    /// The iterator element type is `Result<BlockId>`.
    ///
    /// If the tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn latest_blocks(&self) -> WorkchainLatestBlocksIter<'_> {
        WorkchainLatestBlocksIter::new(self.workchain, self.root.as_ref())
    }

    /// Gets an iterator over the raw entries of the shard descriptions tree, sorted by key.
    /// The iterator element type is `Result<(ShardIdent, CellSlice)>`.
    ///
    /// If the tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn raw_iter(&self) -> WorkchainShardsTreeRawIter<'_> {
        WorkchainShardsTreeRawIter::new(self.workchain, self.root.as_ref())
    }

    /// Gets an iterator over the raw values of the shard descriptions tree, sorted by key.
    /// The iterator element type is `Result<CellSlice>`.
    ///
    /// If the tree is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn raw_values(&self) -> WorkchainShardsTreeRawValuesIter<'_> {
        WorkchainShardsTreeRawValuesIter::new(self.workchain, self.root.as_ref())
    }

    fn try_build_raw(shards: &[(&ShardIdent, &ShardDescription)]) -> Result<Cell, Error> {
        fn make_leaf(descr: &ShardDescription, cx: &dyn CellContext) -> Result<Cell, Error> {
            let mut builder = CellBuilder::new();
            ok!(builder.store_bit_zero());
            ok!(descr.store_into(&mut builder, cx));
            builder.build_ext(cx)
        }

        fn make_edge(left: Cell, right: Cell, cx: &dyn CellContext) -> Result<Cell, Error> {
            let mut builder = CellBuilder::new();
            ok!(builder.store_bit_one());
            ok!(builder.store_reference(left));
            ok!(builder.store_reference(right));
            builder.build_ext(cx)
        }

        #[inline]
        fn read_shard(
            iter: &mut std::slice::Iter<(&ShardIdent, &ShardDescription)>,
            cx: &dyn CellContext,
        ) -> Result<(ShardIdent, Cell), Error> {
            match iter.next() {
                Some((ident, descr)) => {
                    let cell = make_leaf(descr, cx)?;
                    Ok((**ident, cell))
                }
                None => Err(Error::Unbalanced),
            }
        }

        let shards = &mut shards.iter();
        let cx = Cell::empty_context();

        let first = ok!(read_shard(shards, cx));
        if first.0.is_full() {
            return Ok(first.1);
        }

        if first.0.is_right_child() {
            return Err(Error::Unbalanced);
        }

        let mut stack = vec![first];
        'outer: loop {
            // Get next leaf from the iterator
            let (mut next_shard, mut next_cell) = ok!(read_shard(shards, cx));

            // Repeat until we make a root
            loop {
                if next_shard.is_left_child() {
                    stack.push((next_shard, next_cell));
                    continue 'outer;
                }

                // Pop left child
                let Some((left_shard, left_cell)) = stack.pop() else {
                    break 'outer;
                };

                // Compute common parent shard
                let parent_shard = match (left_shard.merge(), next_shard.merge()) {
                    (Some(left_parent), Some(right_parent)) if left_parent == right_parent => {
                        left_parent
                    }
                    _ => break 'outer,
                };

                // Create parent cell
                let parent_cell = ok!(make_edge(left_cell, next_cell, cx));

                // Break if parent shard is a root
                if parent_shard.is_full() {
                    return Ok(parent_cell);
                }

                // Otherwise use parent shard as next child
                next_shard = parent_shard;
                next_cell = parent_cell;
            }
        }

        Err(Error::Unbalanced)
    }
}

impl From<WorkchainShardHashes> for Cell {
    #[inline]
    fn from(value: WorkchainShardHashes) -> Self {
        value.root
    }
}

/// An iterator over the entries of a [`ShardHashes`].
///
/// This struct is created by the [`iter`] method on [`ShardHashes`].
/// See its documentation for more.
///
/// [`iter`]: ShardHashes::iter
#[derive(Clone)]
pub struct ShardHashesIter<'a> {
    inner: ShardsTreeRawIter<'a>,
}

impl<'a> ShardHashesIter<'a> {
    fn new(dict: &'a Option<Cell>) -> Self {
        Self {
            inner: ShardsTreeRawIter::new(dict),
        }
    }
}

impl Iterator for ShardHashesIter<'_> {
    type Item = Result<(ShardIdent, ShardDescription), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok((shard_ident, mut value)) => match ShardDescription::load_from(&mut value) {
                Ok(value) => Ok((shard_ident, value)),
                Err(e) => Err(self.inner.finish(e)),
            },
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the latest blocks of a [`ShardHashes`].
///
/// This struct is created by the [`latest_blocks`] method on [`ShardHashes`].
/// See its documentation for more.
///
/// [`latest_blocks`]: ShardHashes::latest_blocks
#[derive(Clone)]
pub struct LatestBlocksIter<'a> {
    inner: ShardsTreeRawIter<'a>,
}

impl<'a> LatestBlocksIter<'a> {
    /// Creates an iterator over the latest blocks of a [`ShardHashes`].
    pub fn new(dict: &'a Option<Cell>) -> Self {
        Self {
            inner: ShardsTreeRawIter::new(dict),
        }
    }
}

impl Iterator for LatestBlocksIter<'_> {
    type Item = Result<BlockId, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok((shard_ident, value)) => match parse_block_id(shard_ident, value) {
                Ok(value) => Ok(value),
                Err(e) => Err(self.inner.finish(e)),
            },
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the entries of a [`WorkchainShardHashes`].
///
/// This struct is created by the [`iter`] method on [`WorkchainShardHashes`].
/// See its documentation for more.
///
/// [`iter`]: WorkchainShardHashes::iter
#[derive(Clone)]
pub struct WorkchainShardHashesIter<'a> {
    inner: WorkchainShardsTreeRawIter<'a>,
}

impl<'a> WorkchainShardHashesIter<'a> {
    /// Creates an iterator over the entries of a [`WorkchainShardHashes`].
    pub fn new(workchain: i32, root: &'a DynCell) -> Self {
        Self {
            inner: WorkchainShardsTreeRawIter::new(workchain, root),
        }
    }
}

impl Iterator for WorkchainShardHashesIter<'_> {
    type Item = Result<(ShardIdent, ShardDescription), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok((shard_ident, mut value)) => match ShardDescription::load_from(&mut value) {
                Ok(value) => Ok((shard_ident, value)),
                Err(e) => Err(self.inner.finish(e)),
            },
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the latest blocks of a [`WorkchainShardHashes`].
///
/// This struct is created by the [`latest_blocks`] method on [`WorkchainShardHashes`].
/// See its documentation for more.
///
/// [`latest_blocks`]: WorkchainShardHashes::latest_blocks
#[derive(Clone)]
pub struct WorkchainLatestBlocksIter<'a> {
    inner: WorkchainShardsTreeRawIter<'a>,
}

impl<'a> WorkchainLatestBlocksIter<'a> {
    /// Creates an iterator over the latest blocks of a [`WorkchainShardHashes`].
    pub fn new(workchain: i32, root: &'a DynCell) -> Self {
        Self {
            inner: WorkchainShardsTreeRawIter::new(workchain, root),
        }
    }
}

impl Iterator for WorkchainLatestBlocksIter<'_> {
    type Item = Result<BlockId, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok((shard_ident, value)) => match parse_block_id(shard_ident, value) {
                Ok(block_id) => Ok(block_id),
                Err(e) => Err(self.inner.finish(e)),
            },
            Err(e) => Err(e),
        })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ShardHashes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{Error, SerializeMap};

        let mut map = serializer.serialize_map(None)?;
        for entry in self.iter() {
            let (shard, descr) = entry.map_err(Error::custom)?;
            map.serialize_entry(&shard, &descr)?;
        }
        map.end()
    }
}

/// Description of the most recent state of the shard.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ShardDescription {
    /// Sequence number of the latest block in the shard.
    pub seqno: u32,
    /// The latest known masterchain block at the time of shard generation.
    pub reg_mc_seqno: u32,
    /// The beginning of the logical time range since the last MC block.
    pub start_lt: u64,
    /// The end of the logical time range since the last MC block.
    pub end_lt: u64,
    /// Representation hash of the root cell of the latest block in the shard.
    pub root_hash: HashBytes,
    /// Hash of the BOC encoded root cell of the latest block in the shard.
    pub file_hash: HashBytes,
    /// Whether this shard splits in the next block.
    pub before_split: bool,
    /// Whether this shard merges in the next block.
    pub before_merge: bool,
    /// Hint that this shard should split.
    pub want_split: bool,
    /// Hint that this shard should merge.
    pub want_merge: bool,
    /// Whether the value of catchain seqno has been incremented
    /// and will it also be incremented in the next block.
    pub nx_cc_updated: bool,
    /// Catchain seqno in the next block.
    pub next_catchain_seqno: u32,
    /// Duplicates the shard ident for the latest block in this shard.
    pub next_validator_shard: u64,
    /// Minimal referenced seqno of the masterchain block.
    pub min_ref_mc_seqno: u32,
    /// Unix timestamp when the latest block in this shard was created.
    pub gen_utime: u32,
    /// Planned split/merge time window if present.
    pub split_merge_at: Option<FutureSplitMerge>,
    /// Amount of fees collected in this shard since the last masterchain block.
    pub fees_collected: CurrencyCollection,
    /// Amount of funds created in this shard since the last masterchain block.
    pub funds_created: CurrencyCollection,
}

impl ShardDescription {
    const TAG_LEN: u16 = 4;

    const TAG_V1: u8 = 0xa;
    const TAG_V2: u8 = 0xb;

    /// Converts a `ShardDescription` to a `BlockId` given a shard identifier.
    pub fn as_block_id(&self, shard: ShardIdent) -> BlockId {
        BlockId {
            shard,
            seqno: self.seqno,
            root_hash: self.root_hash,
            file_hash: self.file_hash,
        }
    }
}

impl Store for ShardDescription {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let flags = ((self.before_split as u8) << 7)
            | ((self.before_merge as u8) << 6)
            | ((self.want_split as u8) << 5)
            | ((self.want_merge as u8) << 4)
            | ((self.nx_cc_updated as u8) << 3);

        ok!(builder.store_small_uint(Self::TAG_V1, Self::TAG_LEN));
        ok!(builder.store_u32(self.seqno));
        ok!(builder.store_u32(self.reg_mc_seqno));
        ok!(builder.store_u64(self.start_lt));
        ok!(builder.store_u64(self.end_lt));
        ok!(builder.store_u256(&self.root_hash));
        ok!(builder.store_u256(&self.file_hash));
        ok!(builder.store_u8(flags));
        ok!(builder.store_u32(self.next_catchain_seqno));
        ok!(builder.store_u64(self.next_validator_shard));
        ok!(builder.store_u32(self.min_ref_mc_seqno));
        ok!(builder.store_u32(self.gen_utime));
        ok!(self.split_merge_at.store_into(builder, context));

        let cell = {
            let mut builder = CellBuilder::new();
            ok!(self.fees_collected.store_into(&mut builder, context));
            ok!(self.funds_created.store_into(&mut builder, context));
            ok!(builder.build_ext(context))
        };

        builder.store_reference(cell)
    }
}

impl<'a> Load<'a> for ShardDescription {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let cont_in_cell = match slice.load_small_uint(Self::TAG_LEN) {
            Ok(Self::TAG_V1) => true,
            Ok(Self::TAG_V2) => false,
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        };

        let seqno = ok!(slice.load_u32());
        let reg_mc_seqno = ok!(slice.load_u32());
        let start_lt = ok!(slice.load_u64());
        let end_lt = ok!(slice.load_u64());
        let root_hash = ok!(slice.load_u256());
        let file_hash = ok!(slice.load_u256());

        let flags = ok!(slice.load_u8());
        if flags & 0b11 != 0 {
            return Err(Error::InvalidData);
        }

        let next_catchain_seqno = ok!(slice.load_u32());
        let next_validator_shard = ok!(slice.load_u64());
        let min_ref_mc_seqno = ok!(slice.load_u32());
        let gen_utime = ok!(slice.load_u32());
        let split_merge_at = ok!(Option::<FutureSplitMerge>::load_from(slice));

        let mut cont = if cont_in_cell {
            Some(ok!(slice.load_reference_as_slice()))
        } else {
            None
        };

        let slice = match &mut cont {
            Some(cont) => cont,
            None => slice,
        };

        let fees_collected = ok!(CurrencyCollection::load_from(slice));
        let funds_created = ok!(CurrencyCollection::load_from(slice));

        Ok(Self {
            seqno,
            reg_mc_seqno,
            start_lt,
            end_lt,
            root_hash,
            file_hash,
            before_split: flags & 0b10000000 != 0,
            before_merge: flags & 0b01000000 != 0,
            want_split: flags & 0b00100000 != 0,
            want_merge: flags & 0b00010000 != 0,
            nx_cc_updated: flags & 0b00001000 != 0,
            next_catchain_seqno,
            next_validator_shard,
            min_ref_mc_seqno,
            gen_utime,
            split_merge_at,
            fees_collected,
            funds_created,
        })
    }
}

fn parse_block_id(shard: ShardIdent, mut value: CellSlice) -> Result<BlockId, Error> {
    ok!(value.skip_first(ShardDescription::TAG_LEN, 0));

    Ok(BlockId {
        shard,
        seqno: ok!(value.load_u32()),
        root_hash: {
            // Skip some fields (reg_mc_seqno: u32, start_lt: u64, end_lt: u64)
            ok!(value.skip_first(32 + 64 + 64, 0));
            ok!(value.load_u256())
        },
        file_hash: ok!(value.load_u256()),
    })
}

/// Time window for shard split/merge.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum FutureSplitMerge {
    /// Shard split window info.
    Split {
        /// Unix timestamp of the planned start of the split.
        split_utime: u32,
        /// Window duration in seconds.
        interval: u32,
    },
    /// Shard merge window info.
    Merge {
        /// Unix timestamp of the planned start of the merge.
        merge_utime: u32,
        /// Window duration in seconds.
        interval: u32,
    },
}

impl Store for FutureSplitMerge {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        match *self {
            Self::Split {
                split_utime,
                interval,
            } => {
                ok!(builder.store_bit_zero());
                ok!(builder.store_u32(split_utime));
                builder.store_u32(interval)
            }
            Self::Merge {
                merge_utime,
                interval,
            } => {
                ok!(builder.store_bit_one());
                ok!(builder.store_u32(merge_utime));
                builder.store_u32(interval)
            }
        }
    }
}

impl<'a> Load<'a> for FutureSplitMerge {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let bit = ok!(slice.load_bit());
        let utime = ok!(slice.load_u32());
        let interval = ok!(slice.load_u32());
        Ok(if bit {
            Self::Merge {
                merge_utime: utime,
                interval,
            }
        } else {
            Self::Split {
                split_utime: utime,
                interval,
            }
        })
    }
}

/// An iterator over the raw entries of shard trees in multiple workchains.
#[derive(Clone)]
pub struct ShardsTreeRawIter<'a> {
    dict_iter: dict::RawIter<'a>,
    shard_hashes_iter: Option<WorkchainShardsTreeRawIter<'a>>,
    status: IterStatus,
}

impl<'a> ShardsTreeRawIter<'a> {
    fn new(dict: &'a Option<Cell>) -> Self {
        Self {
            dict_iter: dict::RawIter::new(dict, 32),
            shard_hashes_iter: None,
            status: IterStatus::Valid,
        }
    }

    #[inline(always)]
    fn finish(&mut self, error: Error) -> Error {
        self.status = IterStatus::Broken;
        error
    }
}

impl<'a> Iterator for ShardsTreeRawIter<'a> {
    type Item = Result<(ShardIdent, CellSlice<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if unlikely(!self.status.is_valid()) {
            return if self.status.is_unexpected_cell() {
                self.status = IterStatus::Broken;
                Some(Err(Error::UnexpectedExoticCell))
            } else {
                None
            };
        }

        loop {
            // Try to read next item from the current shards tree
            if let Some(iter) = &mut self.shard_hashes_iter {
                match iter.next() {
                    Some(next_item) => break Some(next_item),
                    None => self.shard_hashes_iter = None,
                }
            }

            // Try to get a shards tree for the next workchain
            self.shard_hashes_iter = Some(match self.dict_iter.next()? {
                Ok((key, value)) => {
                    // Parse workchain id from the raw iterator
                    let workchain = match i32::load_from_data(&key) {
                        Some(workchain) => workchain,
                        None => return Some(Err(self.finish(Error::CellUnderflow))),
                    };

                    // Shards tree is in the first reference in each value
                    let tree_root = match value.get_reference(0) {
                        Ok(cell) => cell,
                        Err(e) => break Some(Err(self.finish(e))),
                    };

                    // Create shards tree iterator
                    WorkchainShardsTreeRawIter::new(workchain, tree_root)
                }
                Err(e) => break Some(Err(self.finish(e))),
            });
        }
    }
}

/// An iterator over the keys of a shards tree in a single workchain.
#[derive(Clone)]
pub struct WorkchainShardsTreeKeysIter<'a> {
    inner: WorkchainShardsTreeRawIter<'a>,
}

impl<'a> WorkchainShardsTreeKeysIter<'a> {
    /// Creates an iterator over the keys of a shards tree in a single workchain.
    pub fn new(workchain: i32, root: &'a DynCell) -> Self {
        Self {
            inner: WorkchainShardsTreeRawIter::new(workchain, root),
        }
    }
}

impl Iterator for WorkchainShardsTreeKeysIter<'_> {
    type Item = Result<ShardIdent, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok((key, _)) => Some(Ok(key)),
            Err(e) => Some(Err(e)),
        }
    }
}

/// An iterator over the raw values of a shards tree in a single workchain.
#[derive(Clone)]
pub struct WorkchainShardsTreeRawValuesIter<'a> {
    inner: WorkchainShardsTreeRawIter<'a>,
}

impl<'a> WorkchainShardsTreeRawValuesIter<'a> {
    /// Creates an iterator over the raw values of a shards tree in a single workchain.
    pub fn new(workchain: i32, root: &'a DynCell) -> Self {
        Self {
            inner: WorkchainShardsTreeRawIter::new(workchain, root),
        }
    }
}

impl<'a> Iterator for WorkchainShardsTreeRawValuesIter<'a> {
    type Item = Result<CellSlice<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok((_, value)) => Some(Ok(value)),
            Err(e) => Some(Err(e)),
        }
    }
}

/// An iterator over the raw entries of a shards tree in a single workchain.
#[derive(Clone)]
pub struct WorkchainShardsTreeRawIter<'a> {
    workchain: i32,
    leaf: Option<CellSlice<'a>>,
    segments: Vec<IterSegment<'a>>,
    status: IterStatus,
}

impl<'a> WorkchainShardsTreeRawIter<'a> {
    /// Creates an iterator over the raw entries of a shards tree in a single workchain.
    pub fn new(workchain: i32, root: &'a DynCell) -> Self {
        let status = 'error: {
            let mut slice = match root.as_slice() {
                Ok(slice) => slice,
                Err(_) => break 'error IterStatus::UnexpectedCell,
            };

            let is_fork = match slice.load_bit() {
                Ok(bit) => bit,
                Err(_) => break 'error IterStatus::Broken,
            };

            let mut result = Self {
                workchain,
                leaf: None,
                segments: Vec::new(),
                status: IterStatus::Valid,
            };

            if is_fork {
                result.segments.push(IterSegment {
                    data: root,
                    is_right: false,
                });
            } else {
                result.leaf = Some(slice);
            }

            return result;
        };

        // Fallback to a broken iterator
        Self {
            workchain,
            leaf: None,
            segments: Vec::new(),
            status,
        }
    }

    fn finish(&mut self, err: Error) -> Error {
        self.status = IterStatus::Broken;
        err
    }
}

impl<'a> Iterator for WorkchainShardsTreeRawIter<'a> {
    type Item = Result<(ShardIdent, CellSlice<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        fn build_shard_prefix(segments: &[IterSegment<'_>]) -> u64 {
            let mut result = ShardIdent::PREFIX_FULL;
            for segment in segments.iter().rev() {
                result = (ShardIdent::PREFIX_FULL * segment.is_right as u64) | (result >> 1);
            }
            result
        }

        if unlikely(!self.status.is_valid()) {
            return if self.status.is_unexpected_cell() {
                self.status = IterStatus::Broken;
                Some(Err(Error::UnexpectedExoticCell))
            } else {
                None
            };
        }

        let leaf = match self.leaf.take() {
            Some(leaf) => leaf,
            None => loop {
                let segment = self.segments.last()?;
                let mut slice = match segment.data.get_reference_as_slice(segment.is_right as u8) {
                    Ok(child) => child,
                    Err(e) => return Some(Err(self.finish(e))),
                };

                match slice.load_bit() {
                    // Break on leaf
                    Ok(false) => break slice,
                    // Add segment on fork
                    Ok(true) if self.segments.len() < ShardIdent::MAX_SPLIT_DEPTH as usize => {
                        self.segments.push(IterSegment {
                            data: slice.cell(),
                            is_right: false,
                        })
                    }
                    _ => return Some(Err(self.finish(Error::CellUnderflow))),
                };
            },
        };

        // Build shard prefix from segments
        // SAFETY: segments lengths is guaranteed to be in range 1..=ShardIdent::MAX_SPLIT_DEPTH
        let shard_prefix = unsafe {
            ShardIdent::new_unchecked(self.workchain, build_shard_prefix(&self.segments))
        };

        // Remove all finished segments from the top of the stack
        while matches!(self.segments.last(), Some(segment) if segment.is_right) {
            self.segments.pop();
        }

        // Move last bit
        if let Some(segment) = self.segments.last_mut() {
            segment.is_right = true;
        }

        Some(Ok((shard_prefix, leaf)))
    }
}

#[derive(Clone)]
struct IterSegment<'a> {
    data: &'a DynCell,
    is_right: bool,
}

impl Copy for IterSegment<'_> {}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::*;
    use crate::boc::BocRepr;

    #[test]
    fn shard_hashes_serde() {
        let root = ShardIdent::new_full(0);
        let (left, right) = root.split().unwrap();
        let (left_left, left_right) = left.split().unwrap();
        let empty_info = ShardDescription {
            seqno: 0,
            reg_mc_seqno: 0,
            start_lt: 0,
            end_lt: 0,
            root_hash: Default::default(),
            file_hash: Default::default(),
            before_split: false,
            before_merge: false,
            want_split: false,
            want_merge: false,
            nx_cc_updated: false,
            next_catchain_seqno: 0,
            next_validator_shard: 0,
            min_ref_mc_seqno: 0,
            gen_utime: 0,
            split_merge_at: None,
            fees_collected: Default::default(),
            funds_created: Default::default(),
        };
        // arbitrary order
        let input = HashMap::from([
            (left_right, empty_info.clone()),
            (right, empty_info.clone()),
            (left_left, empty_info.clone()),
        ]);
        let hashes = ShardHashes::from_shards(&input).unwrap();
        let serialized = BocRepr::encode(&hashes).unwrap();
        let deserialized = BocRepr::decode(serialized).unwrap();
        assert_eq!(hashes, deserialized);

        for item in deserialized.iter() {
            let (_, _) = item.unwrap();
        }

        // one missing
        let input = HashMap::from([
            (right, empty_info.clone()),
            (left_right, empty_info.clone()),
        ]);
        let hashes = ShardHashes::from_shards(&input);
        assert!(hashes.is_err());

        // unsplitted
        let input = HashMap::from([(root, empty_info.clone())]);
        let hashes = ShardHashes::from_shards(&input).unwrap();
        let serialized = BocRepr::encode(&hashes).unwrap();
        let deserialized = BocRepr::decode(serialized).unwrap();
        assert_eq!(hashes, deserialized);

        let input = HashMap::from([(left, empty_info.clone()), (right, empty_info.clone())]);
        let hashes = ShardHashes::from_shards(&input).unwrap();
        let serialized = BocRepr::encode(&hashes).unwrap();
        let deserialized = BocRepr::decode(serialized).unwrap();
        assert_eq!(hashes, deserialized);

        let master = ShardIdent::new_full(-1);

        let input = HashMap::from([
            (master, empty_info.clone()),
            (left, empty_info.clone()),
            (right, empty_info.clone()),
        ]);
        let hashes = ShardHashes::from_shards(&input).unwrap();
        let serialized = BocRepr::encode(&hashes).unwrap();
        let deserialized = BocRepr::decode(serialized).unwrap();
        assert_eq!(hashes, deserialized);

        let input = HashMap::from([
            (master, empty_info.clone()),
            (right, empty_info.clone()),
            (left_left, empty_info.clone()),
            (left_right, empty_info.clone()),
        ]);
        let hashes = ShardHashes::from_shards(&input).unwrap();
        let serialized = BocRepr::encode(&hashes).unwrap();
        let deserialized = BocRepr::decode(serialized).unwrap();
        assert_eq!(hashes, deserialized);

        // left is extra
        let input = HashMap::from([
            (master, empty_info.clone()),
            (left, empty_info.clone()),
            (right, empty_info.clone()),
            (left_left, empty_info.clone()),
            (left_right, empty_info.clone()),
        ]);
        let hashes = ShardHashes::from_shards(&input);
        assert!(hashes.is_err());
    }
}
