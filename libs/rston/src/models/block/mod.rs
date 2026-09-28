//! Block models.

#[cfg(feature = "sync")]
use std::sync::OnceLock;

pub use self::block_extra::*;
pub use self::block_id::*;
pub use self::block_proof::*;
pub use self::shard_hashes::*;
use crate::cell::*;
use crate::error::Error;
use crate::merkle::MerkleUpdate;
use crate::models::currency::CurrencyCollection;
use crate::models::global_version::GlobalVersion;
use crate::util::*;

mod block_extra;
mod block_id;
mod block_proof;
mod shard_hashes;

#[cfg(test)]
mod tests;

/// A masterchain or shardchain block in the TON `block#11ef55aa` layout.
///
/// Referenced structures remain lazy until their corresponding `load_*` method is called.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[tlb(tag = "#11ef55aa")]
pub struct Block {
    /// Global network id.
    pub global_id: i32,
    /// Block info.
    pub info: Lazy<BlockInfo>,
    /// Tokens flow info.
    pub value_flow: Lazy<ValueFlow>,
    /// Merkle update for the shard state.
    pub state_update: LazyExotic<MerkleUpdate>,
    /// Block content.
    pub extra: Lazy<BlockExtra>,
}

impl Block {
    const DATA_FOR_SIGN_SIZE: usize = 4 + 32 + 32;
    const DATA_FOR_SIGN_TAG: [u8; 4] = [0x70, 0x6e, 0x0b, 0xc5];

    /// Tries to load block info.
    pub fn load_info(&self) -> Result<BlockInfo, Error> {
        self.info.load()
    }

    /// Tries to load tokens flow info.
    pub fn load_value_flow(&self) -> Result<ValueFlow, Error> {
        self.value_flow.load()
    }

    /// Tries to load state update.
    pub fn load_state_update(&self) -> Result<MerkleUpdate, Error> {
        self.state_update.load()
    }

    /// Tries to load block content.
    pub fn load_extra(&self) -> Result<BlockExtra, Error> {
        self.extra.load()
    }

    /// Builds a data for validators to sign.
    pub fn build_data_for_sign(block_id: &BlockId) -> [u8; Self::DATA_FOR_SIGN_SIZE] {
        let mut data = [0u8; Self::DATA_FOR_SIGN_SIZE];
        data[0..4].copy_from_slice(&Self::DATA_FOR_SIGN_TAG);
        data[4..36].copy_from_slice(block_id.root_hash.as_ref());
        data[36..68].copy_from_slice(block_id.file_hash.as_ref());
        data
    }
}

/// Block info.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockInfo {
    /// Block model version.
    pub version: u32,
    /// Whether this block was produced after the shards were merged.
    pub after_merge: bool,
    /// Whether this block was produced before the shards split.
    pub before_split: bool,
    /// Whether this block was produced after the shards split.
    pub after_split: bool,
    /// Hint that the shard with this block should split.
    pub want_split: bool,
    /// Hint that the shard with this block should merge.
    pub want_merge: bool,
    /// Whether this block is a key block.
    pub key_block: bool,

    /// Block flags (currently only bit 1 is used, for [`gen_software`])
    ///
    /// [`gen_software`]: Self::gen_software
    pub flags: u8,
    /// Block sequence number.
    pub seqno: u32,
    /// Block vertical sequence number.
    pub vert_seqno: u32,

    /// Shard id where this block was produced.
    pub shard: ShardIdent,
    /// Unix timestamp when the block was created.
    pub gen_utime: u32,
    /// Logical time range start.
    pub start_lt: u64,
    /// Logical time range end.
    pub end_lt: u64,
    /// Last 4 bytes of the hash of the validator list.
    pub gen_validator_list_hash_short: u32,
    /// Seqno of the catchain session where this block was produced.
    pub gen_catchain_seqno: u32,
    /// Minimal referenced seqno of the masterchain block.
    pub min_ref_mc_seqno: u32,
    /// Previous key block seqno.
    pub prev_key_block_seqno: u32,
    /// The version and capabilities of the software that created this block.
    pub gen_software: GlobalVersion,

    /// Reference to the masterchain block which was used during the creation of this block.
    pub master_ref: Option<Lazy<BlockRef>>,
    /// Reference to the previous block (or blocks).
    #[cfg_attr(feature = "serde", serde(with = "serde_prev_block_ref"))]
    pub prev_ref: Cell,
    /// Optional reference to the previous vertical block.
    pub prev_vert_ref: Option<Lazy<BlockRef>>,
}

#[cfg(feature = "sync")]
impl Default for BlockInfo {
    fn default() -> Self {
        Self {
            version: 0,
            after_merge: false,
            before_split: false,
            after_split: false,
            want_split: false,
            want_merge: false,
            key_block: false,
            flags: 0,
            seqno: 0,
            vert_seqno: 0,
            shard: ShardIdent::MASTERCHAIN,
            gen_utime: 0,
            start_lt: 0,
            end_lt: 0,
            gen_validator_list_hash_short: 0,
            gen_catchain_seqno: 0,
            min_ref_mc_seqno: 0,
            prev_key_block_seqno: 0,
            gen_software: Default::default(),
            master_ref: None,
            prev_ref: PrevBlockRef::empty_single_ref().clone(),
            prev_vert_ref: None,
        }
    }
}

impl BlockInfo {
    const TAG_V1: u32 = 0x9bc7a987;
    const FLAG_WITH_GEN_SOFTWARE: u8 = 0x1;

    /// Set the version and capabilities of the software that created this block.
    pub fn set_gen_software(&mut self, gen_software: Option<GlobalVersion>) {
        if let Some(gen_software) = gen_software {
            self.gen_software = gen_software;
            self.flags |= BlockInfo::FLAG_WITH_GEN_SOFTWARE;
        } else {
            self.gen_software = Default::default();
            self.flags &= !BlockInfo::FLAG_WITH_GEN_SOFTWARE;
        }
    }

    /// Tries to load a reference to the masterchain block.
    pub fn load_master_ref(&self) -> Result<Option<BlockRef>, Error> {
        match &self.master_ref {
            Some(master_ref) => master_ref.load().map(Some),
            None => Ok(None),
        }
    }

    /// Tries to load a reference to the previous block (or blocks).
    pub fn load_prev_ref(&self) -> Result<PrevBlockRef, Error> {
        PrevBlockRef::load_from_cell(&self.prev_ref, self.after_merge)
    }

    /// Set previous block reference.
    pub fn set_prev_ref(&mut self, prev_ref: &PrevBlockRef) {
        match prev_ref {
            PrevBlockRef::Single(prev_ref) => self.set_prev_ref_single(prev_ref),
            PrevBlockRef::AfterMerge { left, right } => self.set_prev_ref_after_merge(left, right),
        }
    }

    /// Set previous block reference (direct).
    pub fn set_prev_ref_single(&mut self, prev_ref: &BlockRef) {
        // NOTE: Unwrap is ok because we control the input.
        self.prev_ref = CellBuilder::build_from(prev_ref).unwrap();
    }

    /// Set previous block reference (split).
    pub fn set_prev_ref_after_merge(&mut self, left: &BlockRef, right: &BlockRef) {
        fn store_split_ref(left: &BlockRef, right: &BlockRef) -> Result<Cell, Error> {
            let cx = Cell::empty_context();
            let mut builder = CellBuilder::new();
            ok!(builder.store_reference(ok!(CellBuilder::build_from_ext(left, cx))));
            ok!(builder.store_reference(ok!(CellBuilder::build_from_ext(right, cx))));
            builder.build_ext(cx)
        }

        // NOTE: Unwrap is ok because we control the input.
        self.prev_ref = store_split_ref(left, right).unwrap();
    }
}

impl Store for BlockInfo {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let packed_flags = ((self.master_ref.is_some() as u8) << 7)
            | ((self.after_merge as u8) << 6)
            | ((self.before_split as u8) << 5)
            | ((self.after_split as u8) << 4)
            | ((self.want_split as u8) << 3)
            | ((self.want_merge as u8) << 2)
            | ((self.key_block as u8) << 1)
            | (self.prev_vert_ref.is_some() as u8);

        ok!(builder.store_u32(Self::TAG_V1));

        ok!(builder.store_u32(self.version));
        ok!(builder.store_u16(u16::from_be_bytes([packed_flags, self.flags])));
        ok!(builder.store_u32(self.seqno));
        ok!(builder.store_u32(self.vert_seqno));
        ok!(self.shard.store_into(builder, context));
        ok!(builder.store_u32(self.gen_utime));
        ok!(builder.store_u64(self.start_lt));
        ok!(builder.store_u64(self.end_lt));
        ok!(builder.store_u32(self.gen_validator_list_hash_short));
        ok!(builder.store_u32(self.gen_catchain_seqno));
        ok!(builder.store_u32(self.min_ref_mc_seqno));
        ok!(builder.store_u32(self.prev_key_block_seqno));

        if self.flags & Self::FLAG_WITH_GEN_SOFTWARE != 0 {
            ok!(self.gen_software.store_into(builder, context));
        }

        if let Some(master_ref) = &self.master_ref {
            ok!(builder.store_reference(master_ref.inner().clone()));
        }

        ok!(builder.store_reference(self.prev_ref.clone()));

        if let Some(prev_vert_ref) = &self.prev_vert_ref {
            builder.store_reference(prev_vert_ref.inner().clone())
        } else {
            Ok(())
        }
    }
}

impl<'a> Load<'a> for BlockInfo {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if ok!(slice.load_u32()) != Self::TAG_V1 {
            return Err(Error::InvalidTag);
        }

        let version = ok!(slice.load_u32());
        let [packed_flags, flags] = ok!(slice.load_u16()).to_be_bytes();
        let seqno = ok!(slice.load_u32());
        if seqno == 0 {
            return Err(Error::InvalidData);
        }
        let vert_seqno = ok!(slice.load_u32());
        let shard = ok!(ShardIdent::load_from(slice));
        let gen_utime = ok!(slice.load_u32());
        let start_lt = ok!(slice.load_u64());
        let end_lt = ok!(slice.load_u64());
        let gen_validator_list_hash_short = ok!(slice.load_u32());
        let gen_catchain_seqno = ok!(slice.load_u32());
        let min_ref_mc_seqno = ok!(slice.load_u32());
        let prev_key_block_seqno = ok!(slice.load_u32());

        let gen_software = if flags & Self::FLAG_WITH_GEN_SOFTWARE != 0 {
            ok!(GlobalVersion::load_from(slice))
        } else {
            GlobalVersion::default()
        };

        let master_ref = if packed_flags & 0b10000000 != 0 {
            Some(ok!(Lazy::<BlockRef>::load_from(slice)))
        } else {
            None
        };

        let prev_ref = ok!(slice.load_reference_cloned());

        let prev_vert_ref = if packed_flags & 0b00000001 != 0 {
            Some(ok!(Lazy::<BlockRef>::load_from(slice)))
        } else {
            None
        };

        if vert_seqno < prev_vert_ref.is_some() as u32 {
            return Err(Error::InvalidData);
        }

        Ok(Self {
            version,
            after_merge: packed_flags & 0b01000000 != 0,
            before_split: packed_flags & 0b00100000 != 0,
            after_split: packed_flags & 0b00010000 != 0,
            want_split: packed_flags & 0b00001000 != 0,
            want_merge: packed_flags & 0b00000100 != 0,
            key_block: packed_flags & 0b00000010 != 0,
            flags,
            seqno,
            vert_seqno,
            shard,
            gen_utime,
            start_lt,
            end_lt,
            gen_validator_list_hash_short,
            gen_catchain_seqno,
            min_ref_mc_seqno,
            prev_key_block_seqno,
            gen_software,
            master_ref,
            prev_ref,
            prev_vert_ref,
        })
    }
}

#[cfg(feature = "serde")]
mod serde_prev_block_ref {
    use super::*;

    pub fn serialize<S: serde::Serializer>(value: &Cell, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{Error, Serialize};

        if serializer.is_human_readable() {
            let prev_block_ref = ok!(PrevBlockRef::load_from_cell(
                value,
                value.reference_count() > 0
            )
            .map_err(Error::custom));
            prev_block_ref.serialize(serializer)
        } else {
            crate::boc::Boc::serialize(value, serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Cell, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Deserialize, Error};

        if deserializer.is_human_readable() {
            PrevBlockRef::deserialize(deserializer).and_then(|prev_block_ref| {
                CellBuilder::build_from(prev_block_ref).map_err(Error::custom)
            })
        } else {
            crate::boc::Boc::deserialize(deserializer)
        }
    }
}

/// Reference to the previous block.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", content = "id"))]
pub enum PrevBlockRef {
    /// Reference to the parent block (simple case).
    Single(BlockRef),
    /// Reference to both parent blocks (case after merge).
    AfterMerge {
        /// Block id from the "left" shard.
        ///
        /// See [`is_left_child`].
        ///
        /// [`is_left_child`]: ShardIdent::is_left_child
        left: BlockRef,
        /// Block id from the "right" shard.
        ///
        /// See [`is_right_child`].
        ///
        /// [`is_right_child`]: ShardIdent::is_right_child
        right: BlockRef,
    },
}

impl PrevBlockRef {
    /// Returns a static reference to an empty single reference.
    #[cfg(feature = "sync")]
    pub fn empty_single_ref() -> &'static Cell {
        static CELL: OnceLock<Cell> = OnceLock::new();
        CELL.get_or_init(|| {
            CellBuilder::build_from(&BlockRef {
                end_lt: 0,
                seqno: 0,
                root_hash: HashBytes::ZERO,
                file_hash: HashBytes::ZERO,
            })
            .unwrap()
        })
    }

    fn load_from_cell(value: &Cell, after_merge: bool) -> Result<Self, Error> {
        let mut s = ok!(value.as_slice());
        Ok(if unlikely(after_merge) {
            let left = ok!(BlockRef::load_from(&mut ok!(s.load_reference_as_slice())));
            let right = ok!(BlockRef::load_from(&mut ok!(s.load_reference_as_slice())));
            PrevBlockRef::AfterMerge { left, right }
        } else {
            PrevBlockRef::Single(ok!(BlockRef::load_from(&mut s)))
        })
    }
}

impl Store for PrevBlockRef {
    fn store_into(&self, builder: &mut CellBuilder, cx: &dyn CellContext) -> Result<(), Error> {
        match self {
            PrevBlockRef::Single(block_ref) => block_ref.store_into(builder, cx),
            PrevBlockRef::AfterMerge { left, right } => {
                ok!(builder.store_reference(ok!(CellBuilder::build_from_ext(left, cx))));
                builder.store_reference(ok!(CellBuilder::build_from_ext(right, cx)))
            }
        }
    }
}

/// Reference to the external block.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockRef {
    /// The end of the logical time of the referenced block.
    pub end_lt: u64,
    /// Sequence number of the referenced block.
    pub seqno: u32,
    /// Representation hash of the root cell of the referenced block.
    pub root_hash: HashBytes,
    /// Hash of the BOC encoded root cell of the referenced block.
    pub file_hash: HashBytes,
}

impl BlockRef {
    /// Converts a `BlockRef` to a `BlockId` given a shard identifier.
    pub fn as_block_id(&self, shard: ShardIdent) -> BlockId {
        BlockId {
            shard,
            seqno: self.seqno,
            root_hash: self.root_hash,
            file_hash: self.file_hash,
        }
    }
}

/// Tokens flow info.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValueFlow {
    /// Total amount transferred from the previous block.
    pub from_prev_block: CurrencyCollection,
    /// Total amount transferred to the next block.
    pub to_next_block: CurrencyCollection,
    /// Sum of all imported amounts from messages.
    pub imported: CurrencyCollection,
    /// Sum of all exported amounts of messages.
    pub exported: CurrencyCollection,

    /// Total fees collected in this block.
    pub fees_collected: CurrencyCollection,

    /// Burned native tokens and extra currencies.
    pub burned: CurrencyCollection,

    /// Shard fees imported to this block.
    pub fees_imported: CurrencyCollection,
    /// Fee recovery (?)
    pub recovered: CurrencyCollection,
    /// Block creation fees.
    pub created: CurrencyCollection,
    /// Minted native tokens and extra currencies.
    pub minted: CurrencyCollection,
}

impl ValueFlow {
    const TAG_V1: u32 = 0xb8e48dfb;
    const TAG_V2: u32 = 0x3ebf98b7;

    /// Returns `true` if an inbound value flow is in sync with an outbound.
    pub fn validate(&self) -> Result<bool, Error> {
        let in_val = self
            .from_prev_block
            .checked_add(&self.imported)
            .and_then(|val| val.checked_add(&self.fees_imported))
            .and_then(|val| val.checked_add(&self.created))
            .and_then(|val| val.checked_add(&self.minted))
            .and_then(|val| val.checked_add(&self.recovered))?;

        let out_val = self
            .to_next_block
            .checked_add(&self.exported)
            .and_then(|val| val.checked_add(&self.fees_collected))
            .and_then(|val| val.checked_add(&self.burned))?;

        Ok(in_val == out_val)
    }
}

impl Store for ValueFlow {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let (tag, store_burned) = if self.burned.is_zero() {
            (Self::TAG_V1, false)
        } else {
            (Self::TAG_V2, true)
        };

        let cell1 = {
            let mut builder = CellBuilder::new();
            ok!(self.from_prev_block.store_into(&mut builder, context));
            ok!(self.to_next_block.store_into(&mut builder, context));
            ok!(self.imported.store_into(&mut builder, context));
            ok!(self.exported.store_into(&mut builder, context));
            ok!(builder.build_ext(context))
        };

        ok!(builder.store_u32(tag));
        ok!(builder.store_reference(cell1));

        ok!(self.fees_collected.store_into(builder, context));
        if store_burned {
            ok!(self.burned.store_into(builder, context));
        }

        let cell2 = {
            let mut builder = CellBuilder::new();
            ok!(self.fees_imported.store_into(&mut builder, context));
            ok!(self.recovered.store_into(&mut builder, context));
            ok!(self.created.store_into(&mut builder, context));
            ok!(self.minted.store_into(&mut builder, context));
            ok!(builder.build_ext(context))
        };
        builder.store_reference(cell2)
    }
}

impl<'a> Load<'a> for ValueFlow {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let with_burned = match ok!(slice.load_u32()) {
            Self::TAG_V1 => false,
            Self::TAG_V2 => true,
            _ => return Err(Error::InvalidTag),
        };

        let fees_collected = ok!(CurrencyCollection::load_from(slice));
        let burned = if with_burned {
            ok!(CurrencyCollection::load_from(slice))
        } else {
            CurrencyCollection::ZERO
        };

        let slice1 = &mut ok!(slice.load_reference_as_slice());
        let slice2 = &mut ok!(slice.load_reference_as_slice());

        Ok(Self {
            from_prev_block: ok!(CurrencyCollection::load_from(slice1)),
            to_next_block: ok!(CurrencyCollection::load_from(slice1)),
            imported: ok!(CurrencyCollection::load_from(slice1)),
            exported: ok!(CurrencyCollection::load_from(slice1)),
            fees_collected,
            burned,
            fees_imported: ok!(CurrencyCollection::load_from(slice2)),
            recovered: ok!(CurrencyCollection::load_from(slice2)),
            created: ok!(CurrencyCollection::load_from(slice2)),
            minted: ok!(CurrencyCollection::load_from(slice2)),
        })
    }
}
