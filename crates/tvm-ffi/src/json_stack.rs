use crate::stack::{Tuple, TupleItem};
use anyhow::Context;
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::ops::Mul;
use tycho_types::boc::Boc;

const TVM_SLICE_TYPE: &str = "tvm.slice";
const TVM_CELL_TYPE: &str = "tvm.cell";
const TVM_NUMBER_DECIMAL_TYPE: &str = "tvm.numberDecimal";
const TVM_TUPLE_TYPE: &str = "tvm.tuple";
const TVM_LIST_TYPE: &str = "tvm.list";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "@type", deny_unknown_fields)]
pub enum TvmStackEntry {
    #[serde(rename = "tvm.stackEntrySlice")]
    Slice { slice: TvmSlice },
    #[serde(rename = "tvm.stackEntryCell")]
    Cell { cell: TvmCell },
    #[serde(rename = "tvm.stackEntryNumber")]
    Number { number: TvmNumberDecimal },
    #[serde(rename = "tvm.stackEntryTuple")]
    Tuple { tuple: TvmTuple },
    #[serde(rename = "tvm.stackEntryList")]
    List { list: TvmList },
    #[serde(rename = "tvm.stackEntryUnsupported")]
    Unsupported {},
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TvmSlice {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub bytes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TvmCell {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub bytes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TvmNumberDecimal {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub number: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TvmTuple {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub elements: Vec<TvmStackEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TvmList {
    #[serde(rename = "@type")]
    pub type_field: String,
    pub elements: Vec<TvmStackEntry>,
}

pub fn legacy_stack_to_json(stack: &Tuple) -> anyhow::Result<Vec<Value>> {
    let mut entries = Vec::new();
    for item in &stack.0 {
        entries.push(legacy_item_to_json(item)?);
    }
    Ok(entries)
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

pub fn json_to_legacy_stack(entries: Vec<Value>) -> anyhow::Result<Tuple> {
    let mut items = Vec::new();
    for entry in entries {
        items.push(json_to_legacy_item(entry)?);
    }
    Ok(Tuple(items))
}

impl TvmStackEntry {
    #[must_use]
    pub fn number(value: impl ToString) -> Self {
        Self::Number {
            number: TvmNumberDecimal {
                type_field: TVM_NUMBER_DECIMAL_TYPE.to_owned(),
                number: value.to_string(),
            },
        }
    }

    #[must_use]
    pub fn cell(bytes: impl Into<String>) -> Self {
        Self::Cell {
            cell: TvmCell {
                type_field: TVM_CELL_TYPE.to_owned(),
                bytes: bytes.into(),
            },
        }
    }

    #[must_use]
    pub fn slice(bytes: impl Into<String>) -> Self {
        Self::Slice {
            slice: TvmSlice {
                type_field: TVM_SLICE_TYPE.to_owned(),
                bytes: bytes.into(),
            },
        }
    }

    #[must_use]
    pub fn tuple(elements: Vec<Self>) -> Self {
        Self::Tuple {
            tuple: TvmTuple {
                type_field: TVM_TUPLE_TYPE.to_owned(),
                elements,
            },
        }
    }

    #[must_use]
    pub fn list(elements: Vec<Self>) -> Self {
        Self::List {
            list: TvmList {
                type_field: TVM_LIST_TYPE.to_owned(),
                elements,
            },
        }
    }

    pub fn into_tuple_item(self) -> anyhow::Result<TupleItem> {
        match self {
            Self::Slice { slice } => {
                ensure_type(&slice.type_field, TVM_SLICE_TYPE)?;
                let cell =
                    Boc::decode_base64(&slice.bytes).context("Failed to decode slice BOC")?;
                Ok(TupleItem::Slice(cell))
            }
            Self::Cell { cell } => {
                ensure_type(&cell.type_field, TVM_CELL_TYPE)?;
                let cell = Boc::decode_base64(&cell.bytes).context("Failed to decode cell BOC")?;
                Ok(TupleItem::Cell(cell))
            }
            Self::Number { number } => {
                ensure_type(&number.type_field, TVM_NUMBER_DECIMAL_TYPE)?;
                Ok(TupleItem::Int(
                    number
                        .number
                        .parse::<BigInt>()
                        .context("Failed to parse stack number")?,
                ))
            }
            Self::Tuple { tuple } => {
                ensure_type(&tuple.type_field, TVM_TUPLE_TYPE)?;
                Ok(TupleItem::Tuple(Tuple(
                    tuple
                        .elements
                        .into_iter()
                        .map(Self::into_tuple_item)
                        .collect::<anyhow::Result<_>>()?,
                )))
            }
            Self::List { list } => {
                ensure_type(&list.type_field, TVM_LIST_TYPE)?;
                Ok(TupleItem::Tuple(Tuple(
                    list.elements
                        .into_iter()
                        .map(Self::into_tuple_item)
                        .collect::<anyhow::Result<_>>()?,
                )))
            }
            Self::Unsupported {} => anyhow::bail!("Unsupported TVM stack entry"),
        }
    }

    #[must_use]
    pub fn from_tuple_item(item: &TupleItem) -> Self {
        match item {
            TupleItem::Int(value) => Self::number(value),
            TupleItem::Cell(cell) => Self::cell(Boc::encode_base64(cell)),
            TupleItem::Slice(cell) => Self::slice(Boc::encode_base64(cell)),
            TupleItem::Cont(continuation) => Self::slice(Boc::encode_base64(&continuation.code)),
            TupleItem::Tuple(tuple) => {
                Self::tuple(tuple.0.iter().map(Self::from_tuple_item).collect())
            }
            TupleItem::Null | TupleItem::Nan | TupleItem::Builder(_) => Self::Unsupported {},
        }
    }
}

pub fn std_stack_into_tuple(entries: Vec<TvmStackEntry>) -> anyhow::Result<Tuple> {
    Ok(Tuple(
        entries
            .into_iter()
            .map(TvmStackEntry::into_tuple_item)
            .collect::<anyhow::Result<_>>()?,
    ))
}

#[must_use]
pub fn std_stack_from_tuple(stack: &Tuple) -> Vec<TvmStackEntry> {
    stack.0.iter().map(TvmStackEntry::from_tuple_item).collect()
}

fn ensure_type(actual: &str, expected: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        actual == expected,
        "Invalid `@type`: expected `{expected}`, got `{actual}`"
    );
    Ok(())
}

fn json_to_mixed_item(value: Value) -> anyhow::Result<TupleItem> {
    match json_to_legacy_item(value.clone()) {
        Ok(item) => Ok(item),
        Err(legacy_err) => {
            let entry: TvmStackEntry = serde_json::from_value(value)
                .with_context(|| format!("Failed to parse stack entry as legacy or std format. Legacy error: {legacy_err}"))?;
            entry.into_tuple_item()
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
    fn test_std_stack_uses_tonlib_wire_types() {
        let cell = CellBuilder::new().build().unwrap();
        let stack = Tuple(vec![
            TupleItem::Int(BigInt::from(-7)),
            TupleItem::Cell(cell.clone()),
            TupleItem::Slice(cell.clone()),
            TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(9))])),
            TupleItem::Null,
            TupleItem::Builder(cell.clone()),
        ]);

        assert_eq!(
            serde_json::to_value(std_stack_from_tuple(&stack)).unwrap(),
            serde_json::json!([
                {
                    "@type": "tvm.stackEntryNumber",
                    "number": {"@type": "tvm.numberDecimal", "number": "-7"}
                },
                {
                    "@type": "tvm.stackEntryCell",
                    "cell": {"@type": "tvm.cell", "bytes": Boc::encode_base64(&cell)}
                },
                {
                    "@type": "tvm.stackEntrySlice",
                    "slice": {"@type": "tvm.slice", "bytes": Boc::encode_base64(&cell)}
                },
                {
                    "@type": "tvm.stackEntryTuple",
                    "tuple": {
                        "@type": "tvm.tuple",
                        "elements": [{
                            "@type": "tvm.stackEntryNumber",
                            "number": {"@type": "tvm.numberDecimal", "number": "9"}
                        }]
                    }
                },
                {"@type": "tvm.stackEntryUnsupported"},
                {"@type": "tvm.stackEntryUnsupported"}
            ])
        );
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
        let entries: Vec<TvmStackEntry> = serde_json::from_value(serde_json::json!([{
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
        }]))
        .unwrap();

        assert_eq!(
            std_stack_into_tuple(entries).unwrap(),
            Tuple(vec![TupleItem::Tuple(Tuple(vec![TupleItem::Int(
                BigInt::from(7)
            )]))])
        );
    }

    #[test]
    fn test_std_stack_validates_nested_type_markers() {
        let entry: TvmStackEntry = serde_json::from_value(serde_json::json!({
            "@type": "tvm.stackEntryNumber",
            "number": {"@type": "wrong", "number": "1"}
        }))
        .unwrap();

        assert_eq!(
            entry.into_tuple_item().unwrap_err().to_string(),
            "Invalid `@type`: expected `tvm.numberDecimal`, got `wrong`"
        );
    }

    #[test]
    fn test_std_stack_rejects_unsupported_input() {
        assert_eq!(
            TvmStackEntry::Unsupported {}
                .into_tuple_item()
                .unwrap_err()
                .to_string(),
            "Unsupported TVM stack entry"
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
