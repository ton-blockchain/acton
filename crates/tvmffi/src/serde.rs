use crate::stack::{Tuple, TupleItem};
use anyhow::anyhow;
use num_bigint::{BigInt, BigUint};
use tonlib_core::cell::{ArcCell, CellBuilder, CellParser};

impl Tuple {
    /// Serialize a tuple to a cell.
    pub fn serialize(&self) -> Result<ArcCell, anyhow::Error> {
        serialize_tuple(self)
    }

    /// Deserialize a tuple from a cell.
    pub fn deserialize(src: &ArcCell) -> Result<Tuple, anyhow::Error> {
        parse_tuple(src)
    }
}

/// Serialize a tuple item to a cell builder
pub fn serialize_tuple_item(builder: &mut CellBuilder, src: &TupleItem) -> anyhow::Result<()> {
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
            builder.store_int(257, &value.clone())?;
        }
        TupleItem::Nan => {
            builder.store_u16(16, 0x02ff)?;
        }
        TupleItem::Cell(cell) => {
            builder.store_u8(8, 0x03)?;
            builder.store_reference(cell)?;
        }
        TupleItem::Slice(cell) => {
            builder.store_u8(8, 0x04)?;
            builder.store_u32(10, 0)?;
            builder.store_u32(10, cell.bit_len() as u32)?;
            builder.store_u32(3, 0)?;
            builder.store_u32(3, cell.references().len() as u32)?;
            builder.store_reference(cell)?;
        }
        TupleItem::Builder(cell) => {
            builder.store_u8(8, 0x05)?;
            builder.store_reference(cell)?;
        }
        TupleItem::Tuple(items) => {
            let mut head: Option<ArcCell> = None;
            let mut tail: Option<ArcCell> = None;

            for (i, item) in items.iter().enumerate() {
                std::mem::swap(&mut head, &mut tail);

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
        TupleItem::TypedTuple { inner: items, .. } => {
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
            Ok(TupleItem::Int(BigInt::from(value)))
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

            let mut parser = cell_ref.parser();
            parser.skip_bits(start_bits as usize)?;
            let root_data_size = (end_bits - start_bits) as usize;
            let root_bits = parser.load_bits(root_data_size)?;

            // skip first refs
            for _ in 0..start_refs {
                parser.next_reference()?;
            }

            let mut builder = CellBuilder::new();
            builder.store_bits(root_data_size, &root_bits)?;

            for _ in start_refs..end_refs {
                let next_ref = parser.next_reference()?;
                builder.store_reference(&next_ref)?;
            }

            let final_cell = builder.build()?;

            Ok(TupleItem::Slice(final_cell.into()))
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

            Ok(TupleItem::Tuple(Tuple(items)))
        }
        6 => {
            // TODO: support continuation
            Ok(TupleItem::Null)
        }
        _ => Err(anyhow!("Unsupported stack item kind: {}", kind)),
    }
}

/// Serialize a tuple (stack) to a cell
pub fn serialize_tuple(src: &Tuple) -> Result<ArcCell, anyhow::Error> {
    let mut builder = CellBuilder::new();
    builder.store_uint(24, &BigUint::from(src.0.len()))?;
    serialize_tuple_tail(&src.0, &mut builder)?;
    Ok(ArcCell::new(builder.build()?))
}

fn serialize_tuple_tail(src: &[TupleItem], builder: &mut CellBuilder) -> anyhow::Result<()> {
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
pub fn parse_tuple(src: &ArcCell) -> Result<Tuple, anyhow::Error> {
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

    Ok(Tuple(result))
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
        let serialized = serialize_tuple(&Tuple(items.clone())).unwrap();
        let deserialized = parse_tuple(&serialized).unwrap();

        assert_eq!(Tuple(items), deserialized);
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

        roundtrip_test(TupleItem::Slice(test_cell));
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
        roundtrip_test(TupleItem::Tuple(Tuple::empty()));
    }

    #[test]
    fn test_single_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![TupleItem::Null])));
        roundtrip_test(TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(
            123u64,
        ))])));
    }

    #[test]
    fn test_multi_item_tuple_roundtrip() {
        let items = vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(42u64)),
            TupleItem::Nan,
        ];
        roundtrip_test(TupleItem::Tuple(Tuple(items)));
    }

    #[test]
    fn test_nested_tuple_roundtrip() {
        let inner_tuple = TupleItem::Tuple(Tuple(vec![
            TupleItem::Null,
            TupleItem::Int(BigInt::from(1u64)),
        ]));
        let outer_tuple =
            TupleItem::Tuple(Tuple(vec![inner_tuple, TupleItem::Int(BigInt::from(2u64))]));
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
