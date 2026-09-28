//! Bounded conversion of TON Center's legacy stack and `TONLib` list semantics.
//! Simulator JSON uses a different vocabulary and must not define this wire API.

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use num_bigint::{BigInt, Sign};
use rston::boc::Boc;
use rston::cell::{CellSlice, DynCell};
use toncenter::v2::Int64Input;
use toncenter::v2::stack::{self as wire, LegacyStackEntry, TvmStackEntry};
use tvm_ffi::stack::{ContData, Tuple, TupleItem};

const MAX_VALUES: usize = 1000;
const MAX_DEPTH: usize = 64;
const MAX_BOC_BYTES: usize = 1024 * 1024;

struct Budget {
    values: usize,
    bytes: usize,
}

impl Budget {
    const fn new() -> Self {
        Self {
            values: MAX_VALUES,
            bytes: MAX_BOC_BYTES,
        }
    }

    fn value(&mut self, depth: usize) -> Result<()> {
        ensure!(depth <= MAX_DEPTH, "TVM stack nesting limit exceeded");
        self.values = self
            .values
            .checked_sub(1)
            .context("TVM stack size limit exceeded")?;
        Ok(())
    }

    fn bytes(&mut self, count: usize) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_sub(count)
            .context("TVM stack byte limit exceeded")?;
        Ok(())
    }
}

pub(super) fn number(value: &str) -> Result<BigInt> {
    ensure!(value.len() <= 80, "TVM integer is too long");
    let (negative, magnitude) = value
        .strip_prefix('-')
        .map_or((false, value), |v| (true, v));
    ensure!(!magnitude.starts_with(['-', '+']), "invalid integer sign");
    let mut number = if let Some(hex) = magnitude
        .strip_prefix("0x")
        .or_else(|| magnitude.strip_prefix("0X"))
    {
        BigInt::parse_bytes(hex.as_bytes(), 16).context("invalid hexadecimal integer")?
    } else {
        magnitude
            .parse::<BigInt>()
            .context("invalid decimal integer")?
    };
    if negative {
        number = -number;
    }
    let minimum = -(BigInt::from(1) << 256_usize);
    ensure!(
        number >= minimum && number < -minimum,
        "integer does not fit TVM int257"
    );
    Ok(number)
}

pub(super) fn input(entries: Vec<LegacyStackEntry>) -> Result<Tuple> {
    // The shared serializer's stack spine is recursive; bound it independently
    // of the total number of entries inside nested tuples and lists.
    ensure!(entries.len() <= 256, "too many input stack entries");
    let mut budget = Budget::new();
    entries
        .into_iter()
        .map(|entry| {
            budget.value(0)?;
            Ok(match entry {
                LegacyStackEntry::Number((_, value)) => TupleItem::Int(match value {
                    Int64Input::Number(value) => value.into(),
                    Int64Input::String(value) => number(&value)?,
                }),
                LegacyStackEntry::Cell((_, value)) => cell(&value.bytes, false, &mut budget)?,
                LegacyStackEntry::Slice((_, value)) => cell(&value.bytes, true, &mut budget)?,
                LegacyStackEntry::CellBytes((_, value)) => cell(&value, false, &mut budget)?,
                LegacyStackEntry::SliceBytes((_, value)) => cell(&value, true, &mut budget)?,
                LegacyStackEntry::Tuple((_, value)) => {
                    compound(value.elements, false, 1, &mut budget)?
                }
                LegacyStackEntry::List((_, value)) => {
                    compound(value.elements, true, 1, &mut budget)?
                }
                LegacyStackEntry::Unsupported(_) => bail!("unsupported input stack entry"),
            })
        })
        .collect::<Result<Vec<_>>>()
        .map(Tuple)
}

fn cell(bytes: &str, slice: bool, budget: &mut Budget) -> Result<TupleItem> {
    budget.bytes(bytes.len())?;
    let cell = Boc::decode_base64(bytes).context("invalid stack BoC")?;
    if slice {
        ensure!(
            !cell.is_exotic(),
            "exotic cell cannot be used as an input slice"
        );
        Ok(TupleItem::Slice(cell))
    } else {
        Ok(TupleItem::Cell(cell))
    }
}

fn standard(entry: TvmStackEntry, depth: usize, budget: &mut Budget) -> Result<TupleItem> {
    budget.value(depth)?;
    Ok(match entry {
        TvmStackEntry::Number(entry) => {
            // Nested TONLib numbers use decimal, unlike legacy top-level values.
            let value = &entry.number.number;
            ensure!(!value.contains(['x', 'X']), "TONLib number must be decimal");
            TupleItem::Int(number(value)?)
        }
        TvmStackEntry::Cell(entry) => cell(&entry.cell.bytes, false, budget)?,
        TvmStackEntry::Slice(entry) => cell(&entry.slice.bytes, true, budget)?,
        TvmStackEntry::Tuple(entry) => compound(entry.tuple.elements, false, depth + 1, budget)?,
        TvmStackEntry::List(entry) => compound(entry.list.elements, true, depth + 1, budget)?,
        TvmStackEntry::Unsupported(_) => bail!("unsupported input stack entry"),
    })
}

fn compound(
    entries: Vec<TvmStackEntry>,
    list: bool,
    depth: usize,
    budget: &mut Budget,
) -> Result<TupleItem> {
    ensure!(
        entries.len() <= budget.values,
        "too many nested stack values"
    );
    if list {
        // Lists become nested pairs, so their physical nesting also needs a bound.
        ensure!(depth + entries.len() <= MAX_DEPTH, "TVM list is too deep");
        let mut tail = TupleItem::Null;
        for (index, entry) in entries.into_iter().enumerate().rev() {
            tail = TupleItem::Tuple(Tuple(vec![standard(entry, depth + index, budget)?, tail]));
        }
        Ok(tail)
    } else {
        entries
            .into_iter()
            .map(|entry| standard(entry, depth, budget))
            .collect::<Result<Vec<_>>>()
            .map(|entries| TupleItem::Tuple(Tuple(entries)))
    }
}

/// Decodes only the public `TONLib` value vocabulary. Continuations are opaque
/// unsupported values, so their captured stacks are neither traversed nor run.
/// The shared scalar/slice decoder preserves slice bit and reference offsets.
pub(super) fn decode(bytes: &str) -> Result<Tuple> {
    ensure!(bytes.len() <= MAX_BOC_BYTES, "VM result stack is too large");
    let mut root = Boc::decode_base64(bytes)?;
    let size = root.as_slice()?.load_uint(24)? as usize;
    ensure!(size <= MAX_VALUES, "VM result has too many stack entries");
    let mut values = Vec::with_capacity(size);
    let mut budget = Budget::new();
    for index in 0..size {
        let mut slice = root.as_slice()?;
        if index == 0 {
            slice.load_uint(24)?;
        }
        let rest = slice.load_reference_cloned()?;
        values.push(decode_value(slice, 0, &mut budget)?);
        root = rest;
    }
    values.reverse();
    Ok(Tuple(values))
}

fn decode_value(mut slice: CellSlice<'_>, depth: usize, budget: &mut Budget) -> Result<TupleItem> {
    budget.value(depth)?;
    let mut tag = slice;
    match tag.load_u8()? {
        6 => Ok(TupleItem::Cont(ContData::default())),
        7 => {
            let size = tag.load_u16()? as usize;
            ensure!(size <= budget.values, "VM tuple is too large");
            let mut values = Vec::with_capacity(size);
            if size > 1 {
                let mut head = tag.load_reference_cloned()?;
                let tail = tag.load_reference_cloned()?;
                values.push(decode_value(tail.as_slice()?, depth + 1, budget)?);
                for _ in 0..size - 2 {
                    let mut slice = head.as_slice()?;
                    let next = slice.load_reference_cloned()?;
                    let tail = slice.load_reference_cloned()?;
                    values.push(decode_value(tail.as_slice()?, depth + 1, budget)?);
                    head = next;
                }
                values.push(decode_value(head.as_slice()?, depth + 1, budget)?);
            } else if size == 1 {
                let value = tag.load_reference_cloned()?;
                values.push(decode_value(value.as_slice()?, depth + 1, budget)?);
            }
            values.reverse();
            Ok(TupleItem::Tuple(Tuple(values)))
        }
        _ => tvm_ffi::serde::parse_tuple_item(&mut slice),
    }
}

pub(super) fn output(stack: &Tuple) -> Result<Vec<LegacyStackEntry>> {
    let mut budget = Budget::new();
    stack
        .iter()
        .map(|item| {
            let value = to_standard(item, 0, &mut budget)?;
            Ok(match value {
                TvmStackEntry::Number(value) => {
                    let number = &value.number.number;
                    let number: BigInt = number
                        .parse()
                        .context("legacy TON Center numbers cannot represent NaN")?;
                    let number = format!(
                        "{}0x{}",
                        if number.sign() == Sign::Minus {
                            "-"
                        } else {
                            ""
                        },
                        number.magnitude().to_str_radix(16)
                    );
                    LegacyStackEntry::Number((
                        wire::LegacyNumberTag::Num,
                        Int64Input::String(number),
                    ))
                }
                TvmStackEntry::Cell(value) => LegacyStackEntry::Cell((
                    wire::LegacyCellTag::Cell,
                    expanded_cell(value.cell.bytes, &mut budget)?,
                )),
                TvmStackEntry::Slice(value) => LegacyStackEntry::Cell((
                    wire::LegacyCellTag::Cell,
                    expanded_cell(value.slice.bytes, &mut budget)?,
                )),
                TvmStackEntry::Tuple(value) => {
                    LegacyStackEntry::Tuple((wire::LegacyTupleTag::Tuple, value.tuple))
                }
                TvmStackEntry::List(value) => {
                    LegacyStackEntry::List((wire::LegacyListTag::List, value.list))
                }
                TvmStackEntry::Unsupported(_) => LegacyStackEntry::Unsupported((
                    wire::LegacyUnsupportedTag::Unsupported,
                    String::new(),
                )),
            })
        })
        .collect()
}

fn expanded_cell(bytes: String, budget: &mut Budget) -> Result<wire::LegacyStackEntryCell> {
    fn expand(cell: &DynCell, depth: usize, budget: &mut Budget) -> Result<wire::LegacyTvmCell> {
        budget.value(depth)?;
        let b64 = STANDARD.encode(cell.data());
        budget.bytes(b64.len())?;
        Ok(wire::LegacyTvmCell {
            data: wire::LegacyTvmCellData {
                b64,
                len: i32::from(cell.bit_len()),
            },
            refs: cell
                .references()
                .map(|child| expand(child, depth + 1, budget))
                .collect::<Result<_>>()?,
            special: cell.is_exotic(),
        })
    }
    let cell = Boc::decode_base64(&bytes)?;
    Ok(wire::LegacyStackEntryCell {
        bytes,
        object: Some(expand(cell.as_ref(), 0, budget)?),
    })
}

fn to_standard(item: &TupleItem, depth: usize, budget: &mut Budget) -> Result<TvmStackEntry> {
    budget.value(depth)?;
    Ok(match item {
        TupleItem::Int(value) => TvmStackEntry::number(value),
        TupleItem::Nan => TvmStackEntry::number("NaN"),
        TupleItem::Cell(value) | TupleItem::Slice(value) => {
            let bytes = Boc::encode_base64(value);
            budget.bytes(bytes.len())?;
            if matches!(item, TupleItem::Slice(_)) {
                TvmStackEntry::slice(bytes)
            } else {
                TvmStackEntry::cell(bytes)
            }
        }
        TupleItem::Null => TvmStackEntry::list(Vec::new()),
        TupleItem::Tuple(values) => {
            if let Some(list) = list_items(item) {
                TvmStackEntry::list(
                    list.into_iter()
                        .map(|item| to_standard(item, depth + 1, budget))
                        .collect::<Result<_>>()?,
                )
            } else {
                TvmStackEntry::tuple(
                    values
                        .iter()
                        .map(|item| to_standard(item, depth + 1, budget))
                        .collect::<Result<_>>()?,
                )
            }
        }
        TupleItem::Builder(_) | TupleItem::Cont(_) => {
            TvmStackEntry::Unsupported(wire::TvmStackEntryUnsupported {
                type_tag: Default::default(),
            })
        }
    })
}

fn list_items(mut item: &TupleItem) -> Option<Vec<&TupleItem>> {
    let mut items = Vec::new();
    loop {
        match item {
            TupleItem::Null => return Some(items),
            TupleItem::Tuple(pair) if pair.len() == 2 => {
                items.push(&pair[0]);
                item = &pair[1];
            }
            _ => return None,
        }
    }
}
