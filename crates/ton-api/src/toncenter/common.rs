use anyhow::anyhow;
use num_bigint::{BigInt, ToBigInt as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrNumber {
    String(String),
    Number(i64),
}

impl StringOrNumber {
    pub fn to_bigint(&self) -> anyhow::Result<BigInt> {
        match self {
            Self::String(value) => value.parse::<BigInt>().map_err(Into::into),
            Self::Number(value) => value
                .to_bigint()
                .ok_or_else(|| anyhow!("cannot convert {value} to bigint")),
        }
    }
}
