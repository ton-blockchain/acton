use std::num::{NonZeroU16, NonZeroU32};

use tycho_crypto::ed25519;

use crate::cell::*;
use crate::dict::{Dict, build_dict_from_sorted_iter};
use crate::error::Error;
use crate::models::block::ShardIdent;
use crate::models::{CurrencyCollection, Signature};
use crate::num::{Tokens, Uint12, VarUint248};

/// Value flow burning config.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#01", validate_with = "Self::is_valid")]
pub struct BurningConfig {
    /// Address of the masterchain account which will burn all inbound message balance.
    pub blackhole_addr: Option<HashBytes>,
    /// Numerator of the potion of burned fees.
    pub fee_burn_num: u32,
    /// Denominator of the potion of burned fees.
    pub fee_burn_denom: NonZeroU32,
}

impl Default for BurningConfig {
    #[inline]
    fn default() -> Self {
        Self {
            blackhole_addr: None,
            fee_burn_num: 0,
            fee_burn_denom: NonZeroU32::MIN,
        }
    }
}

impl BurningConfig {
    /// Returns whether the config is well-formed.
    pub fn is_valid(&self) -> bool {
        self.fee_burn_num <= self.fee_burn_denom.get()
    }

    /// Computes how much fees to burn.
    ///
    /// NOTE: For a well-formed [`BurningConfig`] it never fails
    ///       and returns a value not greater than `tokens`.
    pub fn compute_burned_fees(&self, tokens: Tokens) -> Result<Tokens, Error> {
        if self.fee_burn_num == 0 {
            return Ok(Tokens::ZERO);
        } else if !self.is_valid() {
            return Err(Error::InvalidData);
        }

        let mut tokens = VarUint248::new(tokens.into_inner());
        tokens *= self.fee_burn_num as u128;
        tokens /= self.fee_burn_denom.get() as u128;
        let (hi, lo) = tokens.into_words();
        debug_assert_eq!(
            hi, 0,
            "burned fees must never be greater than original fees"
        );
        Ok(Tokens::new(lo))
    }
}

/// One-time minting config (can be used by L2 to mint native currency).
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#01")]
pub struct MintOnceConfig {
    /// Exact masterchain block seqno.
    pub mint_at: u32,
    /// Native and extra currencies to mint.
    pub delta: CurrencyCollection,
}

/// Config voting setup params.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#91")]
pub struct ConfigVotingSetup {
    /// Proposal configuration for non-critical params.
    pub normal_params: Lazy<ConfigProposalSetup>,
    /// Proposal configuration for critical params.
    pub critical_params: Lazy<ConfigProposalSetup>,
}

/// Config proposal setup params.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#36")]
pub struct ConfigProposalSetup {
    /// The minimal number of voting rounds for the proposal.
    pub min_total_rounds: u8,
    /// The maximum number of voting rounds for the proposal.
    pub max_total_rounds: u8,
    /// The minimum number of winned voting rounds.
    pub min_wins: u8,
    /// The maximum number of lost voting rounds.
    pub max_losses: u8,
    /// The minimal proposal lifetime duration in seconds.
    pub min_store_sec: u32,
    /// The maximum proposal lifetime duration in seconds.
    pub max_store_sec: u32,
    /// Bit price for storage price computation.
    pub bit_price: u32,
    /// Cell price for storage price computation.
    pub cell_price: u32,
}

/// Workchain description.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorkchainDescription {
    /// Unix timestamp from which blocks can be produced.
    pub enabled_since: u32,
    /// Unused stub.
    pub actual_min_split: u8,
    /// The minimal shards split depths.
    pub min_split: u8,
    /// The maximum shards split depths.
    pub max_split: u8,
    /// Whether the workchain is enabled.
    pub active: bool,
    /// Whether the workchain accepts messages.
    pub accept_msgs: bool,
    /// A hash of the zerostate root cell.
    pub zerostate_root_hash: HashBytes,
    /// A hash of the zerostate file.
    pub zerostate_file_hash: HashBytes,
    /// Workchain version.
    pub version: u32,
    /// Workchain format description.
    pub format: WorkchainFormat,
}

impl WorkchainDescription {
    const TAG: u8 = 0xa6;

    /// Returns `true` if the workchain description is valid.
    pub fn is_valid(&self) -> bool {
        self.min_split <= self.max_split
            && self.max_split <= ShardIdent::MAX_SPLIT_DEPTH
            && self.format.is_valid()
    }
}

impl Store for WorkchainDescription {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        if !self.is_valid() {
            return Err(Error::InvalidData);
        }

        let flags: u16 = ((self.format.is_basic() as u16) << 15)
            | ((self.active as u16) << 14)
            | ((self.accept_msgs as u16) << 13);

        ok!(builder.store_u8(Self::TAG));
        ok!(builder.store_u32(self.enabled_since));
        ok!(builder.store_u8(self.actual_min_split));
        ok!(builder.store_u8(self.min_split));
        ok!(builder.store_u8(self.max_split));
        ok!(builder.store_u16(flags));
        ok!(builder.store_u256(&self.zerostate_root_hash));
        ok!(builder.store_u256(&self.zerostate_file_hash));
        ok!(builder.store_u32(self.version));
        self.format.store_into(builder, context)
    }
}

impl<'a> Load<'a> for WorkchainDescription {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_u8() {
            Ok(Self::TAG) => {}
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        }

        let enabled_since = ok!(slice.load_u32());
        let actual_min_split = ok!(slice.load_u8());
        let min_split = ok!(slice.load_u8());
        let max_split = ok!(slice.load_u8());
        let flags = ok!(slice.load_u16());
        if flags << 3 != 0 {
            return Err(Error::InvalidData);
        }

        let result = Self {
            enabled_since,
            actual_min_split,
            min_split,
            max_split,
            active: flags & 0b0100_0000_0000_0000 != 0,
            accept_msgs: flags & 0b0010_0000_0000_0000 != 0,
            zerostate_root_hash: ok!(slice.load_u256()),
            zerostate_file_hash: ok!(slice.load_u256()),
            version: ok!(slice.load_u32()),
            format: ok!(WorkchainFormat::load_from(slice)),
        };

        let basic = flags & 0b1000_0000_0000_0000 != 0;
        if basic != result.format.is_basic() {
            return Err(Error::InvalidData);
        }

        Ok(result)
    }
}

/// Workchain format description.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum WorkchainFormat {
    /// Basic workchain format.
    Basic(WorkchainFormatBasic),
    /// Extended workchain format.
    Extended(WorkchainFormatExtended),
}

impl WorkchainFormat {
    /// Returns `true` if the workchain format is valid.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Basic(_) => true,
            Self::Extended(format) => format.is_valid(),
        }
    }

    /// Returns `true` if the workchain format is [`Basic`].
    ///
    /// [`Basic`]: WorkchainFormatBasic
    pub fn is_basic(&self) -> bool {
        matches!(self, Self::Basic(_))
    }
}

impl Store for WorkchainFormat {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Basic(value) => {
                ok!(builder.store_small_uint(0x1, 4));
                value.store_into(builder, context)
            }
            Self::Extended(value) => {
                ok!(builder.store_small_uint(0x0, 4));
                value.store_into(builder, context)
            }
        }
    }
}

impl<'a> Load<'a> for WorkchainFormat {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(match ok!(slice.load_small_uint(4)) {
            0x1 => Self::Basic(ok!(WorkchainFormatBasic::load_from(slice))),
            0x0 => Self::Extended(ok!(WorkchainFormatExtended::load_from(slice))),
            _ => return Err(Error::InvalidTag),
        })
    }
}

/// Basic workchain format description.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WorkchainFormatBasic {
    /// VM version.
    pub vm_version: i32,
    /// VM mode.
    pub vm_mode: u64,
}

/// Extended workchain format description.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(validate_with = "Self::is_valid")]
pub struct WorkchainFormatExtended {
    /// The minimal address length in bits.
    pub min_addr_len: Uint12,
    /// The maximal address length in bits.
    pub max_addr_len: Uint12,
    /// Address length step in bits.
    pub addr_len_step: Uint12,
    /// Extended workchain type id.
    pub workchain_type_id: NonZeroU32,
}

impl WorkchainFormatExtended {
    /// Returns `true` if the workchain format is valid.
    pub fn is_valid(&self) -> bool {
        self.min_addr_len >= Uint12::new(64)
            && self.min_addr_len <= self.max_addr_len
            && self.max_addr_len <= Uint12::new(1023)
            && self.addr_len_step <= Uint12::new(1023)
    }

    /// Checks that address length is in a valid range and is aligned to the len step.
    pub fn check_addr_len(&self, addr_len: u16) -> bool {
        let addr_len = Uint12::new(addr_len);

        let is_aligned = || {
            if self.addr_len_step.is_zero() {
                return false;
            }

            let var_part = addr_len - self.min_addr_len;
            let step_rem = var_part.into_inner() % self.addr_len_step.into_inner();
            step_rem == 0
        };

        addr_len >= self.min_addr_len
            && addr_len <= self.max_addr_len
            && (addr_len == self.min_addr_len || addr_len == self.max_addr_len || is_aligned())
    }
}

/// Block creation reward.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#6b")]
pub struct BlockCreationRewards {
    /// Reward for each created masterchain block.
    pub masterchain_block_fee: Tokens,
    /// Base reward for basechain blocks.
    pub basechain_block_fee: Tokens,
}

/// Validators election timings.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ElectionTimings {
    /// Validation round length in seconds.
    pub validators_elected_for: u32,
    /// Duration in seconds until the end of the validation round when the election starts.
    pub elections_start_before: u32,
    /// Duration in seconds until the end of the validation round when the election ends.
    pub elections_end_before: u32,
    /// How long validator stake will be frozen after the validation round end.
    pub stake_held_for: u32,
}

/// Range of number of validators.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidatorCountParams {
    /// The maximum number of validators.
    pub max_validators: u16,
    /// The maximum number of masterchain validators.
    pub max_main_validators: u16,
    /// The minimum number of validators.
    pub min_validators: u16,
}

/// Validator stake range and factor.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidatorStakeParams {
    /// The minimum validator stake.
    pub min_stake: Tokens,
    /// The maximum validator stake.
    pub max_stake: Tokens,
    /// The minimum required total stake for elections to be successful.
    pub min_total_stake: Tokens,
    /// Stake constraint (shifted by 16 bits).
    pub max_stake_factor: u32,
}

/// Storage prices for some interval.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#cc")]
pub struct StoragePrices {
    /// Unix timestamp since which this prices are used.
    pub utime_since: u32,
    /// Bit price in base workchain.
    pub bit_price_ps: u64,
    /// Cell price in base workchain.
    pub cell_price_ps: u64,
    /// Bit price in masterchain.
    pub mc_bit_price_ps: u64,
    /// Cell price in masterchain.
    pub mc_cell_price_ps: u64,
}

impl StoragePrices {
    /// Computes the amount of fees for storing `stats` data for `delta` seconds.
    pub fn compute_storage_fee(
        &self,
        is_masterchain: bool,
        delta: u64,
        stats: CellTreeStats,
    ) -> Tokens {
        let mut res = if is_masterchain {
            (stats.cell_count as u128 * self.mc_cell_price_ps as u128)
                .saturating_add(stats.bit_count as u128 * self.mc_bit_price_ps as u128)
        } else {
            (stats.cell_count as u128 * self.cell_price_ps as u128)
                .saturating_add(stats.bit_count as u128 * self.bit_price_ps as u128)
        };
        res = res.saturating_mul(delta as u128);
        Tokens::new(shift_ceil_price(res))
    }
}

/// Gas limits and prices.
#[derive(Default, Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GasLimitsPrices {
    /// The price of gas unit.
    pub gas_price: u64,
    /// The maximum amount of gas available for a compute phase of an ordinary transaction.
    pub gas_limit: u64,
    /// The maximum amount of gas available for a compute phase of a special transaction.
    pub special_gas_limit: u64,
    /// The maximum amount of gas available before `ACCEPT`.
    pub gas_credit: u64,
    /// The maximum amount of gas units per block.
    pub block_gas_limit: u64,
    /// Amount of debt (in tokens) after which the account will be frozen.
    pub freeze_due_limit: u64,
    /// Amount of debt (in tokens) after which the contract will be deleted.
    pub delete_due_limit: u64,
    /// Size of the first portion of gas with different price.
    pub flat_gas_limit: u64,
    /// The gas price for the first portion determinted by [`flat_gas_limit`].
    ///
    /// [`flat_gas_limit`]: GasLimitsPrices::flat_gas_limit
    pub flat_gas_price: u64,
}

impl GasLimitsPrices {
    /// Maximum possible gas value.
    ///
    /// There is no real reason for it to not be just `u64::MAX`,
    /// but for compatibility reasons we have this.
    pub const MAX_GAS: u64 = i64::MAX as u64;

    /// Converts gas units into tokens.
    pub fn compute_gas_fee(&self, gas_used: u64) -> Tokens {
        let mut res = self.flat_gas_price as u128;
        if let Some(extra_gas) = gas_used.checked_sub(self.flat_gas_limit) {
            res = res.saturating_add(shift_ceil_price(self.gas_price as u128 * extra_gas as u128));
        }
        Tokens::new(res)
    }

    /// Computes the amount of gas bought for the specified amount of tokens.
    pub fn gas_bought_for(&self, balance: &Tokens) -> u64 {
        let balance = balance.into_inner();
        if balance == 0 || balance < self.flat_gas_price as u128 {
            return 0;
        }

        let max_gas_threshold = if self.gas_limit > self.flat_gas_limit {
            shift_ceil_price(
                (self.gas_price as u128) * (self.gas_limit - self.flat_gas_limit) as u128,
            )
            .saturating_add(self.flat_gas_price as u128)
        } else {
            self.flat_gas_price as u128
        };

        if balance >= max_gas_threshold || self.gas_price == 0 {
            return self.gas_limit;
        }

        let mut res = ((balance - self.flat_gas_price as u128) << 16) / (self.gas_price as u128);
        res = res.saturating_add(self.flat_gas_limit as u128);

        res.try_into().unwrap_or(u64::MAX).min(Self::MAX_GAS)
    }

    /// Computes gas params for the specified context.
    pub fn compute_gas_params(&self, args: ComputeGasParams<'_>) -> ComputedGasParams {
        let gas_max = if args.is_special {
            self.special_gas_limit
        } else {
            self.gas_bought_for(args.account_balance)
        };

        let gas_limit = if !args.is_tx_ordinary || args.is_special {
            // May use all gas that can be bought using remaining balance.
            gas_max
        } else {
            // Use only gas bought using remaining message balance.
            // If the message is "accepted" by the smart contract,
            // the gas limit will be set to `gas_max`.
            std::cmp::min(self.gas_bought_for(args.message_balance), gas_max)
        };

        let gas_credit = if args.is_tx_ordinary && args.is_in_msg_external {
            // External messages carry no balance,
            // give them some credit to check whether they are accepted.
            std::cmp::min(self.gas_credit, gas_max)
        } else {
            0
        };

        ComputedGasParams {
            max: gas_max,
            limit: gas_limit,
            credit: gas_credit,
        }
    }
}

/// An input of [`GasLimitsPrices::compute_gas_params`].
#[derive(Debug, Clone, Copy)]
pub struct ComputeGasParams<'a> {
    /// Account balance in native currency.
    pub account_balance: &'a Tokens,
    /// Message balance in native currency.
    pub message_balance: &'a Tokens,
    /// Whether the account is present in a fundamental addresses list.
    pub is_special: bool,
    /// Whether the gas is computed for an ordinary transaction.
    pub is_tx_ordinary: bool,
    /// Whether the received message is external.
    pub is_in_msg_external: bool,
}

/// An output of [`GasLimitsPrices::compute_gas_params`].
#[derive(Debug, Clone, Copy)]
pub struct ComputedGasParams {
    /// Maximum possible value of the `limit`.
    pub max: u64,
    /// Gas limit for the out-of-gas exception.
    pub limit: u64,
    /// Free gas (e.g. for external messages without any balance).
    pub credit: u64,
}

impl GasLimitsPrices {
    const TAG_BASE: u8 = 0xdd;
    const TAG_EXT: u8 = 0xde;
    const TAG_FLAT_PFX: u8 = 0xd1;
}

impl Store for GasLimitsPrices {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        ok!(builder.store_u8(Self::TAG_FLAT_PFX));
        ok!(builder.store_u64(self.flat_gas_limit));
        ok!(builder.store_u64(self.flat_gas_price));
        ok!(builder.store_u8(Self::TAG_EXT));
        ok!(builder.store_u64(self.gas_price));
        ok!(builder.store_u64(self.gas_limit));
        ok!(builder.store_u64(self.special_gas_limit));
        ok!(builder.store_u64(self.gas_credit));
        ok!(builder.store_u64(self.block_gas_limit));
        ok!(builder.store_u64(self.freeze_due_limit));
        builder.store_u64(self.delete_due_limit)
    }
}

impl<'a> Load<'a> for GasLimitsPrices {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let mut result = Self::default();
        loop {
            match slice.load_u8() {
                Ok(Self::TAG_FLAT_PFX) => {
                    result.flat_gas_limit = ok!(slice.load_u64());
                    result.flat_gas_price = ok!(slice.load_u64());
                }
                Ok(Self::TAG_EXT) => {
                    result.gas_price = ok!(slice.load_u64());
                    result.gas_limit = ok!(slice.load_u64());
                    result.special_gas_limit = ok!(slice.load_u64());
                    result.gas_credit = ok!(slice.load_u64());
                    result.block_gas_limit = ok!(slice.load_u64());
                    result.freeze_due_limit = ok!(slice.load_u64());
                    result.delete_due_limit = ok!(slice.load_u64());
                    return Ok(result);
                }
                Ok(Self::TAG_BASE) => {
                    result.gas_price = ok!(slice.load_u64());
                    result.gas_limit = ok!(slice.load_u64());
                    result.gas_credit = ok!(slice.load_u64());
                    result.block_gas_limit = ok!(slice.load_u64());
                    result.freeze_due_limit = ok!(slice.load_u64());
                    result.delete_due_limit = ok!(slice.load_u64());
                    return Ok(result);
                }
                Ok(_) => return Err(Error::InvalidTag),
                Err(e) => return Err(e),
            }
        }
    }
}

/// Block limits parameter.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#c3", validate_with = "Self::is_valid")]
pub struct BlockParamLimits {
    /// Value below which the parameter is considered underloaded.
    pub underload: u32,
    /// Soft limit.
    pub soft_limit: u32,
    /// Hard limit.
    pub hard_limit: u32,
}

impl BlockParamLimits {
    /// Returns `true` if parameter limits are valid.
    pub fn is_valid(&self) -> bool {
        self.underload <= self.soft_limit && self.soft_limit <= self.hard_limit
    }
}

/// Block limits.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#5d")]
pub struct BlockLimits {
    /// Block size limits in bytes.
    pub bytes: BlockParamLimits,
    /// Gas limits.
    pub gas: BlockParamLimits,
    /// Logical time delta limits.
    pub lt_delta: BlockParamLimits,
}

/// Message forwarding prices.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#ea")]
pub struct MsgForwardPrices {
    /// Fixed price in addition to the dynamic part.
    pub lump_price: u64,
    /// The price of bits in the message (bits in the root cell are not included).
    pub bit_price: u64,
    /// The price of cells in the message.
    pub cell_price: u64,
    /// TODO: add docs
    pub ihr_price_factor: u32,
    /// Part of fees that is included to the first block.
    pub first_frac: u16,
    /// Part of fees that goes to transit blocks.
    pub next_frac: u16,
}

impl MsgForwardPrices {
    /// Computes fees for forwarding the specified amount of data.
    pub fn compute_fwd_fee(&self, stats: CellTreeStats) -> Tokens {
        let lump = self.lump_price as u128;
        let extra = shift_ceil_price(
            (stats.cell_count as u128 * self.cell_price as u128)
                .saturating_add(stats.bit_count as u128 * self.bit_price as u128),
        );
        Tokens::new(lump.saturating_add(extra))
    }

    /// Computes the part of the fees that is included to the total fees of the current block.
    pub fn get_first_part(&self, total: Tokens) -> Tokens {
        Tokens::new(total.into_inner().saturating_mul(self.first_frac as _) >> 16)
    }

    /// Computes the part of the fees that is included to the total fees of the transit block.
    pub fn get_next_part(&self, total: Tokens) -> Tokens {
        Tokens::new(total.into_inner().saturating_mul(self.next_frac as _) >> 16)
    }
}

/// Catchain configuration params.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CatchainConfig {
    /// Exclude masterchain validators from a validators list for a base workchain.
    pub isolate_mc_validators: bool,
    /// Change the order of validators in the masterchain validators list.
    pub shuffle_mc_validators: bool,
    /// Masterchain catchain session lifetime in seconds.
    pub mc_catchain_lifetime: u32,
    /// Catchain session lifetime for shards in seconds.
    pub shard_catchain_lifetime: u32,
    /// Period in seconds for which the subset of validators is selected for each shard.
    pub shard_validators_lifetime: u32,
    /// The number of validators per shard.
    pub shard_validators_num: u32,
}

impl CatchainConfig {
    const TAG_V1: u8 = 0xc1;
    const TAG_V2: u8 = 0xc2;
}

impl Store for CatchainConfig {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let flags = ((self.isolate_mc_validators as u8) << 1) | (self.shuffle_mc_validators as u8);
        ok!(builder.store_u8(Self::TAG_V2));
        ok!(builder.store_u8(flags));
        ok!(builder.store_u32(self.mc_catchain_lifetime));
        ok!(builder.store_u32(self.shard_catchain_lifetime));
        ok!(builder.store_u32(self.shard_validators_lifetime));
        builder.store_u32(self.shard_validators_num)
    }
}

impl<'a> Load<'a> for CatchainConfig {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let flags = match slice.load_u8() {
            Ok(Self::TAG_V1) => 0,
            Ok(Self::TAG_V2) => ok!(slice.load_u8()),
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        };
        if flags >> 2 != 0 {
            return Err(Error::InvalidData);
        }
        Ok(Self {
            isolate_mc_validators: flags & 0b10 != 0,
            shuffle_mc_validators: flags & 0b01 != 0,
            mc_catchain_lifetime: ok!(slice.load_u32()),
            shard_catchain_lifetime: ok!(slice.load_u32()),
            shard_validators_lifetime: ok!(slice.load_u32()),
            shard_validators_num: ok!(slice.load_u32()),
        })
    }
}

/// Consensus configuration params.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConsensusConfig {
    /// Allow new catchain ids.
    pub new_catchain_ids: bool,
    /// Number of block candidates per round.
    pub round_candidates: NonZeroU32,
    /// Delay in seconds before proposing a new candidate.
    pub next_candidate_delay_ms: u32,
    /// Catchain processing timeout in seconds.
    pub consensus_timeout_ms: u32,
    /// Maximum number of attempts per round.
    pub fast_attempts: u32,
    /// Duration of a round attempt in seconds.
    pub attempt_duration: u32,
    /// The maximum number of dependencies to merge.
    pub catchain_max_deps: u32,
    /// The maximum block size in bytes.
    pub max_block_bytes: u32,
    /// THe maximum size of a collated data in bytes.
    pub max_collated_bytes: u32,
}

impl ConsensusConfig {
    const TAG_V1: u8 = 0xd6;
    const TAG_V2: u8 = 0xd7;
}

impl Store for ConsensusConfig {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let flags = self.new_catchain_ids as u8;

        ok!(builder.store_u8(Self::TAG_V2));
        ok!(builder.store_u8(flags));
        ok!(builder.store_u8(self.round_candidates.get() as u8));
        ok!(builder.store_u32(self.next_candidate_delay_ms));
        ok!(builder.store_u32(self.consensus_timeout_ms));
        ok!(builder.store_u32(self.fast_attempts));
        ok!(builder.store_u32(self.attempt_duration));
        ok!(builder.store_u32(self.catchain_max_deps));
        ok!(builder.store_u32(self.max_block_bytes));
        builder.store_u32(self.max_collated_bytes)
    }
}

impl<'a> Load<'a> for ConsensusConfig {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        use std::num::NonZeroU8;

        let (flags, round_candidates) = match slice.load_u8() {
            Ok(Self::TAG_V1) => (0, ok!(NonZeroU32::load_from(slice))),
            Ok(Self::TAG_V2) => {
                let flags = ok!(slice.load_u8());
                if flags >> 1 != 0 {
                    return Err(Error::InvalidData);
                }
                (flags, ok!(NonZeroU8::load_from(slice)).into())
            }
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        };
        Ok(Self {
            new_catchain_ids: flags & 0b1 != 0,
            round_candidates,
            next_candidate_delay_ms: ok!(slice.load_u32()),
            consensus_timeout_ms: ok!(slice.load_u32()),
            fast_attempts: ok!(slice.load_u32()),
            attempt_duration: ok!(slice.load_u32()),
            catchain_max_deps: ok!(slice.load_u32()),
            max_block_bytes: ok!(slice.load_u32()),
            max_collated_bytes: ok!(slice.load_u32()),
        })
    }
}

/// Validator set.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ValidatorSet {
    /// Unix timestamp from which this set will be active.
    pub utime_since: u32,
    /// Unix timestamp until which this set will be active.
    pub utime_until: u32,
    /// The number of masterchain validators.
    pub main: NonZeroU16,
    /// Total validators weight.
    pub total_weight: u64,
    /// Validators.
    pub list: Vec<ValidatorDescription>,
}

impl ValidatorSet {
    const TAG_V1: u8 = 0x11;
    const TAG_V2: u8 = 0x12;

    /// Computes a validator subset using a zero seed.
    pub fn compute_subset(
        &self,
        shard_ident: ShardIdent,
        cc_config: &CatchainConfig,
        cc_seqno: u32,
    ) -> Option<(Vec<ValidatorDescription>, u32)> {
        if shard_ident.is_masterchain() {
            return self.compute_mc_subset(cc_seqno, cc_config.shuffle_mc_validators);
        }

        let total = self.list.len();
        let main = self.main.get() as usize;

        let mut prng = ValidatorSetPRNG::new(shard_ident, cc_seqno);

        let vset = if cc_config.isolate_mc_validators {
            if total <= main {
                return None;
            }

            let mut list = self.list[main..].to_vec();

            let mut total_weight = 0u64;
            for descr in &mut list {
                descr.prev_total_weight = total_weight;
                total_weight += descr.weight;
            }

            std::borrow::Cow::Owned(Self {
                utime_since: self.utime_since,
                utime_until: self.utime_until,
                main: self.main,
                total_weight,
                list,
            })
        } else {
            std::borrow::Cow::Borrowed(self)
        };

        let count = std::cmp::min(vset.list.len(), cc_config.shard_validators_num as usize);

        let mut nodes = Vec::with_capacity(count);
        let mut holes = Vec::<(u64, u64)>::with_capacity(count);
        let mut total_wt = vset.total_weight;

        for _ in 0..count {
            debug_assert!(total_wt > 0);

            // Generate a pseudo-random number 0..total_wt-1
            let mut p = prng.next_ranged(total_wt);

            for (prev_total_weight, weight) in &holes {
                if p < *prev_total_weight {
                    break;
                }
                p += weight;
            }

            let entry = vset.at_weight(p);

            nodes.push(ValidatorDescription {
                public_key: entry.public_key,
                weight: 1,
                adnl_addr: entry.adnl_addr,
                prev_total_weight: 0,
            });
            debug_assert!(total_wt >= entry.weight);
            total_wt -= entry.weight;

            let new_hole = (entry.prev_total_weight, entry.weight);
            let i = holes.partition_point(|item| item <= &new_hole);
            debug_assert!(i == 0 || holes[i - 1] < new_hole);

            holes.insert(i, new_hole);
        }

        let hash_short = Self::compute_subset_hash_short(&nodes, cc_seqno);

        Some((nodes, hash_short))
    }

    /// Computes a masterchain validator subset using a zero seed.
    ///
    /// NOTE: In most cases you should use the more generic [`ValidatorSet::compute_subset`].
    pub fn compute_mc_subset(
        &self,
        cc_seqno: u32,
        shuffle: bool,
    ) -> Option<(Vec<ValidatorDescription>, u32)> {
        let total = self.list.len();
        let main = self.main.get() as usize;

        let count = std::cmp::min(total, main);
        let subset = if !shuffle {
            self.list[0..count].to_vec()
        } else {
            let mut prng = ValidatorSetPRNG::new(ShardIdent::MASTERCHAIN, cc_seqno);

            let mut indices = vec![0; count];
            for i in 0..count {
                let j = prng.next_ranged(i as u64 + 1) as usize; // number 0 .. i
                debug_assert!(j <= i);
                indices[i] = indices[j];
                indices[j] = i;
            }

            let mut subset = Vec::with_capacity(count);
            for index in indices.into_iter().take(count) {
                subset.push(self.list[index].clone());
            }
            subset
        };

        let hash_short = Self::compute_subset_hash_short(&subset, cc_seqno);
        Some((subset, hash_short))
    }

    /// Computes a masterchain validator subset using a zero seed.
    /// Preserves original validator indexes inside vset.
    ///
    /// NOTE: In most cases you should use the more generic [`ValidatorSet::compute_subset`].
    pub fn compute_mc_subset_indexed(
        &self,
        cc_seqno: u32,
        shuffle: bool,
    ) -> Option<(Vec<IndexedValidatorDescription>, u32)> {
        let total = self.list.len();
        let main = self.main.get() as usize;

        let count = std::cmp::min(total, main);
        let subset = if !shuffle {
            self.list[0..count]
                .iter()
                .enumerate()
                .map(|(i, desc)| IndexedValidatorDescription {
                    desc: desc.clone(),
                    validator_idx: i as u16,
                })
                .collect::<Vec<_>>()
        } else {
            let mut prng = ValidatorSetPRNG::new(ShardIdent::MASTERCHAIN, cc_seqno);

            let mut indices = vec![0; count];
            for i in 0..count {
                let j = prng.next_ranged(i as u64 + 1) as usize; // number 0 .. i
                debug_assert!(j <= i);
                indices[i] = indices[j];
                indices[j] = i;
            }

            let mut subset = Vec::with_capacity(count);
            for index in indices.into_iter().take(count) {
                subset.push(IndexedValidatorDescription {
                    desc: self.list[index].clone(),
                    validator_idx: index as u16,
                });
            }
            subset
        };

        let hash_short =
            Self::compute_subset_hash_short(subset.iter().map(AsRef::as_ref), cc_seqno);
        Some((subset, hash_short))
    }

    /// Compoutes a validator subset short hash.
    pub fn compute_subset_hash_short<'a, I>(subset: I, cc_seqno: u32) -> u32
    where
        I: IntoIterator<Item = &'a ValidatorDescription, IntoIter: ExactSizeIterator>,
    {
        const HASH_SHORT_MAGIC: u32 = 0x901660ED;

        let subset = subset.into_iter();

        let mut hash = crc32c::crc32c(&HASH_SHORT_MAGIC.to_le_bytes());
        hash = crc32c::crc32c_append(hash, &cc_seqno.to_le_bytes());
        hash = crc32c::crc32c_append(hash, &(subset.len() as u32).to_le_bytes());

        for node in subset {
            hash = crc32c::crc32c_append(hash, node.public_key.as_slice());
            hash = crc32c::crc32c_append(hash, &node.weight.to_le_bytes());
            hash = crc32c::crc32c_append(
                hash,
                node.adnl_addr
                    .as_ref()
                    .unwrap_or(HashBytes::wrap(&[0u8; 32]))
                    .as_ref(),
            );
        }

        hash
    }

    fn at_weight(&self, weight_pos: u64) -> &ValidatorDescription {
        debug_assert!(weight_pos < self.total_weight);
        debug_assert!(!self.list.is_empty());
        let i = self
            .list
            .partition_point(|item| item.prev_total_weight <= weight_pos);
        debug_assert!(i != 0);
        &self.list[i - 1]
    }
}

impl Store for ValidatorSet {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let Ok(total) = u16::try_from(self.list.len()) else {
            return Err(Error::IntOverflow);
        };

        let validators = build_dict_from_sorted_iter(
            self.list
                .iter()
                .enumerate()
                .map(|(i, item)| (i as u16, item)),
            context,
        )?;

        ok!(builder.store_u8(Self::TAG_V2));
        ok!(builder.store_u32(self.utime_since));
        ok!(builder.store_u32(self.utime_until));
        ok!(builder.store_u16(total));
        ok!(builder.store_u16(self.main.get()));
        ok!(builder.store_u64(self.total_weight));
        validators.store_into(builder, context)
    }
}

impl<'a> Load<'a> for ValidatorSet {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let with_total_weight = match slice.load_u8() {
            Ok(Self::TAG_V1) => false,
            Ok(Self::TAG_V2) => true,
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        };

        let utime_since = ok!(slice.load_u32());
        let utime_until = ok!(slice.load_u32());
        let total = ok!(slice.load_u16()) as usize;
        let main = ok!(NonZeroU16::load_from(slice));
        if main.get() as usize > total {
            return Err(Error::InvalidData);
        }

        let context = Cell::empty_context();

        let (mut total_weight, validators) = if with_total_weight {
            let total_weight = ok!(slice.load_u64());
            let dict = ok!(Dict::<u16, ValidatorDescription>::load_from(slice));
            (total_weight, dict)
        } else {
            let dict = ok!(Dict::<u16, ValidatorDescription>::load_from_root_ext(
                slice, context
            ));
            (0, dict)
        };

        let mut computed_total_weight = 0u64;
        let mut list = Vec::with_capacity(std::cmp::min(total, 512));
        for (i, entry) in validators.iter().enumerate().take(total) {
            let mut descr = match entry {
                Ok((idx, descr)) if idx as usize == i => descr,
                Ok(_) => return Err(Error::InvalidData),
                Err(e) => return Err(e),
            };

            descr.prev_total_weight = computed_total_weight;
            computed_total_weight = match computed_total_weight.checked_add(descr.weight) {
                Some(weight) => weight,
                None => return Err(Error::InvalidData),
            };
            list.push(descr);
        }

        if list.is_empty() {
            return Err(Error::InvalidData);
        }

        if with_total_weight {
            if total_weight != computed_total_weight {
                return Err(Error::InvalidData);
            }
        } else {
            total_weight = computed_total_weight;
        }

        Ok(Self {
            utime_since,
            utime_until,
            main,
            total_weight,
            list,
        })
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ValidatorSet {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        #[derive(serde::Deserialize)]
        struct ValidatorSetHelper {
            utime_since: u32,
            utime_until: u32,
            main: NonZeroU16,
            #[serde(default)]
            total_weight: u64,
            list: Vec<ValidatorDescription>,
        }

        let parsed = ValidatorSetHelper::deserialize(deserializer)?;
        if parsed.list.is_empty() {
            return Err(Error::custom("empty validators list"));
        }

        let mut result = Self {
            utime_since: parsed.utime_since,
            utime_until: parsed.utime_until,
            main: parsed.main,
            total_weight: 0,
            list: parsed.list,
        };

        for descr in &mut result.list {
            descr.prev_total_weight = result.total_weight;
            let Some(new_total_weight) = result.total_weight.checked_add(descr.weight) else {
                return Err(Error::custom("total weight overflow"));
            };
            result.total_weight = new_total_weight;
        }

        if parsed.total_weight > 0 && parsed.total_weight != result.total_weight {
            return Err(Error::custom("total weight mismatch"));
        }

        Ok(result)
    }
}

/// Validator public key, weight, and optional ADNL address from a TON validator set.
///
/// Binary serialization uses `validator#53` or `validator_addr#73`, depending on
/// whether an ADNL address is present.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidatorDescription {
    /// Validator public key.
    pub public_key: HashBytes, // TODO: replace with tycho_crypto::ed25519::PublicKey ?
    /// Validator weight in some units.
    pub weight: u64,
    /// Optional validator ADNL address.
    #[cfg_attr(feature = "serde", serde(default))]
    pub adnl_addr: Option<HashBytes>,

    /// Total weight of the previous validators in the list.
    /// The field is not serialized.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub prev_total_weight: u64,
}

impl ValidatorDescription {
    const TAG_BASIC: u8 = 0x53;
    const TAG_WITH_ADNL: u8 = 0x73;

    const PUBKEY_TAG: u32 = 0x8e81278a;

    /// Verifies message signature and current public key.
    pub fn verify_signature(&self, data: &[u8], signature: &Signature) -> bool {
        if let Some(public_key) = ed25519::PublicKey::from_bytes(self.public_key.0) {
            public_key.verify_raw(data, signature.as_ref())
        } else {
            false
        }
    }
}

impl Store for ValidatorDescription {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let tag = if self.adnl_addr.is_some() {
            Self::TAG_WITH_ADNL
        } else {
            Self::TAG_BASIC
        };

        ok!(builder.store_u8(tag));
        ok!(builder.store_u32(Self::PUBKEY_TAG));
        ok!(builder.store_u256(&self.public_key));
        ok!(builder.store_u64(self.weight));

        if let Some(adnl) = &self.adnl_addr {
            ok!(builder.store_u256(adnl));
        }

        Ok(())
    }
}

impl<'a> Load<'a> for ValidatorDescription {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let with_adnl = match slice.load_u8() {
            Ok(Self::TAG_BASIC) => false,
            Ok(Self::TAG_WITH_ADNL) => true,
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        };

        Ok(Self {
            public_key: {
                match slice.load_u32() {
                    Ok(Self::PUBKEY_TAG) => ok!(slice.load_u256()),
                    Ok(_) => return Err(Error::InvalidTag),
                    Err(e) => return Err(e),
                }
            },
            weight: ok!(slice.load_u64()),
            adnl_addr: if with_adnl {
                Some(ok!(slice.load_u256()))
            } else {
                None
            },
            prev_total_weight: 0,
        })
    }
}

/// Validator description with its original index in vset.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IndexedValidatorDescription {
    /// Validator description.
    pub desc: ValidatorDescription,
    /// Index in the original validator set.
    pub validator_idx: u16,
}

impl AsRef<ValidatorDescription> for IndexedValidatorDescription {
    #[inline]
    fn as_ref(&self) -> &ValidatorDescription {
        &self.desc
    }
}

impl AsMut<ValidatorDescription> for IndexedValidatorDescription {
    #[inline]
    fn as_mut(&mut self) -> &mut ValidatorDescription {
        &mut self.desc
    }
}

impl std::ops::Deref for IndexedValidatorDescription {
    type Target = ValidatorDescription;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.desc
    }
}

impl std::ops::DerefMut for IndexedValidatorDescription {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.desc
    }
}

/// Random generator used for validator subset calculation.
pub struct ValidatorSetPRNG {
    context: [u8; 48],
    bag: [u64; 8],
}

impl ValidatorSetPRNG {
    /// Creates a new generator with zero seed.
    pub fn new(shard_ident: ShardIdent, cc_seqno: u32) -> Self {
        let seed = [0; 32];
        Self::with_seed(shard_ident, cc_seqno, &seed)
    }

    /// Creates a new generator with the specified seed.
    pub fn with_seed(shard_ident: ShardIdent, cc_seqno: u32, seed: &[u8; 32]) -> Self {
        let mut context = [0u8; 48];
        context[..32].copy_from_slice(seed);
        context[32..40].copy_from_slice(&shard_ident.prefix().to_be_bytes());
        context[40..44].copy_from_slice(&shard_ident.workchain().to_be_bytes());
        context[44..48].copy_from_slice(&cc_seqno.to_be_bytes());

        let mut res = ValidatorSetPRNG {
            context,
            bag: [0; 8],
        };
        res.bag[0] = 8;
        res
    }

    /// Generates next `u64`.
    pub fn next_u64(&mut self) -> u64 {
        if self.cursor() < 7 {
            let next = self.bag[1 + self.cursor() as usize];
            self.bag[0] += 1;
            next
        } else {
            self.reset()
        }
    }

    /// Generates next `u64` multiplied by the specified range.
    pub fn next_ranged(&mut self, range: u64) -> u64 {
        let val = self.next_u64();
        ((range as u128 * val as u128) >> 64) as u64
    }

    fn reset(&mut self) -> u64 {
        use sha2::digest::Digest;

        let hash: [u8; 64] = sha2::Sha512::digest(self.context).into();

        for ctx in self.context[..32].iter_mut().rev() {
            *ctx = ctx.wrapping_add(1);
            if *ctx != 0 {
                break;
            }
        }

        // SAFETY: `std::mem::size_of::<[u64; 8]>() == 64` and src alignment is 1
        unsafe {
            std::ptr::copy_nonoverlapping(hash.as_ptr(), self.bag.as_mut_ptr() as *mut u8, 64);
        }

        // Swap bytes for little endian
        #[cfg(target_endian = "little")]
        self.bag
            .iter_mut()
            .for_each(|item| *item = item.swap_bytes());

        // Reset and use bag[0] as counter
        std::mem::take(&mut self.bag[0])
    }

    #[inline]
    const fn cursor(&self) -> u64 {
        self.bag[0]
    }
}

/// size_limits_config_v2#02
///     max_msg_bits:uint32
///     max_msg_cells:uint32
///     max_library_cells:uint32
///     max_vm_data_depth:uint16
///     max_ext_msg_size:uint32
///     max_ext_msg_depth:uint16
///     max_acc_state_cells:uint32
///     max_acc_state_bits:uint32
///     max_acc_public_libraries:uint32
///     defer_out_queue_size_limit:uint32 = SizeLimitsConfig;
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#02")]
pub struct SizeLimitsConfig {
    /// Max number of bits in message.
    pub max_msg_bits: u32,
    /// Max number of cells in message.
    pub max_msg_cells: u32,
    /// Max number of cells in library.
    pub max_library_cells: u32,
    /// Max cell tree depth for VM data.
    pub max_vm_data_depth: u16,
    /// Max number of bytes of a BOC-encoded external message.
    pub max_ext_msg_size: u32,
    /// Max cell tree depth of an external message.
    pub max_ext_msg_depth: u16,
    /// Max number of cells per account.
    pub max_acc_state_cells: u32,
    /// Max number of bits per account.
    pub max_acc_state_bits: u32,
    /// Max number of public libraries per account.
    pub max_acc_public_libraries: u32,
    /// Size limit of a deferred out messages queue.
    pub defer_out_queue_size_limit: u32,
}

const fn shift_ceil_price(value: u128) -> u128 {
    let r = value & 0xffff != 0;
    (value >> 16) + r as u128
}

/// Authority marks configuration.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#01")]
pub struct AuthorityMarksConfig {
    /// Addresses in masterchain that can manage authority marks.
    pub authority_addresses: Dict<HashBytes, ()>,
    /// Black mark extra currency id.
    pub black_mark_id: u32,
    /// White mark extra currency id.
    pub white_mark_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validator_set_prng() {
        fn make_indices(cc_seqno: u32) -> Vec<usize> {
            let mut prng = ValidatorSetPRNG::new(ShardIdent::BASECHAIN, cc_seqno);

            let count = 10;
            let mut indices = vec![0; count];
            for i in 0..count {
                let j = prng.next_ranged(i as u64 + 1) as usize;
                debug_assert!(j <= i);
                indices[i] = indices[j];
                indices[j] = i;
            }

            indices
        }

        let vs10_first = make_indices(10);
        let vs10_second = make_indices(10);
        assert_eq!(vs10_first, vs10_second);

        let vs11_first = make_indices(11);
        let vs11_second = make_indices(11);
        assert_eq!(vs11_first, vs11_second);
        assert_ne!(vs10_first, vs11_second);
    }
}
