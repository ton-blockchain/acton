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
    Cell { cell: JsonBoc },
    #[serde(rename = "tvm.stackEntrySlice")]
    Slice { slice: JsonBoc },
    #[serde(rename = "tvm.stackEntryBuilder")]
    Builder { builder: JsonBoc },
    #[serde(rename = "tvm.stackEntryTuple")]
    Tuple { tuple: JsonTuple },
    #[serde(rename = "tvm.stackEntryList", alias = "list")]
    List { list: JsonList },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonNumber {
    pub number: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum JsonBoc {
    String(String),
    Object { bytes: String },
}

impl JsonBoc {
    fn as_str(&self) -> &str {
        match self {
            JsonBoc::String(value) => value,
            JsonBoc::Object { bytes } => bytes,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonTuple {
    pub elements: Vec<JsonStackEntry>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonList {
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
            cell: JsonBoc::String(Boc::encode_base64(c)),
        }),
        TupleItem::Slice(c) => Ok(JsonStackEntry::Slice {
            slice: JsonBoc::String(Boc::encode_base64(c)),
        }),
        TupleItem::Cont(cont) => Ok(JsonStackEntry::Slice {
            slice: JsonBoc::String(Boc::encode_base64(&cont.code)),
        }),
        TupleItem::Builder(c) => Ok(JsonStackEntry::Builder {
            builder: JsonBoc::String(Boc::encode_base64(c)),
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
            let c = Boc::decode_base64(cell.as_str()).context("Failed to decode cell BOC")?;
            Ok(TupleItem::Cell(c))
        }
        JsonStackEntry::Slice { slice } => {
            let c = Boc::decode_base64(slice.as_str()).context("Failed to decode slice BOC")?;
            Ok(TupleItem::Slice(c))
        }
        JsonStackEntry::Builder { builder } => {
            let c = Boc::decode_base64(builder.as_str()).context("Failed to decode builder BOC")?;
            Ok(TupleItem::Builder(c))
        }
        JsonStackEntry::Tuple { tuple } => {
            let mut elements = Vec::new();
            for el in tuple.elements {
                elements.push(json_to_item(el)?);
            }
            Ok(TupleItem::Tuple(Tuple(elements)))
        }
        JsonStackEntry::List { list } => {
            let mut elements = Vec::new();
            for el in list.elements {
                elements.push(json_to_item(el)?);
            }
            Ok(TupleItem::Tuple(Tuple(elements)))
        }
    }
}

fn json_to_mixed_item(value: Value) -> anyhow::Result<TupleItem> {
    match json_to_legacy_item(value.clone()) {
        Ok(item) => Ok(item),
        Err(legacy_err) => {
            let entry: JsonStackEntry = serde_json::from_value(value)
                .with_context(|| format!("Failed to parse stack entry as legacy or std format. Legacy error: {legacy_err}"))?;
            json_to_item(entry)
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

    let normalized_type = type_str.to_ascii_lowercase();
    let type_key = normalized_type
        .strip_prefix("tvm.")
        .unwrap_or(normalized_type.as_str());

    match type_key {
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
            let i = if let Some(hex) = s.strip_prefix("-0x").or_else(|| s.strip_prefix("-0X")) {
                -BigInt::parse_bytes(hex.as_bytes(), 16).context("Failed to parse hex BigInt")?
            } else if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                BigInt::parse_bytes(hex.as_bytes(), 16).context("Failed to parse hex BigInt")?
            } else {
                s.parse::<BigInt>().context("Failed to parse BigInt")?
            };
            Ok(TupleItem::Int(i))
        }
        "cell" => {
            let bytes = legacy_stack_bytes(val, "cell")?;
            let c = Boc::decode_base64(bytes)?;
            Ok(TupleItem::Cell(c))
        }
        "slice" => {
            let bytes = legacy_stack_bytes(val, "slice")?;
            let c = Boc::decode_base64(bytes)?;
            Ok(TupleItem::Slice(c))
        }
        "builder" => {
            let bytes = legacy_stack_bytes(val, "builder")?;
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
                .map(|v| json_to_mixed_item(v.clone()))
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(TupleItem::Tuple(Tuple(items)))
        }
        _ => anyhow::bail!("Unsupported legacy stack entry type: {type_str}"),
    }
}

fn legacy_stack_bytes<'a>(value: &'a Value, stack_type: &str) -> anyhow::Result<&'a str> {
    if let Some(bytes) = value.as_str() {
        return Ok(bytes);
    }

    value
        .get("bytes")
        .and_then(Value::as_str)
        .with_context(|| format!("{stack_type} must be a base64 string or an object with `bytes`"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::TupleItem;
    use num_bigint::BigInt;
    use tycho_types::cell::CellBuilder;

    #[test]
    fn test_stack_json_roundtrip() {
        let items = vec![TupleItem::Null, TupleItem::Int(BigInt::from(123))];

        let tuple = Tuple(items);
        let json = stack_to_json(&tuple).unwrap();
        let back = json_to_stack(json).unwrap();

        assert_eq!(tuple, back);
    }

    #[test]
    fn test_legacy_stack_accepts_ton_ton_cell_type_names() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        let cell = builder.build().unwrap();

        let boc = Boc::encode_base64(&cell);

        assert_eq!(
            json_to_legacy_item(serde_json::json!(["tvm.Cell", boc])).unwrap(),
            TupleItem::Cell(cell.clone())
        );
        assert_eq!(
            json_to_legacy_item(serde_json::json!(["tvm.Slice", boc])).unwrap(),
            TupleItem::Slice(cell.clone())
        );
        assert_eq!(
            json_to_legacy_item(serde_json::json!(["tvm.Builder", boc])).unwrap(),
            TupleItem::Builder(cell)
        );
    }

    #[test]
    fn test_legacy_stack_accepts_negative_hex_numbers() {
        assert_eq!(
            json_to_legacy_item(serde_json::json!(["num", "-0x2a"])).unwrap(),
            TupleItem::Int(BigInt::from(-42))
        );
    }

    #[test]
    fn test_std_stack_accepts_list_entries() {
        assert_eq!(
            json_to_stack(vec![serde_json::json!({
                "@type": "tvm.stackEntryList",
                "list": {
                    "@type": "tvm.list",
                    "elements": [
                        {
                            "@type": "tvm.stackEntryNumber",
                            "number": {
                                "@type": "tvm.numberDecimal",
                                "number": "7"
                            }
                        }
                    ]
                }
            })])
            .unwrap(),
            Tuple(vec![TupleItem::Tuple(Tuple(vec![TupleItem::Int(
                BigInt::from(7)
            )]))])
        );
    }

    #[test]
    fn test_legacy_stack_accepts_toncenter_mixed_list_entries() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        let cell = builder.build().unwrap();
        let boc = Boc::encode_base64(&cell);

        assert_eq!(
            json_to_legacy_stack(vec![serde_json::json!([
                "list",
                {
                    "@type": "tvm.list",
                    "elements": [
                        {
                            "@type": "tvm.stackEntryTuple",
                            "tuple": {
                                "@type": "tvm.tuple",
                                "elements": [
                                    {
                                        "@type": "tvm.stackEntryNumber",
                                        "number": {
                                            "@type": "tvm.numberDecimal",
                                            "number": "11"
                                        }
                                    },
                                    {
                                        "@type": "tvm.stackEntryNumber",
                                        "number": {
                                            "@type": "tvm.numberDecimal",
                                            "number": "22"
                                        }
                                    },
                                    {
                                        "@type": "tvm.stackEntryCell",
                                        "cell": {
                                            "@type": "tvm.cell",
                                            "bytes": boc
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            ])])
            .unwrap(),
            Tuple(vec![TupleItem::Tuple(Tuple(vec![TupleItem::Tuple(
                Tuple(vec![
                    TupleItem::Int(BigInt::from(11)),
                    TupleItem::Int(BigInt::from(22)),
                    TupleItem::Cell(cell)
                ])
            )]))])
        );
    }
}
