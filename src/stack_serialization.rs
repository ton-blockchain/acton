use num_bigint::{BigInt, BigUint};
use std::fmt;
use tonlib_core::cell::{ArcCell, CellBuilder, CellParser};

/// Helper function to load a small uint as u64
fn load_uint_as_u64(
    parser: &mut CellParser,
    bits: usize,
) -> Result<u64, Box<dyn std::error::Error>> {
    let big_uint = parser.load_uint(bits)?;
    let nums = big_uint.to_u64_digits();
    if nums.len() == 0 {
        return Ok(0);
    }
    Ok(nums[0])
}

/// Helper function to load a small uint as u32
fn load_uint_as_u32(
    parser: &mut CellParser,
    bits: usize,
) -> Result<u32, Box<dyn std::error::Error>> {
    let big_uint = parser.load_uint(bits)?;
    let nums = big_uint.to_u64_digits();
    if nums.len() == 0 {
        return Ok(0);
    }
    Ok(nums[0] as u32)
}

/// Represents a stack value in TON VM
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TupleItem {
    Null,
    Int(BigInt),
    Nan,
    Cell(ArcCell),
    Slice {
        cell: ArcCell,
        start_bits: u32,
        end_bits: u32,
        start_refs: u32,
        end_refs: u32,
    },
    Builder(ArcCell),
    Tuple(Vec<TupleItem>),
    TypedTuple {
        type_name: String,
        items: Vec<TupleItem>,
    },
}

impl Default for TupleItem {
    fn default() -> Self {
        TupleItem::Null
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
            TupleItem::Slice { .. } => write!(f, "Slice(...)"),
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
            TupleItem::TypedTuple { type_name, items } => {
                if items.len() == 1 {
                    write!(f, "{}", items[0])
                } else {
                    write!(f, "{} (", type_name)?;
                    for (i, item) in items.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", item)?;
                    }
                    write!(f, ")")
                }
            }
        }
    }
}

/// Serialize a tuple item to a cell builder
pub fn serialize_tuple_item(
    src: &TupleItem,
    builder: &mut CellBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
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
            builder.store_uint(15, &BigUint::from(0x0100u16))?;
            builder.store_int(257, &value.clone().into())?;
        }
        TupleItem::Nan => {
            builder.store_u16(16, 0x02ff)?;
        }
        TupleItem::Cell(cell) => {
            builder.store_u8(8, 0x03)?;
            builder.store_reference(&cell)?;
        }
        TupleItem::Slice {
            cell,
            start_bits,
            end_bits,
            start_refs,
            end_refs,
        } => {
            builder.store_u8(8, 0x04)?;
            builder.store_uint(10, &BigUint::from(*start_bits))?;
            builder.store_uint(10, &BigUint::from(*end_bits))?;
            builder.store_uint(3, &BigUint::from(*start_refs))?;
            builder.store_uint(3, &BigUint::from(*end_refs))?;
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
                // Swap
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
                serialize_tuple_item(item, &mut bc)?;
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
            serialize_tuple_item(&TupleItem::Tuple(items.clone()), builder)?
        }
    }
    Ok(())
}

/// Parse a tuple item from a cell parser
pub fn parse_tuple_item(parser: &mut CellParser) -> Result<TupleItem, Box<dyn std::error::Error>> {
    let kind = parser.load_u8(8)?;

    match kind {
        0 => Ok(TupleItem::Null),
        1 => {
            let value = parser.load_i64(64)?;
            Ok(TupleItem::Int(BigInt::from(value as u64)))
        }
        2 => {
            if load_uint_as_u64(parser, 7)? == 0 {
                let value = parser.load_int(257)?;
                Ok(TupleItem::Int(value))
            } else {
                // Skip the bit that should be 1 for nan
                parser.load_bit()?;
                Ok(TupleItem::Nan)
            }
        }
        3 => {
            let cell = parser.next_reference()?;
            Ok(TupleItem::Cell(cell))
        }
        4 => {
            let start_bits = load_uint_as_u32(parser, 10)?;
            let end_bits = load_uint_as_u32(parser, 10)?;
            let start_refs = load_uint_as_u32(parser, 3)?;
            let end_refs = load_uint_as_u32(parser, 3)?;

            let cell_ref = parser.next_reference()?;

            Ok(TupleItem::Slice {
                cell: cell_ref,
                start_bits,
                end_bits,
                start_refs,
                end_refs,
            })
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

                let mut tail_parser = CellParser::new(&tail_ref);
                items.insert(0, parse_tuple_item(&mut tail_parser)?);

                // Store references to avoid lifetime issues
                let mut head_refs = vec![head_ref];
                let mut current_parser = CellParser::new(&head_refs[0]);

                for _ in 0..length - 2 {
                    let old_head = current_parser.next_reference()?;
                    let new_tail = current_parser.next_reference()?;

                    let mut new_tail_parser = CellParser::new(&new_tail);
                    items.insert(0, parse_tuple_item(&mut new_tail_parser)?);

                    head_refs.push(old_head);
                    current_parser = CellParser::new(&head_refs[head_refs.len() - 1]);
                }

                items.insert(0, parse_tuple_item(&mut current_parser)?);
            } else if length == 1 {
                let ref_cell = parser.next_reference()?;
                let mut item_parser = CellParser::new(&ref_cell);
                items.push(parse_tuple_item(&mut item_parser)?);
            }

            Ok(TupleItem::Tuple(items))
        }
        _ => Err(format!("Unsupported stack item kind: {}", kind).into()),
    }
}

/// Serialize a tuple (stack) to a cell
pub fn serialize_tuple(src: &[TupleItem]) -> Result<ArcCell, Box<dyn std::error::Error>> {
    let mut builder = CellBuilder::new();
    builder.store_uint(24, &BigUint::from(src.len()))?;
    serialize_tuple_tail(src, &mut builder)?;
    Ok(ArcCell::new(builder.build()?))
}

fn serialize_tuple_tail(
    src: &[TupleItem],
    builder: &mut CellBuilder,
) -> Result<(), Box<dyn std::error::Error>> {
    if !src.is_empty() {
        // rest:^(VmStackList n)
        let mut tail_builder = CellBuilder::new();
        serialize_tuple_tail(&src[..src.len() - 1], &mut tail_builder)?;
        let tail_cell = ArcCell::new(tail_builder.build()?);
        builder.store_reference(&tail_cell)?;

        // tos
        serialize_tuple_item(&src[src.len() - 1], builder)?;
    }
    Ok(())
}

/// Parse a tuple (stack) from a cell
pub fn parse_tuple(src: &ArcCell) -> Result<Vec<TupleItem>, Box<dyn std::error::Error>> {
    let mut cur_cell = ArcCell::clone(src);
    let mut cs = CellParser::new(&cur_cell);

    let size = load_uint_as_u64(&mut cs, 24)? as usize;
    let mut result: Vec<TupleItem> = Vec::with_capacity(size);

    for _ in 0..size {
        // next_reference берём, пока действителен текущий парсер
        let next_ref = cs.next_reference()?;
        let item = parse_tuple_item(&mut cs)?;
        result.insert(0, item);

        // Обновляем «владельца» и создаём новый парсер, заимствующий из него
        cur_cell = ArcCell::clone(&next_ref);
        cs = CellParser::new(&cur_cell);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip_test(item: TupleItem) {
        let mut builder = CellBuilder::new();
        serialize_tuple_item(&item, &mut builder).unwrap();
        let cell = ArcCell::new(builder.build().unwrap());

        let mut parser = CellParser::new(&cell);
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

        roundtrip_test(TupleItem::Slice {
            cell: test_cell,
            start_bits: 0,
            end_bits: 16,
            start_refs: 0,
            end_refs: 0,
        });
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
        // Test empty stack
        roundtrip_tuple_test(vec![]);

        // Test single item stack
        roundtrip_tuple_test(vec![TupleItem::Null]);

        // Test multi-item stack
        let items = vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(42u64)),
            TupleItem::Nan,
        ];
        roundtrip_tuple_test(items);
    }
}
