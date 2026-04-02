use crate::stack::{Tuple, TupleItem};
use anyhow::anyhow;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use tycho_types::cell::{Cell, CellBuilder, CellSlice};

impl Tuple {
    /// Serialize a tuple to a cell.
    pub fn serialize(&self) -> Result<Cell, anyhow::Error> {
        serialize_tuple(self)
    }

    /// Deserialize a tuple from a cell.
    pub fn deserialize(src: &Cell) -> Result<Tuple, anyhow::Error> {
        parse_tuple(src)
    }
}

/// Serialize a tuple item to a cell builder
pub fn serialize_tuple_item(builder: &mut CellBuilder, src: &TupleItem) -> anyhow::Result<()> {
    match src {
        TupleItem::Null => {
            builder.store_small_uint(0x00, 8)?;
        }
        TupleItem::Int(value) => {
            // Check if value fits in int64
            if value <= &BigInt::from(9223372036854775807i64)
                && value >= &BigInt::from(-9223372036854775808i64)
            {
                builder.store_small_uint(0x01, 8)?;
                let int64 = value
                    .to_i64()
                    .ok_or_else(|| anyhow!("invalid i64 value in tuple serialization"))?;
                builder.store_u64(int64 as u64)?;
                return Ok(());
            }

            // Use int257 for larger values
            builder.store_small_uint(0x02, 8)?;
            builder.store_uint(0, 7)?;
            builder.store_bigint(value, 257, true)?;
        }
        TupleItem::Nan => {
            builder.store_small_uint(0x02, 8)?;
            builder.store_small_uint(0xff, 8)?;
        }
        TupleItem::Cell(cell) => {
            builder.store_small_uint(0x03, 8)?;
            builder.store_reference(cell.clone())?;
        }
        TupleItem::Slice(cell) => {
            builder.store_small_uint(0x04, 8)?;
            builder.store_uint(0, 10)?;
            builder.store_uint(u64::from(cell.bit_len()), 10)?;
            builder.store_uint(0, 3)?;
            builder.store_uint(u64::from(cell.reference_count()), 3)?;
            builder.store_reference(cell.clone())?;
        }
        TupleItem::Builder(cell) => {
            builder.store_small_uint(0x05, 8)?;
            builder.store_reference(cell.clone())?;
        }
        TupleItem::Tuple(items) => {
            let mut head: Option<Cell> = None;
            let mut tail: Option<Cell> = None;

            for (i, item) in items.iter().enumerate() {
                std::mem::swap(&mut head, &mut tail);

                if i > 1 {
                    let mut bc = CellBuilder::new();
                    if let Some(tail) = tail.as_ref() {
                        bc.store_reference(tail.clone())?;
                    }
                    if let Some(head) = head.as_ref() {
                        bc.store_reference(head.clone())?;
                    }
                    head = Some(bc.build()?);
                }

                let mut bc = CellBuilder::new();
                serialize_tuple_item(&mut bc, item)?;
                tail = Some(bc.build()?);
            }

            builder.store_small_uint(0x07, 8)?;
            builder.store_u16(items.len() as u16)?;
            if let Some(h) = &head {
                builder.store_reference(h.clone())?;
            }
            if let Some(t) = &tail {
                builder.store_reference(t.clone())?;
            }
        }
        TupleItem::TypedTuple { inner: items, .. } => {
            serialize_tuple_item(builder, &TupleItem::Tuple(items.clone()))?;
        }
    }
    Ok(())
}

/// Parse a tuple item from a cell parser
pub fn parse_tuple_item(parser: &mut CellSlice<'_>) -> Result<TupleItem, anyhow::Error> {
    let kind = parser.load_small_uint(8)?;

    match kind {
        0 => Ok(TupleItem::Null),
        1 => {
            let value = parser.load_u64()? as i64;
            Ok(TupleItem::Int(BigInt::from(value)))
        }
        2 => {
            if parser.load_uint(7)? == 0 {
                let value = parser.load_bigint(257, true)?;
                Ok(TupleItem::Int(value))
            } else {
                parser.load_bit()?;
                Ok(TupleItem::Nan)
            }
        }
        3 => {
            let cell = parser.load_reference_cloned()?;
            Ok(TupleItem::Cell(cell))
        }
        4 => {
            let start_bits = parser.load_uint(10)? as u16;
            let end_bits = parser.load_uint(10)? as u16;
            let start_refs = parser.load_uint(3)? as u8;
            let end_refs = parser.load_uint(3)? as u8;

            let cell_ref = parser.load_reference_cloned()?;

            let mut parser = cell_ref.as_slice_allow_exotic();
            parser.skip_first(start_bits, start_refs)?;

            let root_data_size = end_bits.saturating_sub(start_bits);
            let mut root_bits = vec![0u8; root_data_size.div_ceil(8) as usize];
            parser.load_raw(&mut root_bits, root_data_size)?;

            let mut builder = CellBuilder::new();
            builder.store_raw(&root_bits, root_data_size)?;

            for _ in start_refs..end_refs {
                let next_ref = parser.load_reference_cloned()?;
                builder.store_reference(next_ref)?;
            }

            let final_cell = builder.build()?;

            Ok(TupleItem::Slice(final_cell))
        }
        5 => {
            let cell = parser.load_reference_cloned()?;
            Ok(TupleItem::Builder(cell))
        }
        7 => {
            let length = parser.load_u16()? as usize;
            let mut items: Vec<TupleItem> = Vec::with_capacity(length);

            if length > 1 {
                let head_ref = parser.load_reference_cloned()?;
                let tail_ref = parser.load_reference_cloned()?;

                let mut tail_parser = tail_ref.as_slice_allow_exotic();
                items.insert(0, parse_tuple_item(&mut tail_parser)?);

                let mut head_refs = vec![head_ref];
                let mut current_parser = head_refs[0].as_slice_allow_exotic();

                for _ in 0..length - 2 {
                    let old_head = current_parser.load_reference_cloned()?;
                    let new_tail = current_parser.load_reference_cloned()?;

                    let mut new_tail_parser = new_tail.as_slice_allow_exotic();
                    items.insert(0, parse_tuple_item(&mut new_tail_parser)?);

                    head_refs.push(old_head);
                    current_parser = head_refs[head_refs.len() - 1].as_slice_allow_exotic();
                }

                items.insert(0, parse_tuple_item(&mut current_parser)?);
            } else if length == 1 {
                let ref_cell = parser.load_reference_cloned()?;
                let mut item_parser = ref_cell.as_slice_allow_exotic();
                items.push(parse_tuple_item(&mut item_parser)?);
            }

            Ok(TupleItem::Tuple(Tuple(items)))
        }
        6 => {
            // TODO: support continuation
            Ok(TupleItem::Null)
        }
        _ => Err(anyhow!("Unsupported stack item kind: {kind}")),
    }
}

/// Serialize a tuple (stack) to a cell
pub fn serialize_tuple(src: &Tuple) -> Result<Cell, anyhow::Error> {
    let mut builder = CellBuilder::new();
    builder.store_uint(src.0.len() as u64, 24)?;
    serialize_tuple_tail(&src.0, &mut builder)?;
    builder.build().map_err(Into::into)
}

fn serialize_tuple_tail(src: &[TupleItem], builder: &mut CellBuilder) -> anyhow::Result<()> {
    if !src.is_empty() {
        // rest:^(VmStackList n)
        let mut tail_builder = CellBuilder::new();
        serialize_tuple_tail(&src[..src.len() - 1], &mut tail_builder)?;
        let tail_cell = tail_builder.build()?;
        builder.store_reference(tail_cell)?;

        // tos
        serialize_tuple_item(builder, &src[src.len() - 1])?;
    }
    Ok(())
}

/// Parse a tuple (stack) from a cell
pub fn parse_tuple(src: &Cell) -> Result<Tuple, anyhow::Error> {
    let mut cur_cell = src.clone();
    let mut cs = cur_cell.as_slice_allow_exotic();

    let size = cs.load_uint(24)? as usize;
    let mut result: Vec<TupleItem> = Vec::with_capacity(size);

    for _ in 0..size {
        let next_ref = cs.load_reference_cloned()?;
        let item = parse_tuple_item(&mut cs)?;
        result.insert(0, item);

        cur_cell = next_ref;
        cs = cur_cell.as_slice_allow_exotic();
    }

    Ok(Tuple(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::needless_pass_by_value)]
    fn roundtrip_test(item: TupleItem) {
        let mut builder = CellBuilder::new();
        serialize_tuple_item(&mut builder, &item).unwrap();
        let cell = builder.build().unwrap();

        let mut parser = cell.as_slice_allow_exotic();
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
        roundtrip_test(TupleItem::Int(BigInt::from(42)));
        roundtrip_test(TupleItem::Int(BigInt::from(-123)));
    }

    #[test]
    fn test_large_int_roundtrip() {
        roundtrip_test(TupleItem::Int(BigInt::from(2u64).pow(100)));
    }

    #[test]
    fn test_nan_roundtrip() {
        roundtrip_test(TupleItem::Nan);
    }

    #[test]
    fn test_cell_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        let cell = builder.build().unwrap();

        roundtrip_test(TupleItem::Cell(cell));
    }

    #[test]
    fn test_slice_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        builder.store_small_uint(43, 8).unwrap();
        let cell = builder.build().unwrap();

        roundtrip_test(TupleItem::Slice(cell));
    }

    #[test]
    fn test_builder_roundtrip() {
        let mut builder = CellBuilder::new();
        builder.store_small_uint(42, 8).unwrap();
        let cell = builder.build().unwrap();

        roundtrip_test(TupleItem::Builder(cell));
    }

    #[test]
    fn test_empty_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![])));
    }

    #[test]
    fn test_single_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(
            42,
        ))])));
    }

    #[test]
    fn test_multiple_item_tuple_roundtrip() {
        roundtrip_test(TupleItem::Tuple(Tuple(vec![
            TupleItem::Int(BigInt::from(42)),
            TupleItem::Null,
            TupleItem::Nan,
        ])));
    }

    #[test]
    fn test_nested_tuple_roundtrip() {
        let nested = TupleItem::Tuple(Tuple(vec![
            TupleItem::Int(BigInt::from(1)),
            TupleItem::Tuple(Tuple(vec![
                TupleItem::Int(BigInt::from(2)),
                TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(3))])),
            ])),
        ]));

        roundtrip_test(nested);
    }

    #[test]
    fn test_serialize_deserialize_empty_tuple() {
        roundtrip_tuple_test(vec![]);
    }

    #[test]
    fn test_serialize_deserialize_simple_tuple() {
        roundtrip_tuple_test(vec![
            TupleItem::Int(BigInt::from(42)),
            TupleItem::Null,
            TupleItem::Nan,
        ]);
    }

    #[test]
    fn test_serialize_deserialize_complex_tuple() {
        let mut cell_builder = CellBuilder::new();
        cell_builder.store_small_uint(0xAB, 8).unwrap();
        let test_cell = cell_builder.build().unwrap();

        roundtrip_tuple_test(vec![
            TupleItem::Int(BigInt::from(12345)),
            TupleItem::Cell(test_cell.clone()),
            TupleItem::Tuple(Tuple(vec![
                TupleItem::Slice(test_cell.clone()),
                TupleItem::Int(BigInt::from(-9876)),
            ])),
            TupleItem::Builder(test_cell),
        ]);
    }

    #[test]
    fn test_serialize_deserialize_deeply_nested() {
        let mut nested = TupleItem::Int(BigInt::from(0));

        for i in 1..10 {
            nested = TupleItem::Tuple(Tuple(vec![TupleItem::Int(BigInt::from(i)), nested]));
        }

        roundtrip_tuple_test(vec![nested]);
    }

    #[test]
    fn test_int_boundary_values() {
        // Test i64 boundaries
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MAX)));
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MIN)));

        // Test values just outside i64 range (should use int257 encoding)
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MAX) + BigInt::from(1)));
        roundtrip_test(TupleItem::Int(BigInt::from(i64::MIN) - BigInt::from(1)));
    }
}
