//! Shard state models.

#[cfg(feature = "sync")]
use std::sync::OnceLock;

pub use self::shard_accounts::*;
pub use self::shard_extra::*;
use crate::cell::*;
use crate::dict::Dict;
use crate::error::*;
use crate::models::block::{BlockRef, ShardIdent};
use crate::models::currency::CurrencyCollection;

mod shard_accounts;
mod shard_extra;

#[cfg(test)]
mod tests;

/// Applied shard state.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ShardState {
    /// The next indivisible state in the shard.
    Unsplit(ShardStateUnsplit),
    /// Next indivisible states after shard split.
    Split(ShardStateSplit),
}

impl Store for ShardState {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Unsplit(state) => state.store_into(builder, context),
            Self::Split(state) => state.store_into(builder, context),
        }
    }
}

impl<'a> Load<'a> for ShardState {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(if ok!(slice.get_bit(0)) {
            match ShardStateUnsplit::load_from(slice) {
                Ok(state) => Self::Unsplit(state),
                Err(e) => return Err(e),
            }
        } else {
            match ShardStateSplit::load_from(slice) {
                Ok(state) => Self::Split(state),
                Err(e) => return Err(e),
            }
        })
    }
}

/// State of the single shard.
///
/// # TLB scheme
///
/// ```text
/// shard_state#9023afe2
///     global_id:int32
///     shard_id:ShardIdent
///     seq_no:uint32 vert_seq_no:#
///     gen_utime:uint32 gen_lt:uint64
///     min_ref_mc_seqno:uint32
///     out_msg_queue_info:^OutMsgQueueInfo
///     before_split:(## 1)
///     accounts:^ShardAccounts
///     ^[
///         overload_history:uint64
///         underload_history:uint64
///         total_balance:CurrencyCollection
///         total_validator_fees:CurrencyCollection
///         libraries:(HashmapE 256 LibDescr)
///         master_ref:(Maybe BlkMasterInfo)
///     ]
///     custom:(Maybe ^McStateExtra)
///     = ShardStateUnsplit;
/// ```
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ShardStateUnsplit {
    /// Global network id.
    pub global_id: i32,
    /// Id of the shard.
    pub shard_ident: ShardIdent,
    /// Sequence number of the corresponding block.
    pub seqno: u32,
    /// Vertical sequcent number of the corresponding block.
    pub vert_seqno: u32,
    /// Unix timestamp when the block was created.
    pub gen_utime: u32,
    /// Logical time when the state was created.
    pub gen_lt: u64,
    /// Minimal referenced seqno of the masterchain block.
    pub min_ref_mc_seqno: u32,

    /// Output messages queue info (stub).
    pub out_msg_queue_info: Cell,

    /// Whether this state was produced before the shards split.
    pub before_split: bool,
    /// Reference to the dictionary with shard accounts.
    pub accounts: Lazy<ShardAccounts>,
    /// Mask for the overloaded blocks.
    pub overload_history: u64,
    /// Mask for the underloaded blocks.
    pub underload_history: u64,
    /// Total balance for all currencies.
    pub total_balance: CurrencyCollection,
    /// Total pending validator fees.
    pub total_validator_fees: CurrencyCollection,
    /// Dictionary with all libraries and its providers.
    pub libraries: Dict<HashBytes, LibDescr>,
    /// Optional reference to the masterchain block.
    pub master_ref: Option<BlockRef>,
    /// Shard state additional info.
    pub custom: Option<Lazy<McStateExtra>>,
}

#[cfg(feature = "sync")]
impl Default for ShardStateUnsplit {
    fn default() -> Self {
        Self {
            global_id: 0,
            shard_ident: ShardIdent::MASTERCHAIN,
            seqno: 0,
            vert_seqno: 0,
            gen_utime: 0,
            gen_lt: 0,
            min_ref_mc_seqno: 0,
            out_msg_queue_info: Cell::default(),
            before_split: false,
            accounts: Self::empty_shard_accounts().clone(),
            overload_history: 0,
            underload_history: 0,
            total_balance: CurrencyCollection::ZERO,
            total_validator_fees: CurrencyCollection::ZERO,
            libraries: Dict::new(),
            master_ref: None,
            custom: None,
        }
    }
}

impl ShardStateUnsplit {
    const TAG_V1: u32 = 0x9023afe2;

    /// Returns a static reference to the empty shard accounts.
    #[cfg(feature = "sync")]
    pub fn empty_shard_accounts() -> &'static Lazy<ShardAccounts> {
        static SHARD_ACCOUNTS: OnceLock<Lazy<ShardAccounts>> = OnceLock::new();
        SHARD_ACCOUNTS.get_or_init(|| Lazy::new(&ShardAccounts::new()).unwrap())
    }

    /// Tries to load shard accounts dictionary.
    pub fn load_accounts(&self) -> Result<ShardAccounts, Error> {
        self.accounts.load()
    }

    /// Tries to load additional masterchain data.
    pub fn load_custom(&self) -> Result<Option<McStateExtra>, Error> {
        match &self.custom {
            Some(custom) => match custom.load() {
                Ok(custom) => Ok(Some(custom)),
                Err(e) => Err(e),
            },
            None => Ok(None),
        }
    }

    /// Tries to set additional masterchain data.
    pub fn set_custom(&mut self, value: Option<&McStateExtra>) -> Result<(), Error> {
        match (&mut self.custom, value) {
            (None, None) => Ok(()),
            (None, Some(value)) => {
                self.custom = Some(ok!(Lazy::new(value)));
                Ok(())
            }
            (Some(_), None) => {
                self.custom = None;
                Ok(())
            }
            (Some(custom), Some(value)) => custom.set(value),
        }
    }
}

impl Store for ShardStateUnsplit {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let child_cell = {
            let mut builder = CellBuilder::new();
            ok!(builder.store_u64(self.overload_history));
            ok!(builder.store_u64(self.underload_history));
            ok!(self.total_balance.store_into(&mut builder, context));
            ok!(self.total_validator_fees.store_into(&mut builder, context));
            ok!(self.libraries.store_into(&mut builder, context));
            ok!(self.master_ref.store_into(&mut builder, context));
            ok!(builder.build_ext(context))
        };

        ok!(builder.store_u32(Self::TAG_V1));

        ok!(builder.store_u32(self.global_id as u32));
        ok!(self.shard_ident.store_into(builder, context));
        ok!(builder.store_u32(self.seqno));
        ok!(builder.store_u32(self.vert_seqno));
        ok!(builder.store_u32(self.gen_utime));

        ok!(builder.store_u64(self.gen_lt));
        ok!(builder.store_u32(self.min_ref_mc_seqno));
        ok!(self.out_msg_queue_info.store_into(builder, context));
        ok!(builder.store_bit(self.before_split));
        ok!(builder.store_reference(self.accounts.inner().clone()));
        ok!(builder.store_reference(child_cell));

        ok!(self.custom.store_into(builder, context));

        Ok(())
    }
}

impl<'a> Load<'a> for ShardStateUnsplit {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if ok!(slice.load_u32()) != Self::TAG_V1 {
            return Err(Error::InvalidTag);
        }

        let out_msg_queue_info = ok!(<_>::load_from(slice));

        let accounts = ok!(Lazy::load_from(slice));

        let child_slice = &mut ok!(slice.load_reference_as_slice());

        let global_id = ok!(slice.load_u32()) as i32;
        let shard_ident = ok!(ShardIdent::load_from(slice));

        Ok(Self {
            global_id,
            shard_ident,
            seqno: ok!(slice.load_u32()),
            vert_seqno: ok!(slice.load_u32()),
            gen_utime: ok!(slice.load_u32()),
            gen_lt: ok!(slice.load_u64()),
            min_ref_mc_seqno: ok!(slice.load_u32()),
            before_split: ok!(slice.load_bit()),
            accounts,
            overload_history: ok!(child_slice.load_u64()),
            underload_history: ok!(child_slice.load_u64()),
            total_balance: ok!(CurrencyCollection::load_from(child_slice)),
            total_validator_fees: ok!(CurrencyCollection::load_from(child_slice)),
            libraries: ok!(Dict::load_from(child_slice)),
            master_ref: ok!(Option::<BlockRef>::load_from(child_slice)),
            out_msg_queue_info,
            #[allow(unused_labels)]
            custom: ok!(Option::<Lazy<McStateExtra>>::load_from(slice)),
        })
    }
}

/// Next indivisible states after shard split.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[tlb(tag = "#5f327da5")]
pub struct ShardStateSplit {
    /// Reference to the state of the left shard.
    pub left: Lazy<ShardStateUnsplit>,
    /// Reference to the state of the right shard.
    pub right: Lazy<ShardStateUnsplit>,
}

/// Shared libraries currently can be present only in masterchain blocks.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LibDescr {
    /// Library code.
    pub lib: Cell,
    /// Accounts in the masterchain that store this library.
    pub publishers: Dict<HashBytes, ()>,
}

impl Store for LibDescr {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        ok!(builder.store_small_uint(0, 2));
        ok!(builder.store_reference(self.lib.clone()));
        match self.publishers.root() {
            Some(root) => builder.store_slice(root.as_slice_allow_exotic()),
            None => Err(Error::InvalidData),
        }
    }
}

impl<'a> Load<'a> for LibDescr {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if ok!(slice.load_small_uint(2)) != 0 {
            return Err(Error::InvalidTag);
        }
        Ok(Self {
            lib: ok!(slice.load_reference_cloned()),
            publishers: ok!(Dict::load_from_root_ext(slice, Cell::empty_context())),
        })
    }
}
