use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{GenesisIssue, validation::check};

/// Split bounds for workchain 0. Its zerostate starts as one unsplit shard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Workchain {
    /// Minimum shard prefix length after the chain starts splitting
    pub min_split: u8,
    /// Maximum shard prefix length
    pub max_split: u8,
}

impl Default for Workchain {
    fn default() -> Self {
        Self {
            min_split: 0,
            max_split: 4,
        }
    }
}

impl Workchain {
    pub(crate) fn validation_issues(&self, issues: &mut Vec<GenesisIssue>) {
        check(
            issues,
            self.min_split <= self.max_split && self.max_split <= 60,
            "workchain",
            &["min_split", "max_split"],
            "Shard split depths must follow: 0 ≤ minimum ≤ maximum ≤ 60",
        );
    }
}

/// Storage charges per second in nanograms scaled by 2^16.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct StoragePrices {
    /// Workchain price for one stored bit
    pub bit_price: u64,
    /// Workchain price for one stored cell
    pub cell_price: u64,
    /// Masterchain price for one stored bit
    pub masterchain_bit_price: u64,
    /// Masterchain price for one stored cell
    pub masterchain_cell_price: u64,
}

impl Default for StoragePrices {
    fn default() -> Self {
        Self {
            bit_price: 1,
            cell_price: 500,
            masterchain_bit_price: 1_000,
            masterchain_cell_price: 500_000,
        }
    }
}

/// Execution limits and flat fees. Variable gas prices live in the root config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct GasLimits {
    /// Maximum gas consumed by an ordinary transaction
    pub gas_limit: u64,
    /// Maximum gas consumed by a special contract transaction
    pub special_gas_limit: u64,
    /// Gas available before an external message is accepted
    pub gas_credit: u64,
    /// Maximum gas consumed by a block
    pub block_gas_limit: u64,
    /// Storage debt that freezes an account, in nanograms
    pub freeze_due_limit: u64,
    /// Storage debt that deletes an account, in nanograms
    pub delete_due_limit: u64,
    /// Gas covered by the flat fee
    pub flat_gas_limit: u64,
    /// Flat execution fee in nanograms
    pub flat_gas_price: u64,
}

impl Default for GasLimits {
    fn default() -> Self {
        Self {
            gas_limit: 1_000_000,
            special_gas_limit: 1_000_000,
            gas_credit: 10_000,
            block_gas_limit: 10_000_000,
            freeze_due_limit: 100_000_000,
            delete_due_limit: 1_000_000_000,
            flat_gas_limit: 100,
            flat_gas_price: 40_000,
        }
    }
}

impl GasLimits {
    pub(crate) fn masterchain() -> Self {
        Self {
            special_gas_limit: 70_000_000,
            block_gas_limit: 2_500_000,
            flat_gas_price: 1_000_000,
            ..Self::default()
        }
    }

    pub(crate) fn validation_issues(&self, issues: &mut Vec<GenesisIssue>, path: &str, name: &str) {
        for (key, value) in [
            ("gas_limit", self.gas_limit),
            ("special_gas_limit", self.special_gas_limit),
            ("block_gas_limit", self.block_gas_limit),
        ] {
            check(
                issues,
                value > 0,
                path,
                &[key],
                format!("{name}: gas limits must be positive"),
            );
        }
        for (key, value) in [
            ("gas_credit", self.gas_credit),
            ("flat_gas_limit", self.flat_gas_limit),
        ] {
            check(
                issues,
                value <= self.gas_limit,
                path,
                &[key, "gas_limit"],
                format!(
                    "{name}: gas credit and flat gas limit must not exceed the transaction gas limit"
                ),
            );
        }
        check(
            issues,
            self.freeze_due_limit <= self.delete_due_limit,
            path,
            &["freeze_due_limit", "delete_due_limit"],
            format!("{name}: the account freeze threshold must not exceed the deletion threshold"),
        );
    }
}

/// Message forwarding charges. Cell prices live in the root config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct ForwardingPrices {
    /// Fixed message charge in nanograms
    pub lump_price: u64,
    /// Bit price in nanograms scaled by 2^16
    pub bit_price: u64,
    /// Instant Hypercube Routing multiplier scaled by 2^16
    pub ihr_factor: u32,
    /// Fraction collected by the first validator, scaled by 2^16
    pub first_frac: u16,
    /// Fraction collected by each subsequent validator, scaled by 2^16
    pub next_frac: u16,
}

impl Default for ForwardingPrices {
    fn default() -> Self {
        Self {
            lump_price: 400_000,
            bit_price: 26_214_400,
            ihr_factor: 98_304,
            first_frac: 21_845,
            next_frac: 21_845,
        }
    }
}

impl ForwardingPrices {
    pub(crate) fn masterchain() -> Self {
        Self {
            lump_price: 10_000_000,
            bit_price: 655_360_000,
            ..Self::default()
        }
    }
}

/// Ordered thresholds used by TON to decide whether to split or merge shards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// Below this value, a block is underloaded
    pub underload: u32,
    /// Soft resource ceiling
    pub soft: u32,
    /// Hard resource ceiling
    pub hard: u32,
}

impl Limits {
    fn validation_issues(&self, issues: &mut Vec<GenesisIssue>, path: &str, name: &str) {
        check(
            issues,
            self.underload < self.soft && self.soft < self.hard,
            path,
            &["underload", "soft", "hard"],
            format!(
                "{name}: the underload threshold must be below the soft limit, and the soft limit below the hard limit"
            ),
        );
    }
}

/// Independent block resource thresholds for one chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct BlockLimits {
    /// Serialized block size in bytes
    pub bytes: Limits,
    /// Gas consumed by transactions
    pub gas: Limits,
    /// Logical time delta covered by a block
    pub logical_time: Limits,
}

impl Default for BlockLimits {
    fn default() -> Self {
        Self {
            bytes: Limits {
                underload: 262_144,
                soft: 1_048_576,
                hard: 2_097_152,
            },
            gas: Limits {
                underload: 2_000_000,
                soft: 10_000_000,
                hard: 20_000_000,
            },
            logical_time: Limits {
                underload: 1_000,
                soft: 5_000,
                hard: 10_000,
            },
        }
    }
}

impl BlockLimits {
    pub(crate) fn validation_issues(&self, issues: &mut Vec<GenesisIssue>, path: &str, name: &str) {
        for (key, label, limits) in [
            ("bytes", "bytes", &self.bytes),
            ("gas", "gas", &self.gas),
            ("logical_time", "logical time", &self.logical_time),
        ] {
            limits.validation_issues(
                issues,
                &format!("{path}.{key}"),
                &format!("{name} ({label})"),
            );
        }
    }
}

/// Validator rotation intervals and shard committee size.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Catchain {
    /// Masterchain catchain lifetime in seconds
    pub masterchain_lifetime_seconds: u32,
    /// Shard catchain lifetime in seconds
    pub shard_lifetime_seconds: u32,
    /// Shard validator committee lifetime in seconds
    pub shard_validators_lifetime_seconds: u32,
    /// Maximum validators assigned to a shard
    pub shard_validators: u32,
    /// Shuffle masterchain validator assignments between sessions
    pub shuffle_masterchain_validators: bool,
}

impl Default for Catchain {
    fn default() -> Self {
        Self {
            masterchain_lifetime_seconds: 250,
            shard_lifetime_seconds: 250,
            shard_validators_lifetime_seconds: 5_500,
            shard_validators: 7,
            shuffle_masterchain_validators: true,
        }
    }
}

impl Catchain {
    pub(crate) fn validation_issues(&self, issues: &mut Vec<GenesisIssue>) {
        for (key, value) in [
            (
                "masterchain_lifetime_seconds",
                self.masterchain_lifetime_seconds,
            ),
            ("shard_lifetime_seconds", self.shard_lifetime_seconds),
            (
                "shard_validators_lifetime_seconds",
                self.shard_validators_lifetime_seconds,
            ),
            ("shard_validators", self.shard_validators),
        ] {
            check(
                issues,
                value > 0,
                "catchain",
                &[key],
                "Validator rotation intervals and the number of validators per shard must be greater than zero",
            );
        }
    }
}

/// Voting thresholds and storage prices for configuration proposals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Voting {
    /// Minimum voting rounds before a proposal is accepted
    pub min_rounds: u8,
    /// Maximum voting rounds before a proposal expires
    pub max_rounds: u8,
    /// Successful rounds required to accept the proposal
    pub min_wins: u8,
    /// Failed rounds allowed before the proposal expires
    pub max_losses: u8,
    /// Minimum proposal storage period in seconds
    pub min_store_seconds: u32,
    /// Maximum proposal storage period in seconds
    pub max_store_seconds: u32,
    /// Price per stored bit per second in nanograms
    pub bit_price: u32,
    /// Price per stored cell per second in nanograms
    pub cell_price: u32,
}

impl Default for Voting {
    fn default() -> Self {
        Self {
            min_rounds: 2,
            max_rounds: 3,
            min_wins: 2,
            max_losses: 2,
            min_store_seconds: 1_000_000,
            max_store_seconds: 10_000_000,
            bit_price: 1,
            cell_price: 500,
        }
    }
}

impl Voting {
    pub(crate) const fn critical() -> Self {
        Self {
            min_rounds: 4,
            max_rounds: 7,
            min_wins: 4,
            max_losses: 2,
            min_store_seconds: 5_000_000,
            max_store_seconds: 20_000_000,
            bit_price: 2,
            cell_price: 1_000,
        }
    }

    pub(crate) fn validation_issues(&self, issues: &mut Vec<GenesisIssue>, path: &str, name: &str) {
        check(
            issues,
            self.min_rounds > 0 && self.min_rounds <= self.max_rounds,
            path,
            &["min_rounds", "max_rounds"],
            format!(
                "{name}: minimum rounds must be greater than zero and must not exceed maximum rounds"
            ),
        );
        check(
            issues,
            self.min_wins > 0 && self.min_wins <= self.max_rounds,
            path,
            &["min_wins", "max_rounds"],
            format!(
                "{name}: required wins must be greater than zero and must not exceed maximum rounds"
            ),
        );
        check(
            issues,
            self.max_losses > 0,
            path,
            &["max_losses"],
            format!("{name}: allowed losses must be greater than zero"),
        );
        check(
            issues,
            self.min_store_seconds <= self.max_store_seconds,
            path,
            &["min_store_seconds", "max_store_seconds"],
            format!("{name}: minimum storage duration must not exceed maximum storage duration"),
        );
    }
}
