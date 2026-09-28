//! Blockchain config and params.

pub use self::params::*;
use crate::cell::*;
use crate::dict::{Dict, DictKey};
use crate::error::Error;
use crate::models::currency::ExtraCurrencyCollection;
use crate::models::global_version::GlobalVersion;
use crate::num::Tokens;

mod params;

#[cfg(test)]
mod tests;

/// Blockchain config.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockchainConfig {
    /// Configuration contract address.
    pub address: HashBytes,
    /// Configuration parameters.
    pub params: BlockchainConfigParams,
}

impl BlockchainConfig {
    /// Creates a new blockchain config with only the address set.
    pub fn new_empty(address: HashBytes) -> Self {
        let mut params = BlockchainConfigParams(Dict::new());
        params
            .set_raw(ConfigParam0::ID, CellBuilder::build_from(address).unwrap())
            .unwrap();

        Self { address, params }
    }
}

impl std::ops::Deref for BlockchainConfig {
    type Target = BlockchainConfigParams;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.params
    }
}

impl std::ops::DerefMut for BlockchainConfig {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.params
    }
}

/// A non-empty dictionary with blockchain config params.
#[derive(Debug, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct BlockchainConfigParams(Dict<u32, Cell>);

impl BlockchainConfigParams {
    /// Creates a dictionary from a raw cell.
    ///
    /// NOTE: Root is mandatory since the configs dictionary can't be empty.
    pub fn from_raw(dict_root: Cell) -> Self {
        Self(Dict::from_raw(Some(dict_root)))
    }

    /// Returns the elector account address (in masterchain).
    ///
    /// Uses [`ConfigParam1`].
    pub fn get_elector_address(&self) -> Result<HashBytes, Error> {
        ok!(self.get::<ConfigParam1>()).ok_or(Error::CellUnderflow)
    }

    /// Updates the elector account address (in masterchain).
    ///
    /// Uses [`ConfigParam1`].
    pub fn set_elector_address(&mut self, address: &HashBytes) -> Result<bool, Error> {
        self.set_raw(ConfigParam1::ID, ok!(CellBuilder::build_from(address)))
    }

    /// Returns the minter account address (in masterchain).
    ///
    /// Uses [`ConfigParam2`] with a fallback to [`ConfigParam0`] (config).
    pub fn get_minter_address(&self) -> Result<HashBytes, Error> {
        match ok!(self.get::<ConfigParam2>()) {
            Some(address) => Ok(address),
            None => ok!(self.get::<ConfigParam0>()).ok_or(Error::CellUnderflow),
        }
    }

    /// Updates the minter account address (in masterchain).
    ///
    /// Uses [`ConfigParam2`].
    pub fn set_minter_address(&mut self, address: &HashBytes) -> Result<bool, Error> {
        self.set_raw(ConfigParam2::ID, ok!(CellBuilder::build_from(address)))
    }

    /// Returns the burning config.
    ///
    /// Uses [`ConfigParam5`].
    pub fn get_burning_config(&self) -> Result<BurningConfig, Error> {
        ok!(self.get::<ConfigParam5>()).ok_or(Error::CellUnderflow)
    }

    /// Updates the burning config.
    ///
    /// Uses [`ConfigParam5`].
    pub fn set_burning_config(&mut self, config: &BurningConfig) -> Result<bool, Error> {
        self.set_raw(ConfigParam5::ID, ok!(CellBuilder::build_from(config)))
    }

    /// Returns the fee collector account address (in masterchain).
    ///
    /// Uses [`ConfigParam3`] with a fallback to [`ConfigParam1`] (elector).
    pub fn get_fee_collector_address(&self) -> Result<HashBytes, Error> {
        match ok!(self.get::<ConfigParam3>()) {
            Some(address) => Ok(address),
            None => ok!(self.get::<ConfigParam1>()).ok_or(Error::CellUnderflow),
        }
    }

    /// Updates the fee collector address (in masterchain).
    ///
    /// Uses [`ConfigParam3`].
    pub fn set_fee_collector_address(&mut self, address: &HashBytes) -> Result<bool, Error> {
        self.set_raw(ConfigParam3::ID, ok!(CellBuilder::build_from(address)))
    }

    /// Returns the lowest supported block version and required capabilities.
    ///
    /// Uses [`ConfigParam8`].
    pub fn get_global_version(&self) -> Result<GlobalVersion, Error> {
        ok!(self.get::<ConfigParam8>()).ok_or(Error::CellUnderflow)
    }

    /// Updates the global version.
    ///
    /// Uses [`ConfigParam8`].
    pub fn set_global_version(&mut self, version: &GlobalVersion) -> Result<bool, Error> {
        self.set_raw(ConfigParam8::ID, ok!(CellBuilder::build_from(version)))
    }

    /// Returns a list of params that must be present in config.
    ///
    /// Uses [`ConfigParam9`].
    pub fn get_mandatory_params(&self) -> Result<Dict<u32, ()>, Error> {
        ok!(self.get::<ConfigParam9>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a list of params that must be present in config.
    ///
    /// Uses [`ConfigParam9`].
    pub fn set_mandatory_params(&mut self, params: &[u32]) -> Result<bool, Error> {
        let mut dict = Dict::new();
        for id in params {
            ok!(dict.set(*id, ()));
        }
        self.set_raw(
            ConfigParam9::ID,
            ok!(CellBuilder::build_from(NonEmptyDict(dict))),
        )
    }

    /// Returns a list of params that have a different set of update requirements.
    ///
    /// Uses [`ConfigParam10`].
    pub fn get_critical_params(&self) -> Result<Dict<u32, ()>, Error> {
        ok!(self.get::<ConfigParam10>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a list of params that have a different set of update requirements.
    ///
    /// Uses [`ConfigParam10`].
    pub fn set_critical_params(&mut self, params: &[u32]) -> Result<bool, Error> {
        let mut dict = Dict::new();
        for id in params {
            ok!(dict.set(*id, ()));
        }
        self.set_raw(
            ConfigParam10::ID,
            ok!(CellBuilder::build_from(NonEmptyDict(dict))),
        )
    }

    /// Returns a dictionary with workchain descriptions.
    ///
    /// Uses [`ConfigParam12`].
    pub fn get_workchains(&self) -> Result<Dict<i32, WorkchainDescription>, Error> {
        ok!(self.get::<ConfigParam12>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a list of workchain descriptions.
    ///
    /// Uses [`ConfigParam12`].
    pub fn set_workchains(
        &mut self,
        workchains: &Dict<i32, WorkchainDescription>,
    ) -> Result<bool, Error> {
        self.set_raw(ConfigParam12::ID, ok!(CellBuilder::build_from(workchains)))
    }

    /// Returns a block creation reward for the specified workchain in tokens.
    ///
    /// Uses [`ConfigParam14`].
    pub fn get_block_creation_reward(&self, masterchain: bool) -> Result<Tokens, Error> {
        let rewards = ok!(self.get_block_creation_rewards());
        Ok(if masterchain {
            rewards.masterchain_block_fee
        } else {
            rewards.basechain_block_fee
        })
    }

    /// Returns a block creation rewards in tokens.
    ///
    /// Uses [`ConfigParam14`].
    pub fn get_block_creation_rewards(&self) -> Result<BlockCreationRewards, Error> {
        ok!(self.get::<ConfigParam14>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a block creation rewards in tokens.
    ///
    /// Uses [`ConfigParam14`].
    pub fn set_block_creation_rewards(
        &mut self,
        rewards: &BlockCreationRewards,
    ) -> Result<bool, Error> {
        self.set_raw(ConfigParam14::ID, ok!(CellBuilder::build_from(rewards)))
    }

    /// Returns election timings.
    ///
    /// Uses [`ConfigParam15`].
    pub fn get_election_timings(&self) -> Result<ElectionTimings, Error> {
        ok!(self.get::<ConfigParam15>()).ok_or(Error::CellUnderflow)
    }

    /// Updates election timings.
    ///
    /// Uses [`ConfigParam15`].
    pub fn set_election_timings(&mut self, timings: &ElectionTimings) -> Result<bool, Error> {
        self.set_raw(ConfigParam15::ID, ok!(CellBuilder::build_from(timings)))
    }

    /// Returns possible validator count.
    ///
    /// Uses [`ConfigParam16`].
    pub fn get_validator_count_params(&self) -> Result<ValidatorCountParams, Error> {
        ok!(self.get::<ConfigParam16>()).ok_or(Error::CellUnderflow)
    }

    /// Updates possible validator count.
    ///
    /// Uses [`ConfigParam16`].
    pub fn set_validator_count_params(
        &mut self,
        params: &ValidatorCountParams,
    ) -> Result<bool, Error> {
        self.set_raw(ConfigParam16::ID, ok!(CellBuilder::build_from(params)))
    }

    /// Returns validator stake range and factor.
    ///
    /// Uses [`ConfigParam17`].
    pub fn get_validator_stake_params(&self) -> Result<ValidatorStakeParams, Error> {
        ok!(self.get::<ConfigParam17>()).ok_or(Error::CellUnderflow)
    }

    /// Updates validator stake range and factor.
    ///
    /// Uses [`ConfigParam17`].
    pub fn set_validator_stake_params(
        &mut self,
        params: &ValidatorStakeParams,
    ) -> Result<bool, Error> {
        self.set_raw(ConfigParam17::ID, ok!(CellBuilder::build_from(params)))
    }

    /// Returns a list with a history of all storage prices.
    ///
    /// Uses [`ConfigParam18`].
    pub fn get_storage_prices(&self) -> Result<Dict<u32, StoragePrices>, Error> {
        ok!(self.get::<ConfigParam18>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a list with a history of all storage prices.
    ///
    /// Uses [`ConfigParam18`].
    pub fn set_storage_prices(&mut self, prices: &[StoragePrices]) -> Result<bool, Error> {
        let mut dict = Dict::<u32, StoragePrices>::new();
        for (i, prices) in prices.iter().enumerate() {
            ok!(dict.set(i as u32, *prices));
        }
        self.set_raw(
            ConfigParam18::ID,
            ok!(CellBuilder::build_from(NonEmptyDict(dict))),
        )
    }

    /// Returns a global id.
    pub fn get_global_id(&self) -> Result<i32, Error> {
        ok!(self.get::<ConfigParam19>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a global id.
    pub fn set_global_id(&mut self, global_id: i32) -> Result<bool, Error> {
        self.set_raw(
            ConfigParam19::ID,
            ok!(CellBuilder::build_from(global_id as u32)),
        )
    }

    /// Returns gas limits and prices.
    ///
    /// Uses [`ConfigParam20`] (for masterchain) or [`ConfigParam21`] (for other workchains).
    pub fn get_gas_prices(&self, masterchain: bool) -> Result<GasLimitsPrices, Error> {
        ok!(if masterchain {
            self.get::<ConfigParam20>()
        } else {
            self.get::<ConfigParam21>()
        })
        .ok_or(Error::CellUnderflow)
    }

    /// Updates gas limits and prices.
    ///
    /// Uses [`ConfigParam20`] (for masterchain) or [`ConfigParam21`] (for other workchains).
    pub fn set_gas_prices(
        &mut self,
        masterchain: bool,
        prices: &GasLimitsPrices,
    ) -> Result<bool, Error> {
        let id = if masterchain {
            ConfigParam20::ID
        } else {
            ConfigParam21::ID
        };
        self.set_raw(id, ok!(CellBuilder::build_from(prices)))
    }

    /// Returns block limits.
    ///
    /// Uses [`ConfigParam22`] (for masterchain) or [`ConfigParam23`] (for other workchains).
    pub fn get_block_limits(&self, masterchain: bool) -> Result<BlockLimits, Error> {
        ok!(if masterchain {
            self.get::<ConfigParam22>()
        } else {
            self.get::<ConfigParam23>()
        })
        .ok_or(Error::CellUnderflow)
    }

    /// Updates block limits.
    ///
    /// Uses [`ConfigParam22`] (for masterchain) or [`ConfigParam23`] (for other workchains).
    pub fn set_block_limits(
        &mut self,
        masterchain: bool,
        limits: &BlockLimits,
    ) -> Result<bool, Error> {
        let id = if masterchain {
            ConfigParam22::ID
        } else {
            ConfigParam23::ID
        };
        self.set_raw(id, ok!(CellBuilder::build_from(limits)))
    }

    /// Returns message forwarding prices.
    ///
    /// Uses [`ConfigParam24`] (for masterchain) or [`ConfigParam25`] (for other workchains).
    pub fn get_msg_forward_prices(&self, masterchain: bool) -> Result<MsgForwardPrices, Error> {
        ok!(if masterchain {
            self.get::<ConfigParam24>()
        } else {
            self.get::<ConfigParam25>()
        })
        .ok_or(Error::CellUnderflow)
    }

    /// Updates message forwarding prices.
    ///
    /// Uses [`ConfigParam24`] (for masterchain) or [`ConfigParam25`] (for other workchains).
    pub fn set_msg_forward_prices(
        &mut self,
        masterchain: bool,
        prices: &MsgForwardPrices,
    ) -> Result<bool, Error> {
        let id = if masterchain {
            ConfigParam24::ID
        } else {
            ConfigParam25::ID
        };
        self.set_raw(id, ok!(CellBuilder::build_from(prices)))
    }

    /// Returns a catchain config.
    ///
    /// Uses [`ConfigParam28`].
    pub fn get_catchain_config(&self) -> Result<CatchainConfig, Error> {
        ok!(self.get::<ConfigParam28>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a catchain config.
    ///
    /// Uses [`ConfigParam28`].
    pub fn set_catchain_config(&mut self, config: &CatchainConfig) -> Result<bool, Error> {
        self.set_raw(ConfigParam28::ID, ok!(CellBuilder::build_from(config)))
    }

    /// Returns a consensus config.
    ///
    /// Uses [`ConfigParam29`].
    pub fn get_consensus_config(&self) -> Result<ConsensusConfig, Error> {
        ok!(self.get::<ConfigParam29>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a consensus config.
    ///
    /// Uses [`ConfigParam29`].
    pub fn set_consensus_config(&mut self, config: &ConsensusConfig) -> Result<bool, Error> {
        self.set_raw(ConfigParam29::ID, ok!(CellBuilder::build_from(config)))
    }

    /// Returns a list of fundamental account addresses (in masterchain).
    ///
    /// Uses [`ConfigParam31`].
    pub fn get_fundamental_addresses(&self) -> Result<Dict<HashBytes, ()>, Error> {
        ok!(self.get::<ConfigParam31>()).ok_or(Error::CellUnderflow)
    }

    /// Updates a list of fundamental account addresses (in masterchain).
    ///
    /// Uses [`ConfigParam31`].
    pub fn set_fundamental_addresses(&mut self, addresses: &[HashBytes]) -> Result<bool, Error> {
        let mut dict = Dict::<HashBytes, ()>::new();
        for address in addresses {
            ok!(dict.set(*address, ()));
        }
        self.set_raw(ConfigParam31::ID, ok!(CellBuilder::build_from(dict)))
    }

    /// Returns `true` if the config contains info about the previous validator set.
    ///
    /// Uses [`ConfigParam32`] or [`ConfigParam33`].
    pub fn contains_prev_validator_set(&self) -> Result<bool, Error> {
        Ok(ok!(self.contains::<ConfigParam32>()) || ok!(self.contains::<ConfigParam33>()))
    }

    /// Returns `true` if the config contains info about the next validator set.
    ///
    /// Uses [`ConfigParam36`] or [`ConfigParam37`].
    pub fn contains_next_validator_set(&self) -> Result<bool, Error> {
        Ok(ok!(self.contains::<ConfigParam36>()) || ok!(self.contains::<ConfigParam37>()))
    }

    /// Returns the previous validator set.
    ///
    /// Uses [`ConfigParam33`] (temp prev validators) or [`ConfigParam32`] (prev validators).
    pub fn get_previous_validator_set(&self) -> Result<Option<ValidatorSet>, Error> {
        match ok!(self.get::<ConfigParam33>()) {
            None => self.get::<ConfigParam32>(),
            set => Ok(set),
        }
    }

    /// Returns the current validator set.
    ///
    /// Uses [`ConfigParam35`] (temp validators) or [`ConfigParam34`] (current validators).
    pub fn get_current_validator_set(&self) -> Result<ValidatorSet, Error> {
        match ok!(self.get::<ConfigParam35>()) {
            Some(set) => Ok(set),
            None => ok!(self.get::<ConfigParam34>()).ok_or(Error::CellUnderflow),
        }
    }

    /// Returns the next validator set.
    ///
    /// Uses [`ConfigParam37`] (temp next validators) or [`ConfigParam36`] (next validators).
    pub fn get_next_validator_set(&self) -> Result<Option<ValidatorSet>, Error> {
        match ok!(self.get::<ConfigParam37>()) {
            None => self.get::<ConfigParam36>(),
            set => Ok(set),
        }
    }

    /// Returns size limits.
    pub fn get_size_limits(&self) -> Result<SizeLimitsConfig, Error> {
        ok!(self.get::<ConfigParam43>()).ok_or(Error::CellUnderflow)
    }

    /// Updates size limits.
    pub fn set_size_limits(&mut self, size_limits: &SizeLimitsConfig) -> Result<bool, Error> {
        self.set_raw(ConfigParam43::ID, ok!(CellBuilder::build_from(size_limits)))
    }

    /// Returns authority marks config.
    pub fn get_authority_marks_config(&self) -> Result<AuthorityMarksConfig, Error> {
        ok!(self.get::<ConfigParam100>()).ok_or(Error::CellUnderflow)
    }

    /// Updates authority marks config.
    pub fn set_authority_marks_config(
        &mut self,
        params: &AuthorityMarksConfig,
    ) -> Result<bool, Error> {
        self.set_raw(ConfigParam100::ID, ok!(CellBuilder::build_from(params)))
    }

    /// Returns `true` if the config contains a param for the specified id.
    pub fn contains<'a, T: KnownConfigParam<'a>>(&'a self) -> Result<bool, Error> {
        self.0.contains_key(T::ID)
    }

    /// Returns `true` if the config contains a param for the specified id.
    pub fn contains_raw(&self, id: u32) -> Result<bool, Error> {
        self.0.contains_key(id)
    }

    /// Tries to get a parameter from the blockchain config.
    pub fn get<'a, T: KnownConfigParam<'a>>(&'a self) -> Result<Option<T::Value>, Error> {
        let Some(mut slice) = ok!(self.get_raw(T::ID)) else {
            return Ok(None);
        };
        match <T::Wrapper as Load<'a>>::load_from(&mut slice) {
            Ok(wrapped) => Ok(Some(wrapped.into_inner())),
            Err(e) => Err(e),
        }
    }

    /// Tries to update a parameter in the blockchain config.
    pub fn set<'a, T: KnownConfigParam<'a>>(&'a mut self, value: &T::Value) -> Result<bool, Error> {
        let value = ok!(CellBuilder::build_from(T::Wrapper::wrap_inner(value)));
        self.set_raw(T::ID, value)
    }

    /// Tries to get a raw parameter from the blockchain config (as slice).
    pub fn get_raw(&self, id: u32) -> Result<Option<CellSlice<'_>>, Error> {
        match ok!(self.get_raw_cell_ref(id)) {
            Some(cell) => cell.as_slice().map(Some),
            None => Ok(None),
        }
    }

    /// Tries to get a raw parameter from the blockchain config (as cell).
    pub fn get_raw_cell(&self, id: u32) -> Result<Option<Cell>, Error> {
        match ok!(self.0.get_raw(id)) {
            Some(slice) => match slice.get_reference_cloned(0) {
                Ok(cell) => Ok(Some(cell)),
                Err(e) => Err(e),
            },
            None => Ok(None),
        }
    }

    /// Tries to get a raw parameter from the blockchain config (as cell ref).
    pub fn get_raw_cell_ref(&self, id: u32) -> Result<Option<&'_ DynCell>, Error> {
        match ok!(self.0.get_raw(id)) {
            Some(slice) => match slice.get_reference(0) {
                Ok(cell) => Ok(Some(cell)),
                Err(e) => Err(e),
            },
            None => Ok(None),
        }
    }

    /// Tries to set a parameter in the blockchain config.
    ///
    /// NOTE: Use with caution, as it doesn't check the value structure.
    pub fn set_raw(&mut self, id: u32, value: Cell) -> Result<bool, Error> {
        self.0.set(id, value)
    }

    /// Removes a parameter from the blockchain config.
    ///
    /// NOTE: Removing a zero parameter successfully does nothing
    /// and returns `None` as it is required and externally managed.
    pub fn remove(&mut self, id: u32) -> Result<Option<Cell>, Error> {
        if id == 0 {
            return Ok(None);
        }
        self.0.remove(id)
    }

    /// Returns a reference to the underlying dictionary.
    pub fn as_dict(&self) -> &Dict<u32, Cell> {
        &self.0
    }
}

impl Store for BlockchainConfigParams {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        match self.0.root() {
            Some(root) => builder.store_reference(root.clone()),
            None => Err(Error::InvalidData),
        }
    }
}

impl<'a> Load<'a> for BlockchainConfigParams {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let root = ok!(slice.load_reference_cloned());
        Ok(Self(Dict::from(Some(root))))
    }
}

/// Marker trait which is implemented for known config params.
pub trait KnownConfigParam<'a> {
    /// Parameter index in a configuration dictionary.
    const ID: u32;

    /// Associated value type.
    type Value;

    /// Value wrapper.
    type Wrapper: ConfigParamWrapper<Self::Value> + Store + Load<'a>;
}

/// Trait to customize config param representation.
pub trait ConfigParamWrapper<T> {
    /// Converts this wrapper into an underlying type.
    fn into_inner(self) -> T;

    /// Converts an inner type into a wrapper.
    fn wrap_inner(inner: &T) -> &Self;
}

/// Identity wrapper for [`ConfigParamWrapper`].
#[repr(transparent)]
pub struct ParamIdentity<T>(T);

impl<T> ConfigParamWrapper<T> for ParamIdentity<T> {
    #[inline]
    fn into_inner(self) -> T {
        self.0
    }

    #[inline]
    fn wrap_inner(inner: &T) -> &Self {
        // SAFETY: `ParamIdentity` is a transparent wrapper.
        unsafe { &*(inner as *const T).cast() }
    }
}

impl<'a, T: Load<'a>> Load<'a> for ParamIdentity<T> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match T::load_from(slice) {
            Ok(value) => Ok(Self(value)),
            Err(e) => Err(e),
        }
    }
}

impl<T: Store> Store for ParamIdentity<T> {
    #[inline]
    fn store_into(&self, builder: &mut CellBuilder, cx: &dyn CellContext) -> Result<(), Error> {
        self.0.store_into(builder, cx)
    }
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for ParamIdentity<T> {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for ParamIdentity<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self)
    }
}

/// Dict wrapper for [`ConfigParamWrapper`] for parsing non-empty dictionaries.
#[repr(transparent)]
pub struct NonEmptyDict<T>(T);

impl<T> ConfigParamWrapper<T> for NonEmptyDict<T> {
    fn into_inner(self) -> T {
        self.0
    }

    #[inline]
    fn wrap_inner(inner: &T) -> &Self {
        // SAFETY: `NonEmptyDict` is a transparent wrapper.
        unsafe { &*(inner as *const T).cast() }
    }
}

impl<'a, K, V> Load<'a> for NonEmptyDict<Dict<K, V>>
where
    K: DictKey,
{
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match Dict::load_from_root_ext(slice, Cell::empty_context()) {
            Ok(value) => Ok(Self(value)),
            Err(e) => Err(e),
        }
    }
}

impl<K, V> Store for NonEmptyDict<Dict<K, V>> {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        match self.0.root() {
            Some(root) => builder.store_slice(ok!(root.as_slice())),
            None => Err(Error::InvalidData),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NonEmptyDict<Dict<u32, ()>> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(None)?;
        for entry in self.0.iter() {
            match entry {
                Ok((key, _)) => ok!(seq.serialize_element(&key)),
                Err(e) => return Err(serde::ser::Error::custom(e)),
            }
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NonEmptyDict<Dict<u32, ()>> {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        let parsed = ok!(Vec::<u32>::deserialize(deserializer));
        if parsed.is_empty() {
            return Err(Error::custom("dictionary is empty"));
        }

        let mut res = Dict::new();
        for item in parsed {
            ok!(res.set(item, ()).map_err(Error::custom));
        }

        Ok(Self(res))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NonEmptyDict<Dict<u32, StoragePrices>> {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NonEmptyDict<Dict<u32, StoragePrices>> {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let res = ok!(Dict::deserialize(deserializer));
        if res.is_empty() {
            return Err(serde::de::Error::custom("dictionary is empty"));
        }
        Ok(Self(res))
    }
}

macro_rules! define_config_params {
    ($(
        $(#[doc = $doc:expr])*
        $(#[cfg($($cfg_attrs:tt)*)])?
        $(#[serde($($serde_attrs:tt)*)])?
        $id:literal => $ident:ident($($ty:tt)*)
    ),*$(,)?) => {
        $(
            $(#[cfg($($cfg_attrs)*)])?
            $(#[doc = $doc])*
            pub struct $ident;

            $(#[cfg($($cfg_attrs)*)])?
            impl<'a> KnownConfigParam<'a> for $ident {
                const ID: u32 = $id;

                define_config_params!(@wrapper $($ty)*);
            }
        )*

        #[cfg(feature = "serde")]
        impl serde::Serialize for BlockchainConfigParams {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::{Error, SerializeMap};

                let dict = &self.0;
                if !serializer.is_human_readable() {
                    return crate::boc::BocRepr::serialize(dict, serializer);
                }

                let mut map = serializer.serialize_map(None)?;

                for entry in dict.iter() {
                    let (key, value) = match entry {
                        Ok(entry) => entry,
                        Err(e) => return Err(Error::custom(e)),
                    };

                    {
                        match key {
                            $(
                                $(#[cfg($($cfg_attrs)*)])?
                                $($id => {
                                    let value = define_config_params!(
                                        @ser
                                        $ident,
                                        value,
                                        $($serde_attrs)*
                                    );
                                    ok!(map.serialize_entry(&key, &value));
                                },)?
                            )*
                            _ => ok!(map.serialize_entry(&key, value.as_ref())),
                        }
                    }
                }

                map.end()
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for BlockchainConfigParams {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::{Error, Visitor, MapAccess};

                #[derive(serde::Deserialize)]
                struct RawValue(#[serde(with = "crate::boc::Boc")] Cell);

                struct MapVisitor;

                impl<'de> Visitor<'de> for MapVisitor {
                    type Value = BlockchainConfigParams;

                    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                        f.write_str("a config params map")
                    }

                    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
                    where
                        M: MapAccess<'de>,
                    {
                        let mut dict = Dict::new();

                        while let Some(key) = access.next_key::<u32>()? {
                            let value = match key {
                                $(
                                    $(#[cfg($($cfg_attrs)*)])?
                                    $($id => define_config_params!(
                                        @de
                                        $ident,
                                        access,
                                        $($serde_attrs)*
                                    ),)?
                                )*
                                _ => {
                                    let RawValue(cell) = ok!(access.next_value());
                                    cell
                                }
                            };
                            ok!(dict.set(key, value).map_err(Error::custom));
                        }

                        Ok(BlockchainConfigParams(dict))
                    }
                }

                if deserializer.is_human_readable() {
                    deserializer.deserialize_map(MapVisitor)
                } else {
                    crate::boc::BocRepr::deserialize(deserializer)
                }
            }
        }
    };

    (@wrapper $wrapper:ident => $($ty:tt)*) => {
        type Value = $($ty)*;
        type Wrapper = $wrapper<Self::Value>;
    };
    (@wrapper $($ty:tt)*) => {
        type Value = $($ty)*;
        type Wrapper = ParamIdentity<Self::Value>;
    };

    (@ser $ident:ident, $value:ident, transparent) => {{
        ok!($value.parse::<<$ident as KnownConfigParam>::Wrapper>().map_err(Error::custom))
    }};
    (@ser $ident:ident, $value:ident, remote = $remote:ident) => {{
        let value = ok!($value.parse::<<$ident as KnownConfigParam>::Wrapper>().map_err(Error::custom));
        $remote(<<$ident as KnownConfigParam>::Wrapper>::into_inner(value))
    }};

    (@de $ident:ident, $access:ident, transparent) => {{
        let parsed = ok!($access.next_value::<<$ident as KnownConfigParam>::Wrapper>());
        ok!(CellBuilder::build_from(&parsed).map_err(Error::custom))
    }};
    (@de $ident:ident, $access:ident, remote = $remote:ident) => {{
        let parsed = ok!($access.next_value::<$remote>());
        ok!(CellBuilder::build_from(&parsed.0).map_err(Error::custom))
    }};
}

define_config_params! {
    /// Configuration account address (in masterchain).
    #[serde(transparent)]
    0 => ConfigParam0(HashBytes),

    /// Elector account address (in masterchain).
    #[serde(transparent)]
    1 => ConfigParam1(HashBytes),

    /// Minter account address (in masterchain).
    #[serde(transparent)]
    2 => ConfigParam2(HashBytes),

    /// Fee collector account address (in masterchain).
    #[serde(transparent)]
    3 => ConfigParam3(HashBytes),

    /// DNS root account address (in masterchain).
    #[serde(transparent)]
    4 => ConfigParam4(HashBytes),

    /// Burning config.
    #[serde(transparent)]
    5 => ConfigParam5(BurningConfig),

    /// Mint new price and mint add price (unused).
    6 => ConfigParam6(CellSlice<'a>),

    /// Target amount of minted extra currencies.
    #[serde(transparent)]
    7 => ConfigParam7(ExtraCurrencyCollection),

    /// The lowest supported block version and required capabilities.
    ///
    /// Contains a [`GlobalVersion`].
    #[serde(transparent)]
    8 => ConfigParam8(GlobalVersion),

    /// Params that must be present in config.
    #[serde(transparent)]
    9 => ConfigParam9(NonEmptyDict => Dict<u32, ()>),

    /// Params that have a different set of update requirements.
    #[serde(transparent)]
    10 => ConfigParam10(NonEmptyDict => Dict<u32, ()>),

    /// Config voting setup params.
    ///
    /// Contains a [`ConfigVotingSetup`].
    #[serde(transparent)]
    11 => ConfigParam11(ConfigVotingSetup),

    /// Known workchain descriptions.
    ///
    /// Contains a dictionary with workchain id as key and [`WorkchainDescription`] as value.
    #[serde(transparent)]
    12 => ConfigParam12(Dict<i32, WorkchainDescription>),

    /// Complaint pricing.
    13 => ConfigParam13(CellSlice<'a>),

    /// Block creation reward for masterchain and basechain.
    ///
    /// Contains a [`BlockCreationRewards`].
    #[serde(transparent)]
    14 => ConfigParam14(BlockCreationRewards),

    /// Validators election timings.
    ///
    /// Contains [`ElectionTimings`].
    #[serde(transparent)]
    15 => ConfigParam15(ElectionTimings),

    /// Range of number of validators.
    ///
    /// Contains a [`ValidatorCountParams`].
    #[serde(transparent)]
    16 => ConfigParam16(ValidatorCountParams),

    /// Validator stake range and factor.
    ///
    /// Contains [`ValidatorStakeParams`]
    #[serde(transparent)]
    17 => ConfigParam17(ValidatorStakeParams),

    /// Storage prices for different intervals of time.
    ///
    /// Contains a dictionary with a history of all [`StoragePrices`].
    #[serde(transparent)]
    18 => ConfigParam18(NonEmptyDict => Dict<u32, StoragePrices>),

    /// Global ID.
    #[serde(transparent)]
    19 => ConfigParam19(i32),

    /// Masterchain gas limits and prices.
    ///
    /// Contains [`GasLimitsPrices`].
    #[serde(transparent)]
    20 => ConfigParam20(GasLimitsPrices),

    /// Base workchain gas limits and prices.
    ///
    /// Contains [`GasLimitsPrices`].
    #[serde(transparent)]
    21 => ConfigParam21(GasLimitsPrices),

    /// Masterchain block limits.
    ///
    /// Contains [`BlockLimits`].
    #[serde(transparent)]
    22 => ConfigParam22(BlockLimits),

    /// Base workchain block limits.
    ///
    /// Contains [`BlockLimits`].
    #[serde(transparent)]
    23 => ConfigParam23(BlockLimits),

    /// Message forwarding prices for masterchain.
    ///
    /// Contains [`MsgForwardPrices`].
    #[serde(transparent)]
    24 => ConfigParam24(MsgForwardPrices),

    /// Message forwarding prices for base workchain.
    ///
    /// Contains [`MsgForwardPrices`].
    #[serde(transparent)]
    25 => ConfigParam25(MsgForwardPrices),

    /// Catchain configuration params.
    ///
    /// Contains a [`CatchainConfig`].
    #[serde(transparent)]
    28 => ConfigParam28(CatchainConfig),

    /// Consensus configuration params.
    ///
    /// Contains a [`ConsensusConfig`].
    #[serde(transparent)]
    29 => ConfigParam29(ConsensusConfig),

    /// Delector configuration params.
    30 => ConfigParam30(CellSlice<'a>),

    /// Fundamental smartcontract addresses.
    ///
    /// Contains a dictionary with addresses (in masterchain) of fundamental contracts as keys.
    #[serde(remote = HashBytesList)]
    31 => ConfigParam31(Dict<HashBytes, ()>),

    /// Previous validator set.
    ///
    /// Contains a [`ValidatorSet`].
    #[serde(transparent)]
    32 => ConfigParam32(ValidatorSet),

    /// Previous temporary validator set.
    ///
    /// Contains a [`ValidatorSet`].
    #[serde(transparent)]
    33 => ConfigParam33(ValidatorSet),

    /// Current validator set.
    ///
    /// Contains a [`ValidatorSet`].
    #[serde(transparent)]
    34 => ConfigParam34(ValidatorSet),

    /// Current temporary validator set.
    ///
    /// Contains a [`ValidatorSet`].
    #[serde(transparent)]
    35 => ConfigParam35(ValidatorSet),

    /// Next validator set.
    ///
    /// Contains a [`ValidatorSet`].
    #[serde(transparent)]
    36 => ConfigParam36(ValidatorSet),

    /// Next temporary validator set.
    ///
    /// Contains a [`ValidatorSet`].
    #[serde(transparent)]
    37 => ConfigParam37(ValidatorSet),

    /// Size limits.
    ///
    /// Contains a [`SizeLimitsConfig`].
    #[serde(transparent)]
    43 => ConfigParam43(SizeLimitsConfig),

    /// Mint once config (can be used by L2 to mint native currency).
    ///
    /// Contains a [`MintOnceConfig`].
    #[serde(transparent)]
    50 => ConfigParam50(MintOnceConfig),

    /// Authority marks config.
    ///
    /// Contains an [`AuthorityMarksConfig`].
    #[serde(transparent)]
    100 => ConfigParam100(AuthorityMarksConfig),
}

#[cfg(feature = "serde")]
#[repr(transparent)]
struct HashBytesList(Dict<HashBytes, ()>);

#[cfg(feature = "serde")]
impl serde::Serialize for HashBytesList {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(None)?;
        for entry in self.0.iter() {
            match entry {
                Ok((ref key, _)) => ok!(seq.serialize_element(key)),
                Err(e) => return Err(serde::ser::Error::custom(e)),
            }
        }
        seq.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for HashBytesList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let parsed = ok!(Vec::<HashBytes>::deserialize(deserializer));

        let mut res = Dict::new();
        for item in parsed {
            ok!(res.set(item, ()).map_err(Error::custom));
        }

        Ok(Self(res))
    }
}
