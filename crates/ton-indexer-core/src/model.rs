//! Stable block identity and hash types.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use thiserror::Error;

/// A 256-bit hash serialized as lowercase hexadecimal.
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Hash256([u8; 32]);

impl Hash256 {
    /// The all-zero hash.
    pub const ZERO: Self = Self([0; 32]);

    /// Creates a hash from its bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the hash bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consumes the hash and returns its bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for Hash256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for Hash256 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

/// Error returned when parsing a 256-bit hexadecimal hash.
#[derive(Debug, Error)]
pub enum HashParseError {
    /// The input does not contain exactly 64 hexadecimal characters.
    #[error("expected 64 hexadecimal characters, got {0}")]
    InvalidLength(usize),
    /// The input contains invalid hexadecimal.
    #[error("invalid hexadecimal hash: {0}")]
    InvalidHex(#[from] hex::FromHexError),
}

impl FromStr for Hash256 {
    type Err = HashParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 {
            return Err(HashParseError::InvalidLength(value.len()));
        }

        let mut bytes = [0; 32];
        hex::decode_to_slice(value, &mut bytes)?;
        Ok(Self(bytes))
    }
}

impl Serialize for Hash256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Hash256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Full canonical identity of a TON block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BlockId {
    /// Workchain id (`-1` for masterchain, `0` for basechain).
    pub workchain: i32,
    /// Tagged 64-bit shard prefix.
    pub shard: u64,
    /// Sequence number within the shardchain.
    pub seqno: u32,
    /// Root cell representation hash.
    pub root_hash: Hash256,
    /// Hash of the serialized block `BoC`.
    pub file_hash: Hash256,
}

impl BlockId {
    /// Masterchain workchain id.
    pub const MASTERCHAIN_WORKCHAIN: i32 = -1;
    /// Full-shard prefix used by the masterchain and an unsplit workchain.
    pub const FULL_SHARD: u64 = 0x8000_0000_0000_0000;

    /// Returns whether this is a masterchain block id.
    #[must_use]
    pub const fn is_masterchain(self) -> bool {
        self.workchain == Self::MASTERCHAIN_WORKCHAIN
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "({},{:016x},{})",
            self.workchain, self.shard, self.seqno
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_json_is_hex() {
        let hash = Hash256::new([0xab; 32]);
        let json = serde_json::to_string(&hash).unwrap();
        assert_eq!(json, format!("\"{}\"", "ab".repeat(32)));
        assert_eq!(serde_json::from_str::<Hash256>(&json).unwrap(), hash);
    }
}
