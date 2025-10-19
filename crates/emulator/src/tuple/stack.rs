use abi::StructDescription;
use anyhow::anyhow;
use num_bigint::{BigInt, BigUint};
use std::fmt;
use std::ops::{Deref, DerefMut};
use tonlib_core::cell::{ArcCell, CellBuilder, CellParser};

#[derive(Default, Debug, Clone)]
pub struct Tuple(pub Vec<TupleItem>);

impl Tuple {
    pub fn empty() -> Tuple {
        Tuple(vec![])
    }
}

impl Deref for Tuple {
    type Target = Vec<TupleItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Tuple {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PartialEq for Tuple {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl fmt::Display for Tuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() == 1 {
            write!(f, "{}", self.0[0])
        } else {
            write!(f, "(")?;
            for (i, item) in self.0.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", item)?;
            }
            write!(f, ")")
        }
    }
}

impl Tuple {
    pub fn push_string(&mut self, s: &str) {
        let mut b = CellBuilder::new();
        b.store_bits(s.len() * 8, s.as_bytes()).unwrap();
        self.push(TupleItem::Slice(TupleSLice {
            cell: ArcCell::from(b.build().unwrap()),
            start_bits: 0,
            end_bits: (s.len() * 8) as u32,
            end_refs: 0,
            start_refs: 0,
        }));
    }

    pub fn push_bool(&mut self, v: bool) {
        self.push(TupleItem::Int(if v {
            BigInt::from(-1)
        } else {
            BigInt::from(0)
        }));
    }
}

#[derive(Debug, Clone, Eq)]
pub struct TupleSLice {
    pub cell: ArcCell,
    pub start_bits: u32,
    pub end_bits: u32,
    pub start_refs: u32,
    pub end_refs: u32,
}

/// Represents a stack value in TON VM
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TupleItem {
    Null,
    Int(BigInt),
    Nan,
    Cell(ArcCell),
    Slice(TupleSLice),
    Builder(ArcCell),
    Tuple(Vec<TupleItem>),
    TypedTuple {
        type_name: String,
        items: Vec<TupleItem>,
        abi: Option<StructDescription>,
    },
}

impl PartialEq for TupleSLice {
    fn eq(&self, other: &Self) -> bool {
        let self_bits_len = (self.end_bits - self.start_bits) as usize;
        let other_bits_len = (other.end_bits - other.start_bits) as usize;
        let self_refs_count = (self.end_refs - self.start_refs) as usize;
        let other_refs_count = (other.end_refs - other.start_refs) as usize;

        if self_bits_len != other_bits_len || self_refs_count != other_refs_count {
            // fast path
            return false;
        }

        let mut self_parser = self.cell.parser();
        let mut other_parser = other.cell.parser();

        if self_parser.skip_bits(self.start_bits as usize).is_err()
            || other_parser.skip_bits(other.start_bits as usize).is_err()
        {
            return false;
        }

        match (
            self_parser.load_bits(self_bits_len),
            other_parser.load_bits(other_bits_len),
        ) {
            (Ok(self_data), Ok(other_data)) => {
                if self_data != other_data {
                    return false;
                }
            }
            _ => return false,
        }

        let mut self_parser = self.cell.parser();
        let mut other_parser = other.cell.parser();

        for _ in 0..self_refs_count {
            match (self_parser.next_reference(), other_parser.next_reference()) {
                (Ok(self_ref), Ok(other_ref)) => {
                    if self_ref.cell_hash() != other_ref.cell_hash() {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        true
    }
}

impl Default for TupleItem {
    fn default() -> Self {
        TupleItem::Null
    }
}

fn format_item_with_type(item: &TupleItem, type_name: &str) -> String {
    match item {
        TupleItem::Int(value) if type_name == "bool" => {
            if *value == BigInt::from(0) {
                "false".to_string()
            } else if *value == BigInt::from(18446744073709551615u64) {
                "true".to_string()
            } else {
                format!("{}", value)
            }
        }
        _ => format!("{}", item),
    }
}

impl fmt::Display for TupleItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TupleItem::Int(value) => {
                if *value == BigInt::from(18446744073709551615u64) {
                    write!(f, "-1")
                } else {
                    write!(f, "{}", value)
                }
            }
            TupleItem::Null => write!(f, "null"),
            TupleItem::Nan => write!(f, "NaN"),
            TupleItem::Cell(cell) => write!(f, "{:?}", cell),
            TupleItem::Slice(TupleSLice {
                cell,
                start_bits,
                end_bits,
                ..
            }) => {
                let mut parser = cell.parser();
                if let Ok(()) = parser.skip_bits(*start_bits as usize) {
                    if let Ok(data) = parser.load_bits((*end_bits - *start_bits) as usize) {
                        if let Ok(utf8_string) = String::from_utf8(data) {
                            write!(f, "{}", utf8_string)
                        } else {
                            write!(f, "Slice(...)")
                        }
                    } else {
                        write!(f, "Slice(...)")
                    }
                } else {
                    write!(f, "Slice(...)")
                }
            }
            TupleItem::Builder(_) => write!(f, "Builder(...)"),
            TupleItem::Tuple(items) => {
                if items.len() == 1 {
                    write!(f, "{}", items[0])
                } else {
                    write!(f, "(")?;
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", item)?;
                    }
                    write!(f, ")")
                }
            }
            TupleItem::TypedTuple {
                type_name,
                items,
                abi,
            } => {
                if items.len() == 1 {
                    write!(f, "{}", items[0])
                } else {
                    if let Some(struct_desc) = abi {
                        if items.len() == struct_desc.fields.len() {
                            write!(f, "{} {{\n", type_name)?;
                            for (i, (field, item)) in
                                struct_desc.fields.iter().zip(items.iter()).enumerate()
                            {
                                write!(
                                    f,
                                    "    {}: {}",
                                    field.name,
                                    format_item_with_type(item, &field.type_name)
                                )?;
                                if i < struct_desc.fields.len() - 1 {
                                    write!(f, ",")?;
                                }
                                write!(f, "\n")?;
                            }
                            write!(f, "}}")?;
                            return Ok(());
                        }
                    }

                    write!(
                        f,
                        "{}({})",
                        type_name,
                        items
                            .iter()
                            .map(|item| format!("{}", item))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
        }
    }
}

/// Serialize a tuple item to a cell builder
pub fn serialize_tuple_item(
    builder: &mut CellBuilder,
    src: &TupleItem,
) -> Result<(), anyhow::Error> {
    match src {
        TupleItem::Null => {
            builder.store_u8(8, 0x00)?;
        }
        TupleItem::Int(value) => {
            // Check if value fits in int64
            if value <= &BigInt::from(9223372036854775807i64)
                && value >= &BigInt::from(-9223372036854775808i64)
            {
                builder.store_u8(8, 0x01)?;
                builder.store_int(64, value)?;
                return Ok(());
            }
            // Use int257 for larger values
            builder.store_u16(15, 0x0100)?;
            builder.store_int(257, &value.clone().into())?;
        }
        TupleItem::Nan => {
            builder.store_u16(16, 0x02ff)?;
        }
        TupleItem::Cell(cell) => {
            builder.store_u8(8, 0x03)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Slice(TupleSLice {
            cell,
            start_bits,
            end_bits,
            start_refs,
            end_refs,
        }) => {
            builder.store_u8(8, 0x04)?;
            builder.store_u32(10, *start_bits)?;
            builder.store_u32(10, *end_bits)?;
            builder.store_u32(3, *start_refs)?;
            builder.store_u32(3, *end_refs)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Builder(cell) => {
            builder.store_u8(8, 0x05)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Tuple(items) => {
            let mut head: Option<ArcCell> = None;
            let mut tail: Option<ArcCell> = None;

            for (i, item) in items.iter().enumerate() {
                let s = head;
                head = tail;
                tail = s;

                if i > 1 {
                    let mut bc = CellBuilder::new();
                    bc.store_reference(tail.as_ref().unwrap())?;
                    bc.store_reference(head.as_ref().unwrap())?;
                    head = Some(ArcCell::new(bc.build()?));
                }

                let mut bc = CellBuilder::new();
                serialize_tuple_item(&mut bc, item)?;
                tail = Some(ArcCell::new(bc.build()?));
            }

            builder.store_u8(8, 0x07)?;
            builder.store_u16(16, items.len() as u16)?;
            if let Some(h) = &head {
                builder.store_reference(h)?;
            }
            if let Some(t) = &tail {
                builder.store_reference(t)?;
            }
        }
        TupleItem::TypedTuple { items, .. } => {
            serialize_tuple_item(builder, &TupleItem::Tuple(items.clone()))?
        }
    }
    Ok(())
}

/// Parse a tuple item from a cell parser
pub fn parse_tuple_item(parser: &mut CellParser) -> Result<TupleItem, anyhow::Error> {
    let kind = parser.load_u8(8)?;

    match kind {
        0 => Ok(TupleItem::Null),
        1 => {
            let value = parser.load_i64(64)?;
            Ok(TupleItem::Int(BigInt::from(value as u64)))
        }
        2 => {
            if parser.load_u64(7)? == 0 {
                let value = parser.load_int(257)?;
                Ok(TupleItem::Int(value))
            } else {
                parser.load_bit()?;
                Ok(TupleItem::Nan)
            }
        }
        3 => {
            let cell = parser.next_reference()?;
            Ok(TupleItem::Cell(cell))
        }
        4 => {
            let start_bits = parser.load_u32(10)?;
            let end_bits = parser.load_u32(10)?;
            let start_refs = parser.load_u32(3)?;
            let end_refs = parser.load_u32(3)?;

            let cell_ref = parser.next_reference()?;

            Ok(TupleItem::Slice(TupleSLice {
                cell: cell_ref,
                start_bits,
                end_bits,
                start_refs,
                end_refs,
            }))
        }
        5 => {
            let cell = parser.next_reference()?;
            Ok(TupleItem::Builder(cell))
        }
        7 => {
            let length = parser.load_u16(16)? as usize;
            let mut items: Vec<TupleItem> = Vec::with_capacity(length);

            if length > 1 {
                let head_ref = parser.next_reference()?;
                let tail_ref = parser.next_reference()?;

                let mut tail_parser = tail_ref.parser();
                items.insert(0, parse_tuple_item(&mut tail_parser)?);

                let mut head_refs = vec![head_ref];
                let mut current_parser = head_refs[0].parser();

                for _ in 0..length - 2 {
                    let old_head = current_parser.next_reference()?;
                    let new_tail = current_parser.next_reference()?;

                    let mut new_tail_parser = new_tail.parser();
                    items.insert(0, parse_tuple_item(&mut new_tail_parser)?);

                    head_refs.push(old_head);
                    current_parser = head_refs[head_refs.len() - 1].parser();
                }

                items.insert(0, parse_tuple_item(&mut current_parser)?);
            } else if length == 1 {
                let ref_cell = parser.next_reference()?;
                let mut item_parser = ref_cell.parser();
                items.push(parse_tuple_item(&mut item_parser)?);
            }

            Ok(TupleItem::Tuple(items))
        }
        _ => Err(anyhow!("Unsupported stack item kind: {}", kind).into()),
    }
}

/// Serialize a tuple (stack) to a cell
pub fn serialize_tuple(src: &[TupleItem]) -> Result<ArcCell, anyhow::Error> {
    let mut builder = CellBuilder::new();
    builder.store_uint(24, &BigUint::from(src.len()))?;
    serialize_tuple_tail(src, &mut builder)?;
    Ok(ArcCell::new(builder.build()?))
}

fn serialize_tuple_tail(src: &[TupleItem], builder: &mut CellBuilder) -> Result<(), anyhow::Error> {
    if !src.is_empty() {
        // rest:^(VmStackList n)
        let mut tail_builder = CellBuilder::new();
        serialize_tuple_tail(&src[..src.len() - 1], &mut tail_builder)?;
        let tail_cell = ArcCell::new(tail_builder.build()?);
        builder.store_reference(&tail_cell)?;

        // tos
        serialize_tuple_item(builder, &src[src.len() - 1])?;
    }
    Ok(())
}

/// Parse a tuple (stack) from a cell
pub fn parse_tuple(src: &ArcCell) -> Result<Vec<TupleItem>, anyhow::Error> {
    let mut cur_cell = ArcCell::clone(src);
    let mut cs = cur_cell.parser();

    let size = cs.load_u32(24)? as usize;
    let mut result: Vec<TupleItem> = Vec::with_capacity(size);

    for _ in 0..size {
        let next_ref = cs.next_reference()?;
        let item = parse_tuple_item(&mut cs)?;
        result.insert(0, item);

        cur_cell = ArcCell::clone(&next_ref);
        cs = cur_cell.parser();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_test(item: TupleItem) {
        let mut builder = CellBuilder::new();
        serialize_tuple_item(&mut builder, &item).unwrap();
        let cell = ArcCell::new(builder.build().unwrap());

        let mut parser = cell.parser();
        let deserialized = parse_tuple_item(&mut parser).unwrap();

        assert_eq!(item, deserialized);
    }

    fn roundtrip_tuple_test(items: Vec<TupleItem>) {
        let serialized = serialize_tuple(&items).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();

        assert_eq!(items, deserialized);
    }

    #[test]
    fn test_null_roundtrip() {
        roundtrip_test(TupleItem::Null);
    }

    #[test]
    fn test_small_int_roundtrip() {
        roundtrip_test(TupleItem::Int(BigInt::from(42u64)));
        // roundtrip_test(TupleItem::Int(BigInt::from(i64::MIN)));
        // roundtrip_test(TupleItem::Int(BigInt::from(i64::MAX)));
    }

    #[test]
    fn test_large_int_roundtrip() {
        // Large integer that doesn't fit in i64
        let large_int = BigInt::from(1u128 << 100);
        roundtrip_test(TupleItem::Int(large_int));
    }

    #[test]
    fn test_nan_roundtrip() {
        roundtrip_test(TupleItem::Nan);
    }

    #[test]
    fn test_cell_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_u8(8, 42).unwrap();
        let test_cell = ArcCell::new(builder.build().unwrap());

        roundtrip_test(TupleItem::Cell(test_cell));
    }

    #[test]
    fn test_slice_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_u8(8, 42).unwrap();
        builder.store_u8(8, 43).unwrap();
        let test_cell = ArcCell::new(builder.build().unwrap());

        roundtrip_test(TupleItem::Slice(TupleSLice {
            cell: test_cell,
            start_bits: 0,
            end_bits: 16,
            start_refs: 0,
            end_refs: 0,
        }));
    }

    #[test]
    fn test_builder_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_u8(8, 42).unwrap();
        let test_cell = ArcCell::new(builder.build().unwrap());

        roundtrip_test(TupleItem::Builder(test_cell));
    }

    #[test]
    fn test_empty_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(vec![]));
    }

    #[test]
    fn test_single_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(vec![TupleItem::Null]));
        roundtrip_test(TupleItem::Tuple(vec![TupleItem::Int(BigInt::from(123u64))]));
    }

    #[test]
    fn test_multi_item_tuple_roundtrip() {
        let items = vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(42u64)),
            TupleItem::Nan,
        ];
        roundtrip_test(TupleItem::Tuple(items));
    }

    #[test]
    fn test_nested_tuple_roundtrip() {
        let inner_tuple =
            TupleItem::Tuple(vec![TupleItem::Null, TupleItem::Int(BigInt::from(1u64))]);
        let outer_tuple = TupleItem::Tuple(vec![inner_tuple, TupleItem::Int(BigInt::from(2u64))]);
        roundtrip_test(outer_tuple);
    }

    #[test]
    fn test_tuple_stack_roundtrip() {
        roundtrip_tuple_test(vec![]);
        roundtrip_tuple_test(vec![TupleItem::Null]);
        let items = vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(42u64)),
            TupleItem::Nan,
        ];
        roundtrip_tuple_test(items);
    }
}
