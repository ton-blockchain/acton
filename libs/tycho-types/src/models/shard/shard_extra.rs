use crate::cell::*;
use crate::dict::{AugDict, AugDictExtra, Dict};
use crate::error::Error;
use crate::models::block::{BlockRef, ShardHashes};
use crate::models::config::BlockchainConfig;
use crate::models::currency::CurrencyCollection;

/// Additional content for masterchain state.
///
/// # TLB scheme
///
/// ```text
/// masterchain_state_extra#cc26
///     shard_hashes:ShardHashes
///     config:ConfigParams
///     ^[
///         flags:(## 16) { flags <= 3 }
///         validator_info:ValidatorInfo
///         prev_blocks:OldMcBlocksInfo
///         after_key_block:Bool
///         last_key_block:(Maybe ExtBlkRef)
///         block_create_stats:(flags . 0)?BlockCreateStats
///     ]
///     global_balance:CurrencyCollection
///     = McStateExtra;
/// ```
#[derive(Debug, Clone)]
pub struct McStateExtra {
    /// A tree of the most recent descriptions for all currently existing shards
    /// for all workchains except the masterchain.
    pub shards: ShardHashes,
    /// The most recent blockchain config (if the block is a key block).
    pub config: BlockchainConfig,
    /// Brief validator info.
    pub validator_info: ValidatorInfo,
    /// A dictionary with previous masterchain blocks.
    pub prev_blocks: AugDict<u32, KeyMaxLt, KeyBlockRef>,
    /// Whether this state was produced after the key block.
    pub after_key_block: bool,
    /// Optional reference to the latest known key block.
    pub last_key_block: Option<BlockRef>,
    /// Block creation stats for validators from the current set.
    pub block_create_stats: Option<Dict<HashBytes, CreatorStats>>,
    /// Total balance of all accounts.
    pub global_balance: CurrencyCollection,
}

impl McStateExtra {
    const TAG: u16 = 0xcc26;
    const BLOCK_STATS_TAG: u8 = 0x17;
}

impl Store for McStateExtra {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let flags = self.block_create_stats.is_some() as u16;

        let cell = {
            let mut builder = CellBuilder::new();
            ok!(builder.store_u16(flags));
            ok!(self.validator_info.store_into(&mut builder, context));
            ok!(self.prev_blocks.store_into(&mut builder, context));
            ok!(builder.store_bit(self.after_key_block));
            ok!(self.last_key_block.store_into(&mut builder, context));

            if let Some(stats) = &self.block_create_stats {
                ok!(builder.store_u8(Self::BLOCK_STATS_TAG));
                ok!(stats.store_into(&mut builder, context));
            }

            ok!(builder.build_ext(context))
        };

        ok!(builder.store_u16(Self::TAG));
        ok!(self.shards.store_into(builder, context));
        ok!(self.config.store_into(builder, context));
        ok!(builder.store_reference(cell));
        self.global_balance.store_into(builder, context)
    }
}

impl<'a> Load<'a> for McStateExtra {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_u16() {
            Ok(Self::TAG) => {}
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        }

        let shards = ok!(ShardHashes::load_from(slice));
        let config = ok!(BlockchainConfig::load_from(slice));

        let child_slice = &mut ok!(slice.load_reference_as_slice());
        let flags = ok!(child_slice.load_u16());

        const RESERVED_BITS: usize = 2;

        if flags >> RESERVED_BITS != 0 {
            return Err(Error::InvalidData);
        }

        Ok(Self {
            shards,
            config,
            validator_info: ok!(ValidatorInfo::load_from(child_slice)),
            prev_blocks: ok!(AugDict::load_from(child_slice)),
            after_key_block: ok!(child_slice.load_bit()),
            last_key_block: ok!(Option::<BlockRef>::load_from(child_slice)),
            block_create_stats: if flags & 0b001 != 0 {
                if ok!(child_slice.load_u8()) != Self::BLOCK_STATS_TAG {
                    return Err(Error::InvalidTag);
                }
                Some(ok!(Dict::load_from(child_slice)))
            } else {
                None
            },
            global_balance: ok!(CurrencyCollection::load_from(slice)),
        })
    }
}

/// Brief validator info.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
pub struct ValidatorInfo {
    /// Last 4 bytes of the hash of the validator list.
    pub validator_list_hash_short: u32,
    /// Seqno of the catchain session.
    pub catchain_seqno: u32,
    /// Whether the value of catchain seqno has been incremented
    /// and will it also be incremented in the next block.
    pub nx_cc_updated: bool,
}

/// Brief validator basic info.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
pub struct ValidatorBaseInfo {
    /// Last 4 bytes of the hash of the validator list.
    pub validator_list_hash_short: u32,
    /// Seqno of the catchain session.
    pub catchain_seqno: u32,
}

/// Entry value for the [`OldMcBlocksInfo`] dictionary.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
pub struct KeyBlockRef {
    /// Whether the referenced block is a key block.
    pub is_key_block: bool,
    /// Reference to the block.
    pub block_ref: BlockRef,
}

/// Value augmentation for the [`OldMcBlocksInfo`] dictionary.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Store, Load)]
pub struct KeyMaxLt {
    /// Has key block in a subtree.
    pub has_key_block: bool,
    /// The maximum logical time in a subtree.
    pub max_end_lt: u64,
}

impl AugDictExtra for KeyMaxLt {
    fn comp_add(
        left: &mut CellSlice,
        right: &mut CellSlice,
        b: &mut CellBuilder,
        cx: &dyn CellContext,
    ) -> Result<(), Error> {
        let left = ok!(Self::load_from(left));
        let right = ok!(Self::load_from(right));
        Self {
            has_key_block: left.has_key_block || right.has_key_block,
            max_end_lt: std::cmp::max(left.max_end_lt, right.max_end_lt),
        }
        .store_into(b, cx)
    }
}

/// Block production statistics for the single validator.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[tlb(tag = "#4")]
pub struct CreatorStats {
    /// Masterchain block production statistics.
    pub mc_blocks: BlockCounters,
    /// Block production statistics for other workchains.
    pub shard_blocks: BlockCounters,
}

/// Block counters with absolute value and rates.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Store, Load)]
pub struct BlockCounters {
    /// Unix timestamp in seconds of the last counters update.
    pub updated_at: u32,
    /// Total counter value.
    pub total: u64,
    /// Scaled counter rate.
    pub cnt2048: u64,
    /// Scaled counter rate (better precision).
    pub cnt65536: u64,
}
