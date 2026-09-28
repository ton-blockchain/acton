use crate::cell::*;
use crate::dict::{AugDict, AugDictExtra};
use crate::error::*;
use crate::models::ShardAccount;
use crate::models::currency::CurrencyCollection;

/// A dictionary of account states.
pub type ShardAccounts = AugDict<HashBytes, DepthBalanceInfo, ShardAccount>;

/// Intermediate balance info.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct DepthBalanceInfo {
    /// Depth for which the balance was calculated.
    pub split_depth: u8,
    /// Total balance for a subtree.
    pub balance: CurrencyCollection,
}

impl DepthBalanceInfo {
    const SPLIT_DEPTH_BITS: u16 = 5;

    /// Returns `true` if the split depth is valid.
    #[inline]
    pub const fn is_valid(&self) -> bool {
        self.split_depth <= 30
    }
}

impl AugDictExtra for DepthBalanceInfo {
    fn comp_add(
        left: &mut CellSlice,
        right: &mut CellSlice,
        b: &mut CellBuilder,
        cx: &dyn CellContext,
    ) -> Result<(), Error> {
        let left = ok!(Self::load_from(left));
        let right = ok!(Self::load_from(right));
        Self {
            split_depth: std::cmp::max(left.split_depth, right.split_depth),
            balance: ok!(left.balance.checked_add(&right.balance)),
        }
        .store_into(b, cx)
    }
}

impl Store for DepthBalanceInfo {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        if !self.is_valid() {
            return Err(Error::IntOverflow);
        }
        ok!(builder.store_small_uint(self.split_depth, Self::SPLIT_DEPTH_BITS));
        self.balance.store_into(builder, context)
    }
}

impl<'a> Load<'a> for DepthBalanceInfo {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let result = Self {
            split_depth: ok!(slice.load_small_uint(Self::SPLIT_DEPTH_BITS)),
            balance: ok!(CurrencyCollection::load_from(slice)),
        };
        if result.is_valid() {
            Ok(result)
        } else {
            Err(Error::IntOverflow)
        }
    }
}
