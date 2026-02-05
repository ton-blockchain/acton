use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub type BocBytes = Vec<u8>;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, PartialOrd, Ord)]
pub struct Hash256(pub [u8; 32]);

impl Hash256 {
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

    pub fn to_base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Addr {
    pub workchain: i32,
    pub addr: [u8; 32],
}

impl Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.workchain, hex::encode(self.addr))
    }
}

pub type Seqno = u32;
pub type Lt = u64;
