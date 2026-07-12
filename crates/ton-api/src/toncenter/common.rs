use anyhow::anyhow;
use num_bigint::{BigInt, ToBigInt as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrNumber {
    String(String),
    Number(i64),
    Unsigned(u64),
}

impl StringOrNumber {
    pub fn to_bigint(&self) -> anyhow::Result<BigInt> {
        match self {
            Self::String(value) => value.parse::<BigInt>().map_err(Into::into),
            Self::Number(value) => value
                .to_bigint()
                .ok_or_else(|| anyhow!("cannot convert {value} to bigint")),
            Self::Unsigned(value) => value
                .to_bigint()
                .ok_or_else(|| anyhow!("cannot convert {value} to bigint")),
        }
    }

    pub fn to_i64(&self) -> anyhow::Result<i64> {
        match self {
            Self::String(value) => value.parse::<i64>().map_err(Into::into),
            Self::Number(value) => Ok(*value),
            Self::Unsigned(value) => Ok(i64::try_from(*value)?),
        }
    }

    pub fn to_i32(&self) -> anyhow::Result<i32> {
        Ok(i32::try_from(self.to_i64()?)?)
    }

    pub fn to_u64(&self) -> anyhow::Result<u64> {
        match self {
            Self::Unsigned(value) => Ok(*value),
            _ => Ok(u64::try_from(self.to_i64()?)?),
        }
    }

    pub fn to_u32(&self) -> anyhow::Result<u32> {
        Ok(u32::try_from(self.to_i64()?)?)
    }

    pub fn to_usize(&self) -> anyhow::Result<usize> {
        Ok(usize::try_from(self.to_i64()?)?)
    }
}

impl From<i32> for StringOrNumber {
    fn from(value: i32) -> Self {
        Self::Number(i64::from(value))
    }
}

impl From<u32> for StringOrNumber {
    fn from(value: u32) -> Self {
        Self::Number(i64::from(value))
    }
}
