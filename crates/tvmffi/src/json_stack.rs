use crate::stack::{Tuple, TupleItem};
use anyhow::Context;
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;

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
            cell: c.to_boc_b64(false)?,
        }),
        TupleItem::Slice(c) => Ok(JsonStackEntry::Slice {
            slice: c.to_boc_b64(false)?,
        }),
        TupleItem::Builder(c) => Ok(JsonStackEntry::Builder {
            builder: c.to_boc_b64(false)?,
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

pub fn json_to_stack(entries: Vec<Value>) -> anyhow::Result<Tuple> {
    let mut items = Vec::new();
    for entry in entries {
        let entry: JsonStackEntry = serde_json::from_value(entry)?;
        items.push(json_to_item(entry)?);
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
            let c = ArcCell::from_boc_b64(&cell).context("Failed to decode cell BOC")?;
            Ok(TupleItem::Cell(c))
        }
        JsonStackEntry::Slice { slice } => {
            let c = ArcCell::from_boc_b64(&slice).context("Failed to decode slice BOC")?;
            Ok(TupleItem::Slice(c))
        }
        JsonStackEntry::Builder { builder } => {
            let c = ArcCell::from_boc_b64(&builder).context("Failed to decode builder BOC")?;
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
