//! Currency collection stuff.

use std::collections::{BTreeMap, HashMap};

use crate::cell::*;
use crate::dict::{AugDictExtra, Dict};
use crate::error::Error;
use crate::num::{Tokens, VarUint248};

/// Amounts collection.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct CurrencyCollection {
    /// Amount in native currency.
    pub tokens: Tokens,
    /// Amounts in other currencies.
    pub other: ExtraCurrencyCollection,
}

impl Default for CurrencyCollection {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

impl CurrencyCollection {
    /// The additive identity for the currency collection
    /// (with empty extra currencies).
    pub const ZERO: Self = Self {
        tokens: Tokens::ZERO,
        other: ExtraCurrencyCollection::new(),
    };

    /// Creates a new currency collection with from the specified tokens amount
    /// and empty extra currency collection.
    pub const fn new(tokens: u128) -> Self {
        Self {
            tokens: Tokens::new(tokens),
            other: ExtraCurrencyCollection::new(),
        }
    }

    /// Returns whether balance in tokens and extra currencies is empty.
    pub fn is_zero(&self) -> bool {
        self.tokens.is_zero() && self.other.is_empty()
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        self.tokens.unwrap_bit_len() + 1
    }

    /// Checked currency collection addition.
    /// Computes `self + rhs` for each currency, returning `Err`
    /// if overflow occurred or dictionaries had invalid structure.
    pub fn checked_add(&self, other: &Self) -> Result<Self, Error> {
        Ok(Self {
            tokens: match self.tokens.checked_add(other.tokens) {
                Some(value) => value,
                None => return Err(Error::IntOverflow),
            },
            other: ok!(self.other.checked_add(&other.other)),
        })
    }

    /// Checked currency collection subtraction.
    /// Computes `self - rhs` for each currency, returning `Err`
    /// if overflow occurred or dictionaries had invalid structure.
    pub fn checked_sub(&self, other: &Self) -> Result<Self, Error> {
        Ok(Self {
            tokens: match self.tokens.checked_sub(other.tokens) {
                Some(value) => value,
                None => return Err(Error::IntOverflow),
            },
            other: ok!(self.other.checked_sub(&other.other)),
        })
    }

    /// Tries to add the specified amount of native tokens to the collection.
    pub fn try_add_assign_tokens(&mut self, other: Tokens) -> Result<(), Error> {
        match self.tokens.checked_add(other) {
            Some(value) => {
                self.tokens = value;
                Ok(())
            }
            None => Err(Error::IntOverflow),
        }
    }

    /// Tries to subtract the specified amount of native tokens from the collection.
    pub fn try_sub_assign_tokens(&mut self, other: Tokens) -> Result<(), Error> {
        match self.tokens.checked_sub(other) {
            Some(value) => {
                self.tokens = value;
                Ok(())
            }
            None => Err(Error::IntOverflow),
        }
    }

    /// Tries to add an other currency collection to the current one.
    pub fn try_add_assign(&mut self, other: &Self) -> Result<(), Error> {
        *self = ok!(self.checked_add(other));
        Ok(())
    }

    /// Tries to subtract an other currency collection from the current one.
    pub fn try_sub_assign(&mut self, other: &Self) -> Result<(), Error> {
        *self = ok!(self.checked_sub(other));
        Ok(())
    }

    /// Returns the intersection between two currency collections.
    pub fn checked_clamp(&self, other: &Self) -> Result<Self, Error> {
        Ok(Self {
            other: ok!(self.other.checked_clamp(&other.other)),
            tokens: std::cmp::min(self.tokens, other.tokens),
        })
    }
}

impl From<Tokens> for CurrencyCollection {
    #[inline]
    fn from(tokens: Tokens) -> Self {
        Self {
            tokens,
            other: ExtraCurrencyCollection::new(),
        }
    }
}

impl ExactSize for CurrencyCollection {
    #[inline]
    fn exact_size(&self) -> Size {
        self.tokens.exact_size() + self.other.exact_size()
    }
}

impl AugDictExtra for CurrencyCollection {
    fn comp_add(
        left: &mut CellSlice,
        right: &mut CellSlice,
        b: &mut CellBuilder,
        cx: &dyn CellContext,
    ) -> Result<(), Error> {
        let left = ok!(Self::load_from(left));
        let right = ok!(Self::load_from(right));
        ok!(left.checked_add(&right)).store_into(b, cx)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for CurrencyCollection {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            tokens: u.arbitrary()?,
            other: u.arbitrary()?,
        })
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Self::try_size_hint(depth).unwrap_or_default()
    }

    #[inline]
    fn try_size_hint(
        depth: usize,
    ) -> Result<(usize, Option<usize>), arbitrary::MaxRecursionReached> {
        Ok(arbitrary::size_hint::and(
            <Tokens as arbitrary::Arbitrary>::try_size_hint(depth)?,
            <ExtraCurrencyCollection as arbitrary::Arbitrary>::try_size_hint(depth)?,
        ))
    }
}

/// Dictionary with amounts for multiple currencies.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
#[repr(transparent)]
pub struct ExtraCurrencyCollection(Dict<u32, VarUint248>);

impl Default for ExtraCurrencyCollection {
    #[inline]
    fn default() -> Self {
        Self(Dict::new())
    }
}

impl ExtraCurrencyCollection {
    /// Creates an empty extra currency collection.
    pub const fn new() -> Self {
        Self(Dict::new())
    }

    /// Creates a currency collection from a raw cell.
    pub const fn from_raw(dict: Option<Cell>) -> Self {
        Self(Dict::from_raw(dict))
    }

    /// Creates an `ExtraCurrencyCollection` from an iterator of id-amount pairs.
    pub fn try_from_iter<I>(iter: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = (u32, VarUint248)>,
    {
        let mut values = iter.into_iter().collect::<Box<[_]>>();
        values.sort_unstable_by_key(|(id, _)| *id);
        Dict::try_from_sorted_slice(&values).map(Self)
    }

    /// Returns `true` if the dictionary contains no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns a reference to the underlying dictionary.
    #[inline]
    pub const fn as_dict(&self) -> &Dict<u32, VarUint248> {
        &self.0
    }

    /// Returns the underlying dictionary.
    #[inline]
    pub fn into_dict(self) -> Dict<u32, VarUint248> {
        self.0
    }

    /// Returns a mutable reference to the underlying dictionary.
    #[inline]
    pub fn as_dict_mut(&mut self) -> &mut Dict<u32, VarUint248> {
        &mut self.0
    }

    /// Removes all currencies with zero balance.
    pub fn normalized(&self) -> Result<Self, Error> {
        let mut result = self.clone();
        for entry in self.0.iter() {
            let (currency_id, other) = ok!(entry);
            if other.is_zero() {
                ok!(result.0.remove(currency_id));
            }
        }
        Ok(result)
    }

    /// Removes all currencies with zero balance.
    pub fn normalize(&mut self) -> Result<(), Error> {
        let mut result = self.clone();
        for entry in self.0.iter() {
            let (currency_id, other) = ok!(entry);
            if other.is_zero() {
                ok!(result.0.remove(currency_id));
            }
        }
        *self = result;
        Ok(())
    }

    /// Checked extra currency collection addition.
    /// Computes `self + rhs` for each currency, returning `Err`
    /// if overflow occurred or dictionaries had invalid structure.
    pub fn checked_add(&self, other: &Self) -> Result<Self, Error> {
        let mut result = self.clone();
        for entry in other.0.iter() {
            let (currency_id, other) = ok!(entry);

            let existing = ok!(result.as_dict().get(currency_id)).unwrap_or_default();
            match existing.checked_add(&other) {
                Some(value) if value.is_zero() => {
                    ok!(result.0.remove(currency_id));
                }
                Some(ref value) => {
                    ok!(result.0.set(currency_id, value));
                }
                None => return Err(Error::IntOverflow),
            };
        }
        Ok(result)
    }

    /// Checked extra currency subtraction.
    /// Computes `self - rhs` for each currency, returning `Err`
    /// if overflow occurred or dictionaries had invalid structure.
    pub fn checked_sub(&self, other: &Self) -> Result<Self, Error> {
        let mut result = self.clone();
        for entry in other.0.iter() {
            let (currency_id, other) = ok!(entry);

            let existing = ok!(result.as_dict().get(currency_id)).unwrap_or_default();
            match existing.checked_sub(&other) {
                Some(value) if value.is_zero() => {
                    ok!(result.0.remove(currency_id));
                }
                Some(ref value) => {
                    ok!(result.0.set(currency_id, value));
                }
                None => return Err(Error::IntOverflow),
            };
        }
        Ok(result)
    }

    /// Returns the intersection between two extra currency collections.
    pub fn checked_clamp(&self, other: &Self) -> Result<Self, Error> {
        let mut result = self.clone();
        for entry in self.0.iter() {
            let (currency_id, balance) = ok!(entry);
            match ok!(other.0.get(currency_id)) {
                // Other collection has this currency,
                // so we must update to the lowest balance.
                Some(other_balance) if balance > other_balance => {
                    ok!(result.0.set(currency_id, other_balance));
                }
                // Other collection doesn't have this currency,
                // and we have a non-zero amount.
                None if !balance.is_zero() => {
                    // So we must delete it.
                    ok!(result.0.remove_raw(currency_id));
                }
                // Other collection doesn't have this currency,
                // and we have a zero amount. So we can do nothing.
                _ => {}
            }
        }
        Ok(result)
    }
}

impl<S> TryFrom<&'_ HashMap<u32, VarUint248, S>> for ExtraCurrencyCollection
where
    S: std::hash::BuildHasher,
{
    type Error = Error;

    fn try_from(value: &'_ HashMap<u32, VarUint248, S>) -> Result<Self, Self::Error> {
        let mut values = value.iter().collect::<Box<[_]>>();
        values.sort_unstable_by_key(|(id, _)| *id);
        Dict::try_from_sorted_slice(&values).map(Self)
    }
}

impl<S> TryFrom<HashMap<u32, VarUint248, S>> for ExtraCurrencyCollection
where
    S: std::hash::BuildHasher,
{
    type Error = Error;

    fn try_from(value: HashMap<u32, VarUint248, S>) -> Result<Self, Self::Error> {
        let mut values = value.into_iter().collect::<Box<[_]>>();
        values.sort_unstable_by_key(|(id, _)| *id);
        Dict::try_from_sorted_slice(&values).map(Self)
    }
}

impl TryFrom<&'_ BTreeMap<u32, VarUint248>> for ExtraCurrencyCollection {
    type Error = Error;

    #[inline]
    fn try_from(value: &'_ BTreeMap<u32, VarUint248>) -> Result<Self, Self::Error> {
        Dict::try_from_btree(value).map(Self)
    }
}

impl TryFrom<BTreeMap<u32, VarUint248>> for ExtraCurrencyCollection {
    type Error = Error;

    #[inline]
    fn try_from(value: BTreeMap<u32, VarUint248>) -> Result<Self, Self::Error> {
        Self::try_from(&value)
    }
}

impl From<Dict<u32, VarUint248>> for ExtraCurrencyCollection {
    #[inline]
    fn from(value: Dict<u32, VarUint248>) -> Self {
        Self(value)
    }
}

impl From<ExtraCurrencyCollection> for Dict<u32, VarUint248> {
    #[inline]
    fn from(value: ExtraCurrencyCollection) -> Self {
        value.0
    }
}

impl ExactSize for ExtraCurrencyCollection {
    #[inline]
    fn exact_size(&self) -> Size {
        self.0.exact_size()
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ExtraCurrencyCollection {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let size = u.arbitrary::<u8>()?;
        if size <= 128 {
            Ok(Self(Dict::new()))
        } else {
            let mut dict = Dict::<u32, VarUint248>::new();
            for _ in 128..size {
                dict.set(u.arbitrary::<u32>()?, u.arbitrary::<VarUint248>()?)
                    .unwrap();
            }
            Ok(Self(dict))
        }
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (1, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Lazy;
    use crate::models::{DepthBalanceInfo, ShardAccount, ShardAccounts};

    fn _cc_must_use() -> anyhow::Result<()> {
        #[expect(unused_must_use)]
        {
            CurrencyCollection::new(10).checked_add(&CurrencyCollection::ZERO)?;
        }

        #[expect(unused_must_use)]
        {
            ExtraCurrencyCollection::new().checked_add(&ExtraCurrencyCollection::new())?;
        }

        Ok(())
    }

    #[test]
    fn cc_math() -> anyhow::Result<()> {
        let value = CurrencyCollection {
            tokens: Tokens::new(1),
            other: ExtraCurrencyCollection::try_from_iter([(1, VarUint248::new(1000))])?,
        };

        let mut new_value = CurrencyCollection::ZERO;
        new_value.try_add_assign(&value)?;
        assert_eq!(new_value, value);

        new_value.try_add_assign(&value)?;
        assert_ne!(new_value, value);

        let extra = new_value.other.as_dict().get(1)?;
        assert_eq!(extra, Some(VarUint248::new(2000)));

        Ok(())
    }

    #[test]
    fn aug_dict() -> anyhow::Result<()> {
        let mut accounts = ShardAccounts::new();
        accounts.set(
            HashBytes([0; 32]),
            DepthBalanceInfo {
                split_depth: 0,
                balance: CurrencyCollection {
                    tokens: Tokens::new(500_000_000_000),
                    other: ExtraCurrencyCollection::new(),
                },
            },
            ShardAccount {
                account: Lazy::from_raw(Cell::empty_cell())?,
                last_trans_lt: 0,
                last_trans_hash: Default::default(),
            },
        )?;

        accounts.set(
            HashBytes([1; 32]),
            DepthBalanceInfo {
                split_depth: 0,
                balance: CurrencyCollection {
                    tokens: Tokens::new(500_000_000_000),
                    other: ExtraCurrencyCollection::try_from_iter([(2, VarUint248::new(1000))])?,
                },
            },
            ShardAccount {
                account: Lazy::from_raw(Cell::empty_cell())?,
                last_trans_lt: 0,
                last_trans_hash: Default::default(),
            },
        )?;

        Ok(())
    }
}
