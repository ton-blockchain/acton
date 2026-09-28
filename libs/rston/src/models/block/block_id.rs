use std::str::FromStr;

use crate::cell::*;
use crate::error::{Error, ParseBlockIdError};
use crate::models::Addr;

/// Full block id.
#[derive(Debug, Default, Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockId {
    /// Block shard ident.
    pub shard: ShardIdent,
    /// Block number in shard.
    pub seqno: u32,
    /// Representation hash of the root cell of the block.
    pub root_hash: HashBytes,
    /// Hash of the BOC encoded root cell of the block.
    pub file_hash: HashBytes,
}

impl BlockId {
    /// The number of data bits that this struct occupies.
    pub const BITS: u16 = ShardIdent::BITS + 32 + 256 + 256;

    /// Returns `true` if this block id is for a masterchain block.
    ///
    /// See [`ShardIdent::MASTERCHAIN`]
    #[inline]
    pub const fn is_masterchain(&self) -> bool {
        self.shard.is_masterchain()
    }

    /// Returns short block id.
    pub const fn as_short_id(&self) -> BlockIdShort {
        BlockIdShort {
            shard: self.shard,
            seqno: self.seqno,
        }
    }
}

impl FromStr for BlockId {
    type Err = ParseBlockIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseBlockIdError::Empty);
        }

        let mut parts = s.split(':');
        let workchain = match parts.next() {
            Some(wc) => match wc.parse::<i32>() {
                Ok(wc) => wc,
                Err(_) => return Err(ParseBlockIdError::InvalidShardIdent),
            },
            None => return Err(ParseBlockIdError::Empty),
        };

        let shard = 'shard: {
            if let Some(prefix) = parts.next()
                && let Ok(prefix) = u64::from_str_radix(prefix, 16)
                && let Some(shard) = ShardIdent::new(workchain, prefix)
            {
                break 'shard shard;
            }
            return Err(ParseBlockIdError::InvalidShardIdent);
        };

        let seqno = 'seqno: {
            if let Some(seqno) = parts.next()
                && let Ok(seqno) = seqno.parse::<u32>()
            {
                break 'seqno seqno;
            }
            return Err(ParseBlockIdError::InvalidSeqno);
        };

        let mut result = Self {
            shard,
            seqno,
            ..Default::default()
        };

        'hash: {
            if let Some(hash) = parts.next()
                && hex::decode_to_slice(hash, &mut result.root_hash.0).is_ok()
            {
                break 'hash;
            }
            return Err(ParseBlockIdError::InvalidRootHash);
        }

        'hash: {
            if let Some(hash) = parts.next()
                && hex::decode_to_slice(hash, &mut result.file_hash.0).is_ok()
            {
                break 'hash;
            }
            return Err(ParseBlockIdError::InvalidFileHash);
        }

        if parts.next().is_none() {
            Ok(result)
        } else {
            Err(ParseBlockIdError::UnexpectedPart)
        }
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}:{}:{}:{}",
            self.shard, self.seqno, self.root_hash, self.file_hash,
        ))
    }
}

/// Short block id.
#[derive(Debug, Default, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockIdShort {
    /// Block shard ident.
    pub shard: ShardIdent,
    /// Block number in shard.
    pub seqno: u32,
}

impl BlockIdShort {
    /// Returns `true` if this block id is for a masterchain block.
    ///
    /// See [`ShardIdent::MASTERCHAIN`]
    #[inline]
    pub const fn is_masterchain(&self) -> bool {
        self.shard.is_masterchain()
    }

    /// Returns a previous block id.
    pub fn saturating_prev(&self) -> Self {
        self.saturating_sub(1)
    }

    /// Returns a next block id.
    pub fn saturating_next(&self) -> Self {
        self.saturating_add(1)
    }

    /// Returns a new block id with the seqno
    /// increased at most by the specified value.
    pub fn saturating_add(&self, seqno: u32) -> Self {
        Self {
            shard: self.shard,
            seqno: self.seqno.saturating_add(seqno),
        }
    }

    /// Returns a new block id with the seqno
    /// decreased at most by the specified value.
    pub fn saturating_sub(&self, seqno: u32) -> Self {
        Self {
            shard: self.shard,
            seqno: self.seqno.saturating_sub(seqno),
        }
    }
}

impl std::fmt::Display for BlockIdShort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}:{}", self.shard, self.seqno))
    }
}

impl From<(ShardIdent, u32)> for BlockIdShort {
    #[inline]
    fn from((shard, seqno): (ShardIdent, u32)) -> Self {
        Self { shard, seqno }
    }
}

impl From<BlockIdShort> for (ShardIdent, u32) {
    #[inline]
    fn from(value: BlockIdShort) -> Self {
        (value.shard, value.seqno)
    }
}

/// Shard ident.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ShardIdent {
    workchain: i32,
    prefix: u64,
}

impl Default for ShardIdent {
    #[inline]
    fn default() -> Self {
        ShardIdent::MASTERCHAIN
    }
}

impl std::fmt::Display for ShardIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}:{:016x}", self.workchain, self.prefix))
    }
}

impl std::fmt::Debug for ShardIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl ShardIdent {
    /// The prefix for the full shard.
    pub const PREFIX_FULL: u64 = 0x8000000000000000;

    /// Max possible shard split depth.
    pub const MAX_SPLIT_DEPTH: u8 = 60;

    /// The number of data bits that this struct occupies.
    pub const BITS: u16 = 8 + 32 + 64;

    const UNUSED_BITS_MASK: u64 = !0 >> (Self::MAX_SPLIT_DEPTH + 1);

    /// Masterchain shard ident.
    pub const MASTERCHAIN: Self = Self::new_full(-1);
    /// Base workchain shard ident.
    pub const BASECHAIN: Self = Self::new_full(0);

    /// Tries to create a new shard ident from parts.
    pub const fn new(workchain: i32, prefix: u64) -> Option<Self> {
        if prefix == 0
            || prefix & Self::UNUSED_BITS_MASK != 0
            || workchain == -1 && prefix != Self::PREFIX_FULL
        {
            return None;
        }
        Some(Self { workchain, prefix })
    }

    /// Creates a new full shard ident for the specified workchain.
    pub const fn new_full(workchain: i32) -> Self {
        Self {
            workchain,
            prefix: Self::PREFIX_FULL,
        }
    }

    /// Creates a new shard ident from parts.
    ///
    /// # Safety
    ///
    /// The following must be true:
    /// - prefix must not be zero.
    /// - low bits must not be used (see [`MAX_SPLIT_DEPTH`]).
    ///
    /// [`MAX_SPLIT_DEPTH`]: Self::MAX_SPLIT_DEPTH
    pub const unsafe fn new_unchecked(workchain: i32, prefix: u64) -> Self {
        Self { workchain, prefix }
    }

    /// Returns the shard workchain.
    #[inline]
    pub const fn workchain(&self) -> i32 {
        self.workchain
    }

    /// Returns the shard prefix.
    #[inline]
    pub const fn prefix(&self) -> u64 {
        self.prefix
    }

    /// Returns `true` if this shard is a masterchain shard.
    ///
    /// See [`MASTERCHAIN`]
    ///
    /// [`MASTERCHAIN`]: Self::MASTERCHAIN
    #[inline]
    pub const fn is_masterchain(&self) -> bool {
        self.workchain == Self::MASTERCHAIN.workchain
    }

    /// Returns `true` if this shard could not be merged further.
    ///
    /// See [`PREFIX_FULL`]
    ///
    /// [`PREFIX_FULL`]: Self::PREFIX_FULL
    #[inline]
    pub const fn is_full(&self) -> bool {
        self.prefix == Self::PREFIX_FULL
    }

    /// Returns `true` if this shard is the left child of the parent shard.
    ///
    /// NOTE: Full shard is left.
    pub const fn is_left_child(&self) -> bool {
        !self.is_right_child()
    }

    /// Returns `true` if this shard is the right child of the parent shard.
    ///
    /// NOTE: Full shard is left.
    pub const fn is_right_child(&self) -> bool {
        (self.prefix & (self.prefix_tag() << 1)) != 0
    }

    /// Returns `true` if the current shard is somewhere in the parents
    /// hierarchy of the specified shard.
    pub const fn is_ancestor_of(&self, shard: &Self) -> bool {
        if self.workchain != shard.workchain {
            return false;
        }

        let ancestor_tag = self.prefix_tag();
        let shard_tag = shard.prefix_tag();
        ancestor_tag >= shard_tag
            && ((self.prefix ^ shard.prefix) & (self.prefix_tag_mask() << 1)) == 0
    }

    /// Returns `true` if the current shard is the direct parent of the specified shard.
    pub const fn is_parent_of(&self, child: &Self) -> bool {
        if self.workchain != child.workchain || child.is_full() {
            return false;
        }

        let tag = child.prefix_tag();
        let child_parent_prefix = (child.prefix - tag) | (tag << 1);
        child_parent_prefix == self.prefix
    }

    /// Returns `true` if the current shard is the direct child of the specified shard.
    pub const fn is_child_of(&self, parent: &Self) -> bool {
        parent.is_parent_of(self)
    }

    /// Returns `true` if one shard fully includes another.
    pub const fn intersects(&self, other: &Self) -> bool {
        if self.workchain != other.workchain {
            return false;
        }

        let self_tag_mask = self.prefix_tag();
        let other_tag_mask = other.prefix_tag();

        let longest_prefix_tag = if other_tag_mask > self_tag_mask {
            other_tag_mask
        } else {
            self_tag_mask
        };

        let longest_bits = (!longest_prefix_tag + 1) << 1;
        let equal_bits = self.prefix ^ other.prefix;
        equal_bits & longest_bits == 0
    }

    /// Returns the parent shard of the current shard.
    ///
    /// Returns `None` for the full shard.
    pub const fn merge(&self) -> Option<Self> {
        if self.is_full() {
            None
        } else {
            let tag = self.prefix_tag();
            Some(Self {
                workchain: self.workchain,
                prefix: (self.prefix - tag) | (tag << 1),
            })
        }
    }

    /// Splits the current shard into two children.
    ///
    /// Returns `None` for the shard with `depth > MAX_SPLIT_DEPTH`.
    pub const fn split(&self) -> Option<(Self, Self)> {
        if self.prefix & ((Self::UNUSED_BITS_MASK << 1) + 1) != 0 {
            None
        } else {
            let tag = self.prefix_tag() >> 1;
            let left = Self {
                workchain: self.workchain,
                prefix: self.prefix - tag,
            };
            let right = Self {
                workchain: self.workchain,
                prefix: self.prefix + tag,
            };
            Some((left, right))
        }
    }

    /// Returns the opposite shard if applicable.
    ///
    /// Returns `None` for the shard with `depth == 0`.
    pub const fn opposite(&self) -> Option<Self> {
        if self.is_full() {
            None
        } else {
            let last_bit_mask = self.prefix_tag() << 1;
            Some(Self {
                workchain: self.workchain,
                prefix: self.prefix ^ last_bit_mask,
            })
        }
    }

    /// Returns shard prefix len in bits.
    pub const fn prefix_len(&self) -> u16 {
        match self.prefix {
            0 => 64,
            _ => 63 - self.prefix.trailing_zeros() as u16,
        }
    }

    /// Returns whether the shard depth is in the possible range.
    pub const fn can_split(&self) -> bool {
        self.prefix_len() < Self::MAX_SPLIT_DEPTH as u16
    }

    /// Returns `true` if the specified address could be stored in the current shard.
    pub fn contains_address<T: Addr>(&self, address: &T) -> bool {
        self.workchain == address.workchain()
            && self.contains_account_prefix(&address.prefix().to_be_bytes())
    }

    /// Returns `true` if the specified account could be stored in the current shard.
    pub const fn contains_account(&self, account: &HashBytes) -> bool {
        self.contains_account_prefix(account.first_chunk())
    }

    const fn contains_account_prefix(&self, account: &[u8; 8]) -> bool {
        let mut bits = self.prefix_len();

        let mut byte = 0;
        let prefix = self.prefix.to_be_bytes();
        while bits > 0 && byte < 8 {
            // Get next bytes
            let prefix_byte = prefix[byte];
            let account_byte = account[byte];

            if bits >= 8 {
                // Early exit if they are not equal
                if prefix_byte != account_byte {
                    return false;
                }

                // Move to the next byte
                bits -= 8;
                byte += 1;
            } else {
                // Check if the remaining amount of bits are equal
                let mask = 0xff << (8 - bits);
                return (prefix_byte ^ account_byte) & mask == 0;
            }
        }

        // All bits were checked without exit => contains account
        true
    }

    #[inline]
    const fn prefix_tag(&self) -> u64 {
        self.prefix & self.prefix_tag_mask()
    }

    #[inline]
    const fn prefix_tag_mask(&self) -> u64 {
        !(self.prefix) + 1
    }
}

impl Store for ShardIdent {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let prefix_len = self.prefix_len() as u8;
        let prefix_without_tag = self.prefix - self.prefix_tag();
        ok!(builder.store_u8(prefix_len));
        ok!(builder.store_u32(self.workchain as u32));
        builder.store_u64(prefix_without_tag)
    }
}

impl<'a> Load<'a> for ShardIdent {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let prefix_len = ok!(slice.load_u8());
        if prefix_len > Self::MAX_SPLIT_DEPTH {
            return Err(Error::IntOverflow);
        }

        let workchain = ok!(slice.load_u32()) as i32;
        let prefix_without_tag = ok!(slice.load_u64());

        let tag = 1u64 << (63 - prefix_len);
        let prefix = (prefix_without_tag & (!tag + 1)) | tag;

        Ok(Self { workchain, prefix })
    }
}

impl FromStr for ShardIdent {
    type Err = ParseBlockIdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let workchain = match parts.next() {
            Some(wc) => match wc.parse::<i32>() {
                Ok(wc) => wc,
                Err(_) => return Err(ParseBlockIdError::InvalidShardIdent),
            },
            None => return Err(ParseBlockIdError::Empty),
        };

        let shard = 'shard: {
            if let Some(prefix) = parts.next()
                && let Ok(prefix) = u64::from_str_radix(prefix, 16)
                && let Some(shard) = ShardIdent::new(workchain, prefix)
            {
                break 'shard shard;
            }
            return Err(ParseBlockIdError::InvalidShardIdent);
        };

        Ok(shard)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ShardIdent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.collect_str(&self)
        } else {
            (self.workchain, self.prefix).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ShardIdent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Unexpected, Visitor};

        struct ShardIdentVisitor;

        impl Visitor<'_> for ShardIdentVisitor {
            type Value = ShardIdent;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a shard ident")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                ShardIdent::from_str(value).map_err(Error::custom)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let Ok(string) = std::str::from_utf8(v) else {
                    return Err(Error::invalid_value(Unexpected::Bytes(v), &self));
                };
                self.visit_str(string)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(ShardIdentVisitor)
        } else {
            let (workchain, prefix): (i32, u64) = serde::Deserialize::deserialize(deserializer)?;
            ShardIdent::new(workchain, prefix).ok_or_else(|| Error::custom("invalid shard prefix"))
        }
    }
}
