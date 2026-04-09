use crate::stack::{ContData, Tuple, TupleItem};
use anyhow::anyhow;
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, CellSlice};
use tycho_types::dict::RawDict;

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
        TupleItem::Cont(cont) => {
            // vm_stk_cont#06 cont:VmCont = VmStackValue;
            builder.store_small_uint(0x06, 8)?;
            serialize_vm_cont(builder, cont)?;
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

pub fn parse_vm_cell_slice(parser: &mut CellSlice<'_>) -> Result<Cell, anyhow::Error> {
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

    Ok(final_cell)
}

/// Parse VmStack from a cell slice.
///
/// ```text
/// vm_stack#_ depth:(## 24) stack:(VmStackList depth) = VmStack;
/// vm_stk_cons#_ {n:#} rest:^(VmStackList n) tos:VmStackValue = VmStackList (n + 1);
/// vm_stk_nil#_ = VmStackList 0;
/// ```
fn parse_vm_stack(parser: &mut CellSlice<'_>) -> Result<Tuple, anyhow::Error> {
    let size = parser.load_uint(24)? as usize;
    if size == 0 {
        return Ok(Tuple::empty());
    }

    let mut result: Vec<TupleItem> = Vec::with_capacity(size);

    // First entry: rest reference + tos inline in current parser
    let next_ref = parser.load_reference_cloned()?;
    let item = parse_tuple_item(parser)?;
    result.insert(0, item);

    // Remaining entries from referenced cells
    let mut cur_cell = next_ref;
    for _ in 1..size {
        let mut cs = cur_cell.as_slice_allow_exotic();
        let nr = cs.load_reference_cloned()?;
        let item = parse_tuple_item(&mut cs)?;
        result.insert(0, item);
        cur_cell = nr;
    }

    Ok(Tuple(result))
}

/// Serialize VmStack into a cell builder.
fn serialize_vm_stack(builder: &mut CellBuilder, stack: &Tuple) -> anyhow::Result<()> {
    builder.store_uint(stack.len() as u64, 24)?;
    serialize_tuple_tail(&stack.0, builder)
}

/// Parsed VmControlData fields.
struct VmControlData {
    stack: Option<Tuple>,
    savelist: Option<Cell>,
}

/// Parse VmControlData, returning the captured stack and save list.
///
/// ```text
/// vm_ctl_data$_ nargs:(Maybe uint13) stack:(Maybe VmStack)
///               save:VmSaveList cp:(Maybe int16) = VmControlData;
/// _ cregs:(HashmapE 4 VmStackValue) = VmSaveList;
/// ```
fn parse_vm_control_data(parser: &mut CellSlice<'_>) -> Result<VmControlData, anyhow::Error> {
    // nargs:(Maybe uint13)
    if parser.load_bit()? {
        parser.load_uint(13)?;
    }

    // stack:(Maybe VmStack)
    let stack = if parser.load_bit()? {
        Some(parse_vm_stack(parser)?)
    } else {
        None
    };

    // save:VmSaveList = HashmapE 4 VmStackValue
    let savelist = if parser.load_bit()? {
        Some(parser.load_reference_cloned()?)
    } else {
        None
    };

    // cp:(Maybe int16)
    if parser.load_bit()? {
        parser.load_uint(16)?;
    }

    Ok(VmControlData { stack, savelist })
}

/// Parse a VmCont from a cell slice.
///
/// Supports all TLB-defined continuation variants:
/// ```text
/// vmc_std$00          cdata:VmControlData code:VmCellSlice = VmCont;
/// vmc_envelope$01     cdata:VmControlData next:^VmCont = VmCont;
/// vmc_quit$1000       exit_code:int32 = VmCont;
/// vmc_quit_exc$1001   = VmCont;
/// vmc_repeat$10100    count:uint63 body:^VmCont after:^VmCont = VmCont;
/// vmc_until$110000    body:^VmCont after:^VmCont = VmCont;
/// vmc_again$110001    body:^VmCont = VmCont;
/// vmc_while_cond$110010 cond:^VmCont body:^VmCont after:^VmCont = VmCont;
/// vmc_while_body$110011 cond:^VmCont body:^VmCont after:^VmCont = VmCont;
/// vmc_pushint$1111    value:int32 next:^VmCont = VmCont;
/// ```
#[allow(clippy::branches_sharing_code)]
fn parse_vm_cont(parser: &mut CellSlice<'_>) -> Result<ContData, anyhow::Error> {
    let first_bit = parser.load_bit()?;

    if !first_bit {
        // Tags starting with 0: vmc_std (00) or vmc_envelope (01)
        let second_bit = parser.load_bit()?;

        if !second_bit {
            // vmc_std$00 cdata:VmControlData code:VmCellSlice
            let cdata = parse_vm_control_data(parser)?;
            let code = parse_vm_cell_slice(parser)?;
            Ok(ContData {
                code,
                stack: cdata.stack,
                savelist: cdata.savelist,
            })
        } else {
            // vmc_envelope$01 cdata:VmControlData next:^VmCont
            let cdata = parse_vm_control_data(parser)?;
            let next = parser.load_reference_cloned()?;
            let mut next_parser = next.as_slice_allow_exotic();
            let inner = parse_vm_cont(&mut next_parser)?;

            let merged_stack = match (cdata.stack, inner.stack) {
                (Some(outer), Some(inner)) => {
                    let mut combined = inner;
                    combined.0.extend(outer.0);
                    Some(combined)
                }
                (s @ Some(_), None) | (None, s @ Some(_)) => s,
                (None, None) => None,
            };

            let merged_savelist = match (cdata.savelist, inner.savelist) {
                (Some(outer), Some(inner_sl)) => {
                    let mut merged: RawDict<4> = Some(inner_sl).into();
                    let outer_dict: RawDict<4> = Some(outer).into();
                    for entry in outer_dict.iter() {
                        let (key, value): (_, CellSlice<'_>) = entry?;
                        merged.set_ext(key.as_data_slice(), &value, Cell::empty_context())?;
                    }
                    merged.into_root()
                }
                (s @ Some(_), None) | (None, s @ Some(_)) => s,
                (None, None) => None,
            };

            Ok(ContData {
                code: inner.code,
                stack: merged_stack,
                savelist: merged_savelist,
            })
        }
    } else {
        // Tags starting with 1
        let second_bit = parser.load_bit()?;

        if !second_bit {
            // Tags starting with 10
            let third_bit = parser.load_bit()?;

            if !third_bit {
                // Tags starting with 100
                let fourth_bit = parser.load_bit()?;

                if !fourth_bit {
                    // vmc_quit$1000 exit_code:int32
                    parser.load_uint(32)?;
                    Ok(ContData::default())
                } else {
                    // vmc_quit_exc$1001
                    Ok(ContData::default())
                }
            } else {
                // Tags starting with 101
                let fourth_bit = parser.load_bit()?;

                if !fourth_bit {
                    // vmc_repeat$10100 count:uint63 body:^VmCont after:^VmCont
                    let fifth_bit = parser.load_bit()?;
                    if fifth_bit {
                        return Err(anyhow!("Unsupported VmCont tag starting with 10101"));
                    }
                    parser.load_uint(63)?; // count
                    let body = parser.load_reference_cloned()?;
                    parser.load_reference_cloned()?; // after
                    let mut bp = body.as_slice_allow_exotic();
                    parse_vm_cont(&mut bp)
                } else {
                    Err(anyhow!("Unsupported VmCont tag starting with 1011"))
                }
            }
        } else {
            // Tags starting with 11
            let third_bit = parser.load_bit()?;

            if !third_bit {
                // Tags starting with 110
                let fourth_bit = parser.load_bit()?;

                if !fourth_bit {
                    // Tags starting with 1100
                    let sub_tag = parser.load_uint(2)?;
                    match sub_tag {
                        0b00 => {
                            // vmc_until$110000 body:^VmCont after:^VmCont
                            let body = parser.load_reference_cloned()?;
                            parser.load_reference_cloned()?; // after
                            let mut bp = body.as_slice_allow_exotic();
                            parse_vm_cont(&mut bp)
                        }
                        0b01 => {
                            // vmc_again$110001 body:^VmCont
                            let body = parser.load_reference_cloned()?;
                            let mut bp = body.as_slice_allow_exotic();
                            parse_vm_cont(&mut bp)
                        }
                        0b10 => {
                            // vmc_while_cond$110010 cond:^VmCont body:^VmCont after:^VmCont
                            parser.load_reference_cloned()?; // cond
                            let body = parser.load_reference_cloned()?;
                            parser.load_reference_cloned()?; // after
                            let mut bp = body.as_slice_allow_exotic();
                            parse_vm_cont(&mut bp)
                        }
                        0b11 => {
                            // vmc_while_body$110011 cond:^VmCont body:^VmCont after:^VmCont
                            parser.load_reference_cloned()?; // cond
                            let body = parser.load_reference_cloned()?;
                            parser.load_reference_cloned()?; // after
                            let mut bp = body.as_slice_allow_exotic();
                            parse_vm_cont(&mut bp)
                        }
                        _ => unreachable!(),
                    }
                } else {
                    // Tags starting with 1101 — undefined
                    Err(anyhow!("Unsupported VmCont tag starting with 1101"))
                }
            } else {
                // Tags starting with 111
                let fourth_bit = parser.load_bit()?;

                if fourth_bit {
                    // vmc_pushint$1111 value:int32 next:^VmCont
                    parser.load_uint(32)?; // value
                    let next = parser.load_reference_cloned()?;
                    let mut np = next.as_slice_allow_exotic();
                    parse_vm_cont(&mut np)
                } else {
                    // Tags starting with 1110 — undefined
                    Err(anyhow!("Unsupported VmCont tag starting with 1110"))
                }
            }
        }
    }
}

/// Serialize a VmCont as vmc_std into a cell builder.
///
/// Always serializes as `vmc_std$00` with VmControlData and VmCellSlice.
pub fn serialize_vm_cont(builder: &mut CellBuilder, cont: &ContData) -> anyhow::Result<()> {
    // vmc_std$00
    builder.store_uint(0b00, 2)?;

    // VmControlData:
    // nargs:(Maybe uint13) — absent
    builder.store_bit(false)?;

    // stack:(Maybe VmStack)
    if let Some(stack) = &cont.stack {
        builder.store_bit(true)?;
        serialize_vm_stack(builder, stack)?;
    } else {
        builder.store_bit(false)?;
    }

    // save:VmSaveList = HashmapE 4 VmStackValue
    if let Some(savelist) = &cont.savelist {
        builder.store_bit(true)?;
        builder.store_reference(savelist.clone())?;
    } else {
        builder.store_bit(false)?;
    }

    // cp:(Maybe int16) — always serialize as codepage 0
    builder.store_bit(true)?;
    builder.store_uint(0, 16)?;

    // code:VmCellSlice
    let code = &cont.code;
    builder.store_uint(0, 10)?; // st_bits
    builder.store_uint(code.bit_len() as u64, 10)?; // end_bits
    builder.store_uint(0, 3)?; // st_ref
    builder.store_uint(code.reference_count() as u64, 3)?; // end_ref
    builder.store_reference(code.clone())?;

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
        4 => Ok(TupleItem::Slice(parse_vm_cell_slice(parser)?)),
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
            // vm_stk_cont#06 cont:VmCont = VmStackValue;
            let cont = parse_vm_cont(parser)?;
            Ok(TupleItem::Cont(cont))
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
