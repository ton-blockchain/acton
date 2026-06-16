use base64::Engine;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use tycho_types::boc::Boc;
use tycho_types::models::{IntAddr, StdAddr};
use tycho_types::prelude::HashBytes;

#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default)]
pub struct BocBytes(pub Vec<u8>);

impl BocBytes {
    #[must_use]
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.0)
    }

    pub fn from_base64(s: &str) -> anyhow::Result<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(s)?;
        Ok(Self(bytes))
    }

    pub fn hash(&self) -> anyhow::Result<Hash256> {
        let cell = Boc::decode(self)?;
        Ok(Hash256(*cell.repr_hash().as_array()))
    }
}

impl From<Vec<u8>> for BocBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl From<BocBytes> for Vec<u8> {
    fn from(value: BocBytes) -> Self {
        value.0
    }
}

impl AsRef<[u8]> for BocBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for BocBytes {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BocBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Serialize for BocBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_base64())
    }
}

impl<'de> Deserialize<'de> for BocBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        BocBytes::from_base64(&value).map_err(serde::de::Error::custom)
    }
}

impl ToSql for BocBytes {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Blob(&self.0)))
    }
}

impl FromSql for BocBytes {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Blob(bytes) => Ok(BocBytes(bytes.to_vec())),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default)]
pub struct Hash256(pub [u8; 32]);

impl Hash256 {
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> anyhow::Result<Self> {
        let bytes = hex::decode(s)?;
        if bytes.len() != 32 {
            anyhow::bail!("Invalid hash length");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    pub fn from_base64(s: &str) -> anyhow::Result<Self> {
        let bytes = base64::engine::general_purpose::STANDARD.decode(s)?;
        if bytes.len() != 32 {
            anyhow::bail!("Invalid hash length");
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.0)
    }
}

impl Serialize for Hash256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Hash256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Hash256::from_hex(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Default)]
pub struct Addr {
    pub workchain: i32,
    pub addr: [u8; 32],
}

impl FromStr for Addr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((workchain, addr_hex)) = s.split_once(':') else {
            anyhow::bail!("Invalid address format: expected '<workchain>:<64-hex-bytes>'");
        };

        let workchain = workchain.parse::<i32>()?;
        let bytes = hex::decode(addr_hex)?;
        if bytes.len() != 32 {
            anyhow::bail!("Invalid address length");
        }

        let mut addr = [0u8; 32];
        addr.copy_from_slice(&bytes);
        Ok(Self { workchain, addr })
    }
}

impl Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.workchain, hex::encode(self.addr))
    }
}

impl From<IntAddr> for Addr {
    fn from(value: IntAddr) -> Self {
        Self::from(&value)
    }
}

impl From<&IntAddr> for Addr {
    fn from(value: &IntAddr) -> Self {
        let mut bytes = [0u8; 32];
        let (workchain, address) = match value {
            IntAddr::Std(std) => (std.workchain as i32, std.address.0),
            IntAddr::Var(var) => (var.workchain, {
                // skipped from TVM 11
                [0u8; 32]
            }),
        };
        bytes.copy_from_slice(&address);
        Self {
            workchain,
            addr: bytes,
        }
    }
}

impl From<Addr> for IntAddr {
    fn from(value: Addr) -> Self {
        Self::from(&value)
    }
}

impl From<&Addr> for IntAddr {
    fn from(value: &Addr) -> Self {
        Self::Std(StdAddr::new(
            value.workchain.try_into().unwrap_or_default(),
            HashBytes(value.addr),
        ))
    }
}

impl Serialize for Addr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Addr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Addr::from_str(&value).map_err(serde::de::Error::custom)
    }
}

pub type Seqno = u32;
pub type Lt = u64;

#[cfg(test)]
mod tests {
    use super::{Addr, BocBytes, Hash256};

    #[test]
    fn hash256_serializes_as_hex_string() {
        let hash = Hash256([0xAB; 32]);
        let json = serde_json::to_string(&hash).expect("serialize hash");
        assert_eq!(
            json,
            "\"abababababababababababababababababababababababababababababababab\""
        );
    }

    #[test]
    fn hash256_deserializes_from_hex_string() {
        let json = "\"cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\"";
        let parsed: Hash256 = serde_json::from_str(json).expect("deserialize hash");
        assert_eq!(parsed, Hash256([0xCD; 32]));
    }

    #[test]
    fn hash256_rejects_non_string_json() {
        let bytes = vec![7_u8; 32];
        let json = serde_json::to_string(&bytes).expect("serialize bytes");
        let parsed: Result<Hash256, _> = serde_json::from_str(&json);
        assert!(parsed.is_err(), "array JSON must be rejected");
    }

    #[test]
    fn boc_bytes_serializes_as_base64_string() {
        let bytes = BocBytes(vec![1_u8, 2_u8, 3_u8]);
        let json = serde_json::to_string(&bytes).expect("serialize boc bytes");
        assert_eq!(json, "\"AQID\"");
    }

    #[test]
    fn boc_bytes_deserializes_from_base64_string() {
        let parsed: BocBytes = serde_json::from_str("\"AAECAw==\"").expect("deserialize boc bytes");
        assert_eq!(parsed, BocBytes(vec![0_u8, 1_u8, 2_u8, 3_u8]));
    }

    #[test]
    fn boc_bytes_rejects_non_string_json() {
        let bytes = vec![1_u8, 2_u8, 3_u8];
        let json = serde_json::to_string(&bytes).expect("serialize bytes");
        let parsed: Result<BocBytes, _> = serde_json::from_str(&json);
        assert!(parsed.is_err(), "array JSON must be rejected");
    }

    #[test]
    fn addr_serializes_as_workchain_hex_string() {
        let addr = Addr {
            workchain: -1,
            addr: [0xAB; 32],
        };
        let json = serde_json::to_string(&addr).expect("serialize addr");
        assert_eq!(
            json,
            "\"-1:abababababababababababababababababababababababababababababababab\""
        );
    }

    #[test]
    fn addr_deserializes_from_workchain_hex_string() {
        let json = "\"0:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\"";
        let parsed: Addr = serde_json::from_str(json).expect("deserialize addr");
        assert_eq!(
            parsed,
            Addr {
                workchain: 0,
                addr: [0xCD; 32]
            }
        );
    }

    #[test]
    fn addr_rejects_non_string_json() {
        let json = r#"{ "workchain": 0, "addr": [1, 2, 3] }"#;
        let parsed: Result<Addr, _> = serde_json::from_str(json);
        assert!(parsed.is_err(), "object JSON must be rejected");
    }
}
