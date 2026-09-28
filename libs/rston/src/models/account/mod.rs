//! Account state models.

use crate::cell::*;
use crate::dict::*;
use crate::error::*;
use crate::models::currency::CurrencyCollection;
use crate::models::message::IntAddr;
use crate::num::*;

/// Amount of unique cells and bits for shard states.
#[derive(Debug, Default, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct StorageUsed {
    /// Amount of unique cells.
    pub cells: VarUint56,
    /// The total number of bits in unique cells.
    pub bits: VarUint56,
}

impl StorageUsed {
    /// The additive identity for this type, i.e. `0`.
    pub const ZERO: Self = Self {
        cells: VarUint56::ZERO,
        bits: VarUint56::ZERO,
    };

    /// Computes a total storage usage stats.
    ///
    /// `cell_limit` is the maximum number of unique cells to visit.
    /// If the limit is reached, the function will return [`Error::Cancelled`].
    pub fn compute(account: &Account, cell_limit: usize) -> Result<Self, Error> {
        let cell = {
            let cx = Cell::empty_context();
            let mut storage = CellBuilder::new();
            storage.store_u64(account.last_trans_lt)?;
            account.balance.store_into(&mut storage, cx)?;
            account.state.store_into(&mut storage, cx)?;
            storage.build_ext(cx)?
        };

        let Some(res) = cell.compute_unique_stats(cell_limit) else {
            return Err(Error::Cancelled);
        };

        let res = Self {
            cells: VarUint56::new(res.cell_count),
            bits: VarUint56::new(res.bit_count),
        };

        if res.cells.is_valid() || !res.bits.is_valid() {
            return Err(Error::IntOverflow);
        }

        Ok(res)
    }
}

/// Amount of unique cells and bits.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct StorageUsedShort {
    /// Amount of unique cells.
    pub cells: VarUint56,
    /// The total number of bits in unique cells.
    pub bits: VarUint56,
}

impl StorageUsedShort {
    /// The additive identity for this type, i.e. `0`.
    pub const ZERO: Self = Self {
        cells: VarUint56::ZERO,
        bits: VarUint56::ZERO,
    };
}

/// Storage state extra
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum StorageExtra {
    /// Storage extra is not present.
    #[default]
    None,
    /// Storage extra is present.
    DictHash(HashBytes),
}

impl Store for StorageExtra {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        match self {
            StorageExtra::None => builder.store_zeros(3)?,
            StorageExtra::DictHash(dict_hash) => {
                builder.store_small_uint(1, 3)?;
                builder.store_u256(dict_hash)?;
            }
        }
        Ok(())
    }
}

impl<'a> Load<'a> for StorageExtra {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_small_uint(3)? {
            0b000 => Ok(Self::None),
            0b001 => Ok(Self::DictHash(slice.load_u256()?)),
            _ => Err(Error::InvalidTag),
        }
    }
}

/// Storage profile of an account.
#[derive(Debug, Default, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct StorageInfo {
    /// Amount of unique cells and bits which account state occupies.
    pub used: StorageUsed,
    /// Storage extra
    pub storage_extra: StorageExtra,
    /// Unix timestamp of the last storage phase.
    pub last_paid: u32,
    /// Account debt for storing its state.
    pub due_payment: Option<Tokens>,
}

/// Brief account status.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum AccountStatus {
    /// Account exists but has not yet been deployed.
    Uninit = 0b00,
    /// Account exists but has been frozen.
    Frozen = 0b01,
    /// Account exists and has been deployed.
    Active = 0b10,
    /// Account does not exist.
    NotExists = 0b11,
}

impl AccountStatus {
    /// The number of data bits that this struct occupies.
    pub const BITS: u16 = 2;
}

impl Store for AccountStatus {
    #[inline]
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_small_uint(*self as u8, 2)
    }
}

impl<'a> Load<'a> for AccountStatus {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_small_uint(2) {
            Ok(ty) => Ok(match ty {
                0b00 => Self::Uninit,
                0b01 => Self::Frozen,
                0b10 => Self::Active,
                0b11 => Self::NotExists,
                _ => {
                    debug_assert!(false, "unexpected small uint");
                    // SAFETY: `load_small_uint` must return 2 bits
                    unsafe { std::hint::unreachable_unchecked() }
                }
            }),
            Err(e) => Err(e),
        }
    }
}

/// Shard accounts entry.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ShardAccount {
    /// Optional reference to account state.
    pub account: Lazy<OptionalAccount>,
    /// The exact hash of the last transaction.
    pub last_trans_hash: HashBytes,
    /// The exact logical time of the last transaction.
    pub last_trans_lt: u64,
}

impl ShardAccount {
    /// Tries to load account data.
    pub fn load_account(&self) -> Result<Option<Account>, Error> {
        let OptionalAccount(account) = ok!(self.account.load());
        Ok(account)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ShardAccount {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let account = u.arbitrary::<OptionalAccount>()?;
        Ok(Self {
            account: Lazy::new(&account).unwrap(),
            last_trans_hash: u.arbitrary()?,
            last_trans_lt: u.arbitrary()?,
        })
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Self::try_size_hint(depth).unwrap_or_default()
    }

    fn try_size_hint(
        depth: usize,
    ) -> arbitrary::Result<(usize, Option<usize>), arbitrary::MaxRecursionReached> {
        Ok(arbitrary::size_hint::and_all(&[
            <OptionalAccount as arbitrary::Arbitrary>::try_size_hint(depth)?,
            <HashBytes as arbitrary::Arbitrary>::try_size_hint(depth)?,
            <u64 as arbitrary::Arbitrary>::try_size_hint(depth)?,
        ]))
    }
}

/// A wrapper for `Option<Account>` with customized representation.
#[derive(Default, Debug, Clone, Eq, PartialEq, Store, Load)]
#[repr(transparent)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OptionalAccount(pub Option<Account>);

impl OptionalAccount {
    /// Non-existing account.
    pub const EMPTY: Self = Self(None);

    /// Returns an optional account status.
    pub fn status(&self) -> AccountStatus {
        match &self.0 {
            None => AccountStatus::NotExists,
            Some(account) => account.state.status(),
        }
    }

    /// Logical time after the last transaction execution if account exists
    /// or zero otherwise.
    pub fn last_trans_lt(&self) -> u64 {
        match &self.0 {
            None => 0,
            Some(account) => account.last_trans_lt,
        }
    }

    /// Account balance for all currencies.
    #[cfg(feature = "sync")]
    pub fn balance(&self) -> &CurrencyCollection {
        static DEFAULT_VALANCE: CurrencyCollection = CurrencyCollection::ZERO;

        match &self.0 {
            None => &DEFAULT_VALANCE,
            Some(account) => &account.balance,
        }
    }

    /// Returns an account state if it exists.
    pub fn state(&self) -> Option<&AccountState> {
        Some(&self.0.as_ref()?.state)
    }
}

impl AsRef<Option<Account>> for OptionalAccount {
    #[inline]
    fn as_ref(&self) -> &Option<Account> {
        &self.0
    }
}

impl AsMut<Option<Account>> for OptionalAccount {
    #[inline]
    fn as_mut(&mut self) -> &mut Option<Account> {
        &mut self.0
    }
}

impl From<Account> for OptionalAccount {
    #[inline]
    fn from(value: Account) -> Self {
        Self(Some(value))
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for OptionalAccount {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        u.ratio(9u8, 10u8)?
            .then(|| u.arbitrary())
            .transpose()
            .map(Self)
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Self::try_size_hint(depth).unwrap_or_default()
    }

    #[inline]
    fn try_size_hint(
        depth: usize,
    ) -> arbitrary::Result<(usize, Option<usize>), arbitrary::MaxRecursionReached> {
        <Option<Account>>::try_size_hint(depth)
    }
}

/// Existing account data.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Account {
    /// Account address.
    pub address: IntAddr,
    /// Storage statistics.
    pub storage_stat: StorageInfo,
    /// Logical time after the last transaction execution.
    pub last_trans_lt: u64,
    /// Account balance for all currencies.
    pub balance: CurrencyCollection,
    /// Account state.
    pub state: AccountState,
}

/// State of an existing account.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "status"))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum AccountState {
    /// Account exists but has not yet been deployed.
    Uninit,
    /// Account exists and has been deployed.
    Active(StateInit),
    /// Account exists but has been frozen. Contains a hash of the last known [`StateInit`].
    Frozen(HashBytes),
}

impl AccountState {
    /// Returns an account status.
    pub fn status(&self) -> AccountStatus {
        match self {
            Self::Uninit => AccountStatus::Uninit,
            Self::Active(_) => AccountStatus::Active,
            Self::Frozen(_) => AccountStatus::Frozen,
        }
    }
}

impl Store for AccountState {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Uninit => builder.store_small_uint(0b00, 2),
            Self::Active(state) => {
                ok!(builder.store_bit_one());
                state.store_into(builder, context)
            }
            Self::Frozen(hash) => {
                ok!(builder.store_small_uint(0b01, 2));
                builder.store_u256(hash)
            }
        }
    }
}

impl<'a> Load<'a> for AccountState {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(if ok!(slice.load_bit()) {
            match StateInit::load_from(slice) {
                Ok(state) => Self::Active(state),
                Err(e) => return Err(e),
            }
        } else if ok!(slice.load_bit()) {
            match slice.load_u256() {
                Ok(state) => Self::Frozen(state),
                Err(e) => return Err(e),
            }
        } else {
            Self::Uninit
        })
    }
}

/// Deployed account state.
#[derive(Debug, Clone, Default, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StateInit {
    /// Optional split depth for large smart contracts.
    pub split_depth: Option<SplitDepth>,
    /// Optional special contract flags.
    pub special: Option<SpecialFlags>,
    /// Optional contract code.
    #[cfg_attr(feature = "serde", serde(with = "crate::boc::Boc"))]
    pub code: Option<Cell>,
    /// Optional contract data.
    #[cfg_attr(feature = "serde", serde(with = "crate::boc::Boc"))]
    pub data: Option<Cell>,
    /// Libraries used in smart-contract.
    pub libraries: Dict<HashBytes, SimpleLib>,
}

impl StateInit {
    /// Exact size of this value when it is stored in slice.
    pub const fn exact_size_const(&self) -> Size {
        Size {
            bits: self.bit_len(),
            refs: self.reference_count(),
        }
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        (1 + self.split_depth.is_some() as u16 * SplitDepth::BITS)
            + (1 + self.special.is_some() as u16 * SpecialFlags::BITS)
            + 3
    }

    /// Returns the number of references that this struct occupies.
    pub const fn reference_count(&self) -> u8 {
        self.code.is_some() as u8 + self.data.is_some() as u8 + !self.libraries.is_empty() as u8
    }
}

impl ExactSize for StateInit {
    #[inline]
    fn exact_size(&self) -> Size {
        self.exact_size_const()
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for StateInit {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let split_depth = u.ratio(1u8, 50u8)?.then(|| u.arbitrary()).transpose()?;
        let special = u.ratio(1u8, 50u8)?.then(|| u.arbitrary()).transpose()?;
        let code = u.ratio(9u8, 10u8)?.then(|| u.arbitrary()).transpose()?;
        let data = u.ratio(9u8, 10u8)?.then(|| u.arbitrary()).transpose()?;

        let mut libraries = Dict::new();
        match u.arbitrary::<u8>()? {
            0..=128 => {}
            n => {
                for _ in 128..n {
                    let lib = u.arbitrary::<SimpleLib>()?;
                    if lib.root.level() != 0 || lib.root.has_max_depth() {
                        return Err(arbitrary::Error::IncorrectFormat);
                    }
                    libraries.set(u.arbitrary::<HashBytes>()?, lib).unwrap();
                }
            }
        }

        Ok(Self {
            split_depth,
            special,
            code,
            data,
            libraries,
        })
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Self::try_size_hint(depth).unwrap_or_default()
    }

    fn try_size_hint(
        depth: usize,
    ) -> arbitrary::Result<(usize, Option<usize>), arbitrary::MaxRecursionReached> {
        Ok(arbitrary::size_hint::and_all(&[
            <Option<SplitDepth>>::try_size_hint(depth)?,
            <Option<SpecialFlags>>::try_size_hint(depth)?,
            <Option<Cell>>::try_size_hint(depth)?,
            <Option<Cell>>::try_size_hint(depth)?,
            (1, None),
        ]))
    }
}

/// Special transactions execution flags.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SpecialFlags {
    /// Account will be called at the beginning of each block.
    pub tick: bool,
    /// Account will be called at the end of each block.
    pub tock: bool,
}

impl SpecialFlags {
    /// The number of data bits that this struct occupies.
    pub const BITS: u16 = 2;
}

impl Store for SpecialFlags {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_small_uint(((self.tick as u8) << 1) | self.tock as u8, 2)
    }
}

impl<'a> Load<'a> for SpecialFlags {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_small_uint(2) {
            Ok(data) => Ok(Self {
                tick: data & 0b10 != 0,
                tock: data & 0b01 != 0,
            }),
            Err(e) => Err(e),
        }
    }
}

/// Simple TVM library.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct SimpleLib {
    /// Whether this library is accessible from other accounts.
    pub public: bool,
    /// Library code.
    #[cfg_attr(feature = "serde", serde(with = "crate::boc::Boc"))]
    pub root: Cell,
}
