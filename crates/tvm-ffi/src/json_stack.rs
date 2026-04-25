use crate::stack::{Tuple, TupleItem};
use anyhow::Context;
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::Mul;
use tycho_types::boc::Boc;

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "@type")]
pub enum JsonStackEntry {
    #[serde(rename = "tvm.stackEntryNull")]
    Null {},
    #[serde(rename = "tvm.stackEntryNumber")]
    Number { number: JsonNumber },
    #[serde(rename = "tvm.stackEntryCell")]
    Cell { cell: String },
    #[serde(rename = "tvm.stackEntrySlice")]
    Slice { slice: String },
    #[serde(rename = "tvm.stackEntryBuilder")]
    Builder { builder: String },
    #[serde(rename = "tvm.stackEntryTuple")]
    Tuple { tuple: JsonTuple },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonNumber {
    pub number: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonTuple {
    pub elements: Vec<JsonStackEntry>,
}

pub fn stack_to_json(stack: &Tuple) -> anyhow::Result<Vec<Value>> {
    let mut entries = Vec::new();
    for item in &stack.0 {
        entries.push(serde_json::to_value(item_to_json(item)?)?);
    }
    Ok(entries)
}

pub fn legacy_stack_to_json(stack: &Tuple) -> anyhow::Result<Vec<Value>> {
    let mut entries = Vec::new();
    for item in &stack.0 {
        entries.push(legacy_item_to_json(item)?);
    }
    Ok(entries)
}

fn item_to_json(item: &TupleItem) -> anyhow::Result<JsonStackEntry> {
    match item {
        TupleItem::Null => Ok(JsonStackEntry::Null {}),
        TupleItem::Int(i) => Ok(JsonStackEntry::Number {
            number: JsonNumber {
                number: i.to_string(),
            },
        }),
        TupleItem::Nan => anyhow::bail!("NaN not supported in JSON stack"),
        TupleItem::Cell(c) => Ok(JsonStackEntry::Cell {
            cell: Boc::encode_base64(c),
        }),
        TupleItem::Slice(c) => Ok(JsonStackEntry::Slice {
            slice: Boc::encode_base64(c),
        }),
        TupleItem::Cont(cont) => Ok(JsonStackEntry::Slice {
            slice: Boc::encode_base64(&cont.code),
        }),
        TupleItem::Builder(c) => Ok(JsonStackEntry::Builder {
            builder: Boc::encode_base64(c),
        }),
        TupleItem::Tuple(t) => {
            let mut elements = Vec::new();
            for it in &t.0 {
                elements.push(item_to_json(it)?);
            }
            Ok(JsonStackEntry::Tuple {
                tuple: JsonTuple { elements },
            })
        }
        TupleItem::TypedTuple { inner, .. } => item_to_json(&TupleItem::Tuple(inner.clone())),
    }
}

pub fn legacy_item_to_json(item: &TupleItem) -> anyhow::Result<Value> {
    match item {
        TupleItem::Null => Ok(serde_json::json!(["null", null])),
        TupleItem::Int(i) => {
            if i < &BigInt::from(0u64) {
                return Ok(serde_json::json!(["num", format!("-0x{:x}", i.mul(-1))]));
            }
            Ok(serde_json::json!(["num", format!("0x{i:x}")]))
        }
        TupleItem::Cont(cont) => {
            Ok(serde_json::json!(["cont", { "bytes": Boc::encode_base64(&cont.code) }]))
        }
        TupleItem::Cell(c) => Ok(serde_json::json!(["cell", { "bytes": Boc::encode_base64(c) }])),
        TupleItem::Slice(c) => Ok(serde_json::json!(["slice", { "bytes": Boc::encode_base64(c) }])),
        TupleItem::Builder(c) => {
            Ok(serde_json::json!(["builder", { "bytes": Boc::encode_base64(c) }]))
        }
        TupleItem::Tuple(t) => {
            let elements =
                t.0.iter()
                    .map(legacy_item_to_json)
                    .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(serde_json::json!(["tuple", { "elements": elements }]))
        }
        TupleItem::TypedTuple { inner, .. } => {
            legacy_item_to_json(&TupleItem::Tuple(inner.clone()))
        }
        TupleItem::Nan => anyhow::bail!("NaN not supported in legacy JSON stack"),
    }
}

pub fn json_to_stack(entries: Vec<Value>) -> anyhow::Result<Tuple> {
    let mut items = Vec::new();
    for entry in entries {
        let entry: JsonStackEntry = serde_json::from_value(entry)?;
        items.push(json_to_item(entry)?);
    }
    Ok(Tuple(items))
}

pub fn json_to_legacy_stack(entries: Vec<Value>) -> anyhow::Result<Tuple> {
    let mut items = Vec::new();
    for entry in entries {
        items.push(json_to_legacy_item(entry)?);
    }
    Ok(Tuple(items))
}

fn json_to_item(entry: JsonStackEntry) -> anyhow::Result<TupleItem> {
    match entry {
        JsonStackEntry::Null {} => Ok(TupleItem::Null),
        JsonStackEntry::Number { number } => {
            let i = number
                .number
                .parse::<BigInt>()
                .context("Failed to parse number")?;
            Ok(TupleItem::Int(i))
        }
        JsonStackEntry::Cell { cell } => {
            let c = Boc::decode_base64(&cell).context("Failed to decode cell BOC")?;
            Ok(TupleItem::Cell(c))
        }
        JsonStackEntry::Slice { slice } => {
            let c = Boc::decode_base64(&slice).context("Failed to decode slice BOC")?;
            Ok(TupleItem::Slice(c))
        }
        JsonStackEntry::Builder { builder } => {
            let c = Boc::decode_base64(&builder).context("Failed to decode builder BOC")?;
            Ok(TupleItem::Builder(c))
        }
        JsonStackEntry::Tuple { tuple } => {
            let mut elements = Vec::new();
            for el in tuple.elements {
                elements.push(json_to_item(el)?);
            }
            Ok(TupleItem::Tuple(Tuple(elements)))
        }
    }
}

pub fn json_to_legacy_item(value: Value) -> anyhow::Result<TupleItem> {
    let arr = value
        .as_array()
        .context("Legacy stack entry must be an array")?;
    if arr.len() != 2 {
        anyhow::bail!("Legacy stack entry must have 2 elements");
    }
    let type_str = arr[0]
        .as_str()
        .context("Legacy stack entry type must be a string")?;
    let val = &arr[1];

    match type_str {
        "null" => Ok(TupleItem::Null),
        "num" => {
            let s = val
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| {
                    if val.is_number() {
                        Some(val.to_string())
                    } else {
                        None
                    }
                })
                .context("num value must be string or number")?;
            let i = if s.starts_with("0x") || s.starts_with("0X") {
                BigInt::parse_bytes(&s.as_bytes()[2..], 16).context("Failed to parse hex BigInt")?
            } else {
                s.parse::<BigInt>().context("Failed to parse BigInt")?
            };
            Ok(TupleItem::Int(i))
        }
        "cell" => {
            let bytes = val
                .get("bytes")
                .and_then(|v| v.as_str())
                .context("cell must have bytes")?;
            let c = Boc::decode_base64(bytes)?;
            Ok(TupleItem::Cell(c))
        }
        "slice" => {
            let bytes = val
                .get("bytes")
                .and_then(|v| v.as_str())
                .context("slice must have bytes")?;
            let c = Boc::decode_base64(bytes)?;
            Ok(TupleItem::Slice(c))
        }
        "builder" => {
            let bytes = val
                .get("bytes")
                .and_then(|v| v.as_str())
                .context("builder must have bytes")?;
            let c = Boc::decode_base64(bytes)?;
            Ok(TupleItem::Builder(c))
        }
        "tuple" => {
            let elements = val
                .get("elements")
                .and_then(|v| v.as_array())
                .context("tuple must have elements")?;
            let items = elements
                .iter()
                .map(|v| json_to_legacy_item(v.clone()))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(TupleItem::Tuple(Tuple(items)))
        }
        "cont" => {
            let bytes = val
                .get("bytes")
                .and_then(|v| v.as_str())
                .context("cont must have bytes")?;
            let c = Boc::decode_base64(bytes)?;
            Ok(TupleItem::Cont(crate::stack::ContData::from_code(c)))
        }
        "list" => {
            let elements = val
                .get("elements")
                .and_then(|v| v.as_array())
                .context("list must have elements")?;
            let items = elements
                .iter()
                .map(|v| json_to_legacy_item(v.clone()))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(TupleItem::Tuple(Tuple(items)))
        }
        _ => anyhow::bail!("Unsupported legacy stack entry type: {type_str}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::TupleItem;
    use num_bigint::BigInt;

    #[test]
    fn test_stack_json_roundtrip() {
        let items = vec![TupleItem::Null, TupleItem::Int(BigInt::from(123))];

        let tuple = Tuple(items);
        let json = stack_to_json(&tuple).unwrap();
        let back = json_to_stack(json).unwrap();

        assert_eq!(tuple, back);
    }
}
