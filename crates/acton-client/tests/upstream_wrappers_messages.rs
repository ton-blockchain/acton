#![allow(clippy::needless_raw_string_hashes)]

use acton_client::__private::tycho_types::cell::{CellBuilder, CellFamily, CellSlice};
use acton_client::__private::tycho_types::models::{AnyAddr, ExtAddr, StdAddrFormat};
use acton_client::{
    AbiError, AbiLoad, AbiStore, BigInt, Cell, CellRef, Dictionary, DynamicAbi, DynamicError,
    DynamicPackFn, DynamicUnpackFn, DynamicValue, OwnedSlice, StdAddr, register_custom_codec,
};
use expect_test::{Expect, expect};
use std::fmt::Write as _;
use std::sync::{Arc, Once};

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-wrappers.abi.json")]
mod generated {}

fn cell_tree(cell: &Cell) -> String {
    fn write_cell(cell: &Cell, indent: usize, output: &mut String) {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&" ".repeat(indent));
        write!(
            output,
            "x{{{:X}}}",
            cell.as_slice()
                .expect("cell must be readable")
                .display_data()
        )
        .expect("writing to a String must succeed");
        for index in 0..cell.reference_count() {
            let child = cell
                .reference_cloned(index)
                .expect("cell reference must exist");
            write_cell(&child, indent + 1, output);
        }
    }

    let mut output = String::new();
    write_cell(cell, 0, &mut output);
    output
}

fn cell_from_text(text: &str, refs: Vec<Cell>) -> Cell {
    let text = text
        .strip_prefix("x{")
        .and_then(|text| text.strip_suffix('}'))
        .expect("test cell must use x{...} notation");
    let (hex, padded) = text
        .strip_suffix('_')
        .map_or((text, false), |hex| (hex, true));
    let mut bits = Vec::with_capacity(hex.len() * 4);
    for digit in hex.bytes() {
        let nibble = match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => panic!("invalid test cell digit"),
        };
        for shift in (0..4).rev() {
            bits.push((nibble >> shift) & 1 != 0);
        }
    }
    if padded {
        while bits.last() == Some(&false) {
            bits.pop();
        }
        assert_eq!(
            bits.pop(),
            Some(true),
            "padded cell must contain a terminator"
        );
    }

    let mut builder = CellBuilder::new();
    for bit in bits {
        builder.store_bit(bit).expect("test bit must fit");
    }
    for cell in refs {
        builder
            .store_reference(cell)
            .expect("test reference must fit");
    }
    builder.build().expect("test cell must build")
}

fn owned_slice(text: &str) -> OwnedSlice {
    OwnedSlice::full(cell_from_text(text, Vec::new()))
}

fn run<T>(mut value: T, expected: &Expect) -> T
where
    T: AbiStore + AbiLoad,
{
    let mut last_cell = None;
    for _ in 0..2 {
        let cell = value.to_cell().expect("value must encode");
        expected.assert_eq(&cell_tree(&cell));
        value = T::from_cell(&cell).expect("value must decode");
        last_cell = Some(cell);
    }
    let type_name = std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .expect("Rust type must have a name");
    run_dynamic(
        type_name,
        last_cell
            .as_ref()
            .expect("generated cycle must produce a cell"),
        expected,
    );
    value
}

fn run_alias<T>(
    mut value: T,
    expected: &Expect,
    alias_name: &str,
    store: fn(&T, &mut CellBuilder) -> Result<(), AbiError>,
    load: fn(&mut CellSlice<'_>) -> Result<T, AbiError>,
) -> T {
    let mut last_cell = None;
    for _ in 0..2 {
        let mut builder = CellBuilder::new();
        store(&value, &mut builder).expect("value must encode");
        let cell = builder.build().expect("cell must build");
        expected.assert_eq(&cell_tree(&cell));
        let mut slice = cell.as_slice().expect("cell must be readable");
        value = load(&mut slice).expect("value must decode");
        acton_client::cell::ensure_empty(&slice).expect("slice must be exhausted");
        last_cell = Some(cell);
    }
    run_dynamic(
        alias_name,
        last_cell
            .as_ref()
            .expect("generated cycle must produce a cell"),
        expected,
    );
    value
}

fn dynamic_input_error(type_name: &str, expected: &str) -> DynamicError {
    DynamicError::InvalidInput {
        field_path: type_name.to_owned(),
        expected: expected.to_owned(),
        reason: "unexpected dynamic custom value".to_owned(),
    }
}

fn new_dynamic_abi() -> DynamicAbi {
    let mut abi = DynamicAbi::from_json(generated::ABI_JSON).expect("fixture ABI must parse");
    abi.register_custom_codec(
        "TelegramString",
        Some(Arc::new(|value: &DynamicValue, builder: &mut CellBuilder| {
            let DynamicValue::Slice(value) = value else {
                return Err(dynamic_input_error("TelegramString", "slice"));
            };
            let value = value.as_slice()?;
            builder.store_uint(u64::from(value.size_bits().div_ceil(8)), 8)?;
            builder.store_slice(value)?;
            Ok(())
        }) as DynamicPackFn),
        Some(Arc::new(|slice: &mut CellSlice<'_>| {
            let bits = u16::from(slice.load_u8()?) * 8;
            let prefix = slice.get_prefix(bits, 0);
            let mut builder = CellBuilder::new();
            builder.store_slice(prefix)?;
            slice.skip_first(bits, 0)?;
            Ok(DynamicValue::Slice(OwnedSlice::full(builder.build()?)))
        }) as DynamicUnpackFn),
    )
    .expect("TelegramString dynamic codec must register");
    abi.register_custom_codec(
        "Custom8",
        Some(Arc::new(|value: &DynamicValue, builder: &mut CellBuilder| {
            let DynamicValue::Number(value) = value else {
                return Err(dynamic_input_error("Custom8", "number"));
            };
            acton_client::cell::store_fixed_int(builder, value, 8, false)?;
            Ok(())
        }) as DynamicPackFn),
        Some(Arc::new(|slice: &mut CellSlice<'_>| {
            Ok(DynamicValue::Number(acton_client::cell::load_fixed_int(
                slice, 8, false,
            )?))
        }) as DynamicUnpackFn),
    )
    .expect("Custom8 dynamic codec must register");
    abi.register_custom_codec(
        "MyBorderedInt",
        Some(Arc::new(|value: &DynamicValue, builder: &mut CellBuilder| {
            let DynamicValue::Number(value) = value else {
                return Err(dynamic_input_error("MyBorderedInt", "number"));
            };
            let encoded = if value > &BigInt::from(10) {
                1
            } else if value > &BigInt::from(0) {
                2
            } else {
                3
            };
            builder.store_uint(encoded, 4)?;
            Ok(())
        }) as DynamicPackFn),
        Some(Arc::new(|slice: &mut CellSlice<'_>| {
            let value = match slice.load_uint(4)? {
                1 => BigInt::from(10),
                2 => BigInt::from(0),
                3 => BigInt::from(-1),
                _ => return Err(dynamic_input_error("MyBorderedInt", "valid border")),
            };
            Ok(DynamicValue::Number(value))
        }) as DynamicUnpackFn),
    )
    .expect("MyBorderedInt dynamic codec must register");
    abi.register_custom_codec(
        "Tensor3Skipping1",
        Some(Arc::new(|value: &DynamicValue, builder: &mut CellBuilder| {
            let DynamicValue::Array(values) = value else {
                return Err(dynamic_input_error("Tensor3Skipping1", "array"));
            };
            let [DynamicValue::Number(first), _, DynamicValue::Number(third)] = values.as_slice()
            else {
                return Err(dynamic_input_error(
                    "Tensor3Skipping1",
                    "three-number array",
                ));
            };
            acton_client::cell::store_fixed_int(builder, first, 8, false)?;
            acton_client::cell::store_fixed_int(builder, third, 8, false)?;
            Ok(())
        }) as DynamicPackFn),
        Some(Arc::new(|slice: &mut CellSlice<'_>| {
            Ok(DynamicValue::Array(vec![
                DynamicValue::Number(acton_client::cell::load_fixed_int(slice, 8, false)?),
                DynamicValue::Number(BigInt::from(0)),
                DynamicValue::Number(acton_client::cell::load_fixed_int(slice, 8, false)?),
            ]))
        }) as DynamicUnpackFn),
    )
    .expect("Tensor3Skipping1 dynamic codec must register");
    abi.register_custom_codec(
        "OnlyWithPack",
        Some(Arc::new(|value: &DynamicValue, builder: &mut CellBuilder| {
            let DynamicValue::Number(value) = value else {
                return Err(dynamic_input_error("OnlyWithPack", "number"));
            };
            builder.store_uint(0xff, 8)?;
            acton_client::cell::store_fixed_int(builder, value, 8, false)?;
            Ok(())
        }) as DynamicPackFn),
        None,
    )
    .expect("OnlyWithPack dynamic codec must register");
    abi.register_custom_codec(
        "OnlyWithUnpack",
        None,
        Some(Arc::new(|slice: &mut CellSlice<'_>| {
            if slice.load_u8()? != 0xff {
                return Err(dynamic_input_error(
                    "OnlyWithUnpack",
                    "0xff-prefixed number",
                ));
            }
            Ok(DynamicValue::Number(acton_client::cell::load_fixed_int(
                slice, 8, false,
            )?))
        }) as DynamicUnpackFn),
    )
    .expect("OnlyWithUnpack dynamic codec must register");
    abi
}

fn run_dynamic(type_name: &str, generated_cell: &Cell, expected: &Expect) -> DynamicValue {
    let abi = new_dynamic_abi();
    let ty_idx = abi
        .declaration_type_index(type_name)
        .unwrap_or_else(|| panic!("dynamic ABI declaration `{type_name}` must exist"));
    let mut initial_slice = generated_cell
        .as_slice()
        .expect("generated cell must be readable");
    let mut value = abi
        .unpack_from_slice(ty_idx, &mut initial_slice)
        .expect("generated cell must dynamically decode");
    acton_client::cell::ensure_empty(&initial_slice)
        .expect("dynamic initial slice must be exhausted");
    for _ in 0..2 {
        let cell = abi
            .pack_to_cell(ty_idx, &value)
            .expect("dynamic value must encode");
        expected.assert_eq(&cell_tree(&cell));
        let mut slice = cell.as_slice().expect("dynamic cell must be readable");
        value = abi
            .unpack_from_slice(ty_idx, &mut slice)
            .expect("dynamic value must decode");
        acton_client::cell::ensure_empty(&slice).expect("dynamic slice must be exhausted");
    }
    value
}

fn load_alias<T>(
    cell: &Cell,
    load: fn(&mut CellSlice<'_>) -> Result<T, AbiError>,
) -> Result<T, AbiError> {
    let mut slice = cell.as_slice()?;
    let value = load(&mut slice)?;
    acton_client::cell::ensure_empty(&slice)?;
    Ok(value)
}

fn load_prefix<T: AbiLoad>(cell: &Cell) -> Result<(T, u16, u8), AbiError> {
    let mut slice = cell.as_slice()?;
    let value = T::load_from(&mut slice)?;
    Ok((value, slice.size_bits(), slice.size_refs()))
}

fn friendly_address(value: &str) -> StdAddr {
    StdAddr::from_str_ext(value, StdAddrFormat::any())
        .expect("friendly address must parse")
        .0
}

fn external_address(value: u64, bits: u16) -> AnyAddr {
    let byte_len = usize::from(bits.div_ceil(8));
    let padding = u32::from((8 - bits % 8) % 8);
    let bytes = value
        .checked_shl(padding)
        .expect("external address must fit")
        .to_be_bytes();
    let mut data = vec![0; byte_len];
    let copied = byte_len.min(bytes.len());
    data[byte_len - copied..].copy_from_slice(&bytes[bytes.len() - copied..]);
    AnyAddr::Ext(ExtAddr::new(bits, data).expect("external address must fit"))
}

fn slice_44_with_ref_45() -> OwnedSlice {
    let mut child = CellBuilder::new();
    child.store_uint(45, 32).expect("value must fit");
    let mut root = CellBuilder::new();
    root.store_uint(44, 32).expect("value must fit");
    root.store_reference(child.build().expect("child must build"))
        .expect("reference must fit");
    OwnedSlice::full(root.build().expect("root must build"))
}

fn dict_get<'a, K: PartialEq, V>(dictionary: &'a Dictionary<K, V>, key: &K) -> Option<&'a V> {
    dictionary
        .0
        .iter()
        .find_map(|(candidate, value)| (candidate == key).then_some(value))
}

fn register_codecs() {
    static REGISTER: Once = Once::new();
    REGISTER.call_once(|| {
        register_custom_codec::<generated::TelegramString>(
            "TelegramString",
            Some(|value: &OwnedSlice, builder: &mut CellBuilder| {
                let value = value.as_slice()?;
                let bytes = value.size_bits().div_ceil(8);
                builder.store_uint(u64::from(bytes), 8)?;
                builder.store_slice(value)?;
                Ok(())
            }),
            Some(|slice: &mut CellSlice<'_>| {
                let bits = u16::from(slice.load_u8()?) * 8;
                let prefix = slice.get_prefix(bits, 0);
                let mut builder = CellBuilder::new();
                builder.store_slice(prefix)?;
                slice.skip_first(bits, 0)?;
                Ok(OwnedSlice::full(builder.build()?))
            }),
        )
        .expect("TelegramString codec must register");
        register_custom_codec::<generated::Custom8>(
            "Custom8",
            Some(|value: &BigInt, builder: &mut CellBuilder| {
                acton_client::cell::store_fixed_int(builder, value, 8, false)
            }),
            Some(|slice: &mut CellSlice<'_>| acton_client::cell::load_fixed_int(slice, 8, false)),
        )
        .expect("Custom8 codec must register");
        register_custom_codec::<generated::MyBorderedInt>(
            "MyBorderedInt",
            Some(|value: &BigInt, builder: &mut CellBuilder| {
                let encoded = if value > &BigInt::from(10) {
                    1
                } else if value > &BigInt::from(0) {
                    2
                } else {
                    3
                };
                builder.store_uint(encoded, 4)?;
                Ok(())
            }),
            Some(|slice: &mut CellSlice<'_>| {
                Ok(match slice.load_uint(4)? {
                    1 => BigInt::from(10),
                    2 => BigInt::from(0),
                    3 => BigInt::from(-1),
                    _ => return acton_client::cell::invalid_data("invalid MyBorderedInt"),
                })
            }),
        )
        .expect("MyBorderedInt codec must register");
        register_custom_codec::<generated::MyCustomNothing>(
            "MyCustomNothing",
            Some(|_: &(), builder: &mut CellBuilder| {
                builder.store_uint(123, 32)?;
                builder.store_reference(Cell::empty_cell())?;
                Ok(())
            }),
            None::<fn(&mut CellSlice<'_>) -> Result<(), AbiError>>,
        )
        .expect("MyCustomNothing codec must register");
        register_custom_codec::<generated::Tensor3Skipping1>(
            "Tensor3Skipping1",
            Some(
                |value: &generated::Tensor3Skipping1, builder: &mut CellBuilder| {
                    acton_client::cell::store_fixed_int(builder, &value.0, 8, false)?;
                    acton_client::cell::store_fixed_int(builder, &value.2, 8, false)
                },
            ),
            Some(|slice: &mut CellSlice<'_>| {
                Ok((
                    acton_client::cell::load_fixed_int(slice, 8, false)?,
                    BigInt::from(0),
                    acton_client::cell::load_fixed_int(slice, 8, false)?,
                ))
            }),
        )
        .expect("Tensor3Skipping1 codec must register");
        register_custom_codec::<generated::Color>(
            "Color",
            Some(|value: &generated::Color, builder: &mut CellBuilder| {
                acton_client::cell::store_fixed_int(builder, &value.0, 5, false)
            }),
            Some(|slice: &mut CellSlice<'_>| {
                Ok(generated::Color(acton_client::cell::load_fixed_int(
                    slice, 5, false,
                )?))
            }),
        )
        .expect("Color codec must register");
        register_custom_codec::<generated::OnlyWithPack>(
            "OnlyWithPack",
            Some(|value: &BigInt, builder: &mut CellBuilder| {
                builder.store_uint(0xff, 8)?;
                acton_client::cell::store_fixed_int(builder, value, 8, false)
            }),
            None::<fn(&mut CellSlice<'_>) -> Result<BigInt, AbiError>>,
        )
        .expect("OnlyWithPack codec must register");
        register_custom_codec::<generated::OnlyWithUnpack>(
            "OnlyWithUnpack",
            None::<fn(&BigInt, &mut CellBuilder) -> Result<(), AbiError>>,
            Some(|slice: &mut CellSlice<'_>| {
                if slice.load_u8()? != 0xff {
                    return acton_client::cell::invalid_data("expected OnlyWithUnpack prefix");
                }
                acton_client::cell::load_fixed_int(slice, 8, false)
            }),
        )
        .expect("OnlyWithUnpack codec must register");
    });
}

#[test]
fn msg_single_prefix32() {
    let decoded = run(
        generated::MsgSinglePrefix32 {
            amount1: BigInt::from(80),
            amount2: BigInt::from(800_000_000),
        },
        &expect!["x{8765432115042FAF0800}"],
    );
    expect![[r#"
        (
            80,
            800000000,
        )
    "#]]
    .assert_debug_eq(&(decoded.amount1, decoded.amount2));
}

#[test]
fn msg_single_prefix48() {
    let coins = run(
        generated::MsgSinglePrefix48 {
            amount: generated::UnionTy165::Variant0(BigInt::from(800_000_000)),
        },
        &expect!["x{876543211234217D784004_}"],
    );
    run(
        generated::MsgSinglePrefix48 {
            amount: generated::UnionTy165::Variant1(BigInt::from(80)),
        },
        &expect!["x{87654321123480000000000000284_}"],
    );

    expect![[r#"
        Variant0(
            800000000,
        )
    "#]]
    .assert_debug_eq(&coins.amount);
}

#[test]
fn msg_counter1() {
    run_alias(
        generated::UnionTy188::Variant0(generated::CounterIncrement {
            counter_id: BigInt::from(123),
            inc_by: BigInt::from(78),
        }),
        &expect!["x{123456787B0000004E}"],
        "MsgCounter1",
        generated::store_msg_counter1,
        generated::load_msg_counter1,
    );
    run_alias(
        generated::UnionTy188::Variant1(generated::CounterDecrement {
            counter_id: BigInt::from(0),
            dec_by: BigInt::from(-38),
        }),
        &expect!["x{2345678900FFFFFFDA}"],
        "MsgCounter1",
        generated::store_msg_counter1,
        generated::load_msg_counter1,
    );
    run_alias(
        generated::UnionTy188::Variant2(generated::CounterReset0 {
            counter_id: BigInt::from(0),
        }),
        &expect!["x{3456789000}"],
        "MsgCounter1",
        generated::store_msg_counter1,
        generated::load_msg_counter1,
    );
    run_alias(
        generated::UnionTy188::Variant3(generated::CounterResetTo {
            counter_id: BigInt::from(0),
            initial_value: BigInt::from(29_874_329_774_732_i64),
        }),
        &expect!["x{001843000000001B2BA8D06A8C}"],
        "MsgCounter1",
        generated::store_msg_counter1,
        generated::load_msg_counter1,
    );

    let decrement_error = generated::CounterDecrement::from_cell(&cell_from_text(
        "x{FFFFFFFFFFFFFFFFFFFFFF}",
        Vec::new(),
    ))
    .expect_err("incorrect decrement prefix must fail")
    .to_string();
    let reset_error =
        generated::CounterResetTo::from_cell(&cell_from_text("x{ABCDEF00}", Vec::new()))
            .expect_err("incorrect reset prefix must fail")
            .to_string();
    let decoded = load_alias(
        &cell_from_text("x{3456789000}", Vec::new()),
        generated::load_msg_counter1,
    )
    .expect("message union must decode");

    expect![[r#"
        (
            "Incorrect prefix for 'CounterDecrement': expected 0x23456789, got 0xffffffff",
            "Incorrect prefix for 'CounterResetTo': expected 0x00184300, got 0xabcdef00",
            Variant2(
                CounterReset0 {
                    counter_id: 0,
                },
            ),
        )
    "#]]
    .assert_debug_eq(&(decrement_error, reset_error, decoded));
}

#[test]
fn msg_external1() {
    let zero = friendly_address("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c");
    let masterchain = friendly_address("Ef8o6AM9sUZ8rOqLFY8PYeaC3gbopZR1BMkE8fcD0r5NnmCi");

    run_alias(
        generated::UnionTy197::Variant0(generated::SayHiAndGoodbye {
            dest_addr: None,
            body: generated::UnionTy192::Variant1(generated::BodyPayload2 {
                master_id: BigInt::from(10),
                owner_address: zero.clone(),
            }),
        }),
        &expect!["x{8910A8000000000000000000000000000000000000000000000000000000000000000001_}"],
        "MsgExternal1",
        generated::store_msg_external1,
        generated::load_msg_external1,
    );
    run_alias(
        generated::UnionTy197::Variant0(generated::SayHiAndGoodbye {
            dest_addr: Some(zero.clone()),
            body: generated::UnionTy192::Variant0(generated::BodyPayload1 {
                should_forward: false,
                n_times: BigInt::from(85),
                content: slice_44_with_ref_45(),
            }),
        }),
        &expect![
            "x{8980000000000000000000000000000000000000000000000000000000000000000004000000AA00000059_}\n x{0000002D}"
        ],
        "MsgExternal1",
        generated::store_msg_external1,
        generated::load_msg_external1,
    );
    run_alias(
        generated::UnionTy197::Variant0(generated::SayHiAndGoodbye {
            dest_addr: Some(masterchain.clone()),
            body: generated::UnionTy192::Variant1(generated::BodyPayload2 {
                master_id: BigInt::from(-5),
                owner_address: masterchain,
            }),
        }),
        &expect![
            "x{899FE51D0067B628CF959D5162B1E1EC3CD05BC0DD14B28EA099209E3EE07A57C9B3CFDCFF28E8033DB1467CACEA8B158F0F61E682DE06E8A5947504C904F1F703D2BE4D9E}"
        ],
        "MsgExternal1",
        generated::store_msg_external1,
        generated::load_msg_external1,
    );
    run_alias(
        generated::UnionTy197::Variant1(generated::SayStoreInChain {
            in_masterchain: true,
            contents: CellRef::new(generated::UnionTy192::Variant0(generated::BodyPayload1 {
                should_forward: true,
                n_times: BigInt::from(20),
                content: slice_44_with_ref_45(),
            })),
        }),
        &expect!["x{0013C_}\n x{3000000140000002C}\n  x{0000002D}"],
        "MsgExternal1",
        generated::store_msg_external1,
        generated::load_msg_external1,
    );
    let decoded = run_alias(
        generated::UnionTy197::Variant1(generated::SayStoreInChain {
            in_masterchain: false,
            contents: CellRef::new(generated::UnionTy192::Variant1(generated::BodyPayload2 {
                master_id: BigInt::from(37),
                owner_address: zero.clone(),
            })),
        }),
        &expect![
            "x{00134_}\n x{4960000000000000000000000000000000000000000000000000000000000000000004_}"
        ],
        "MsgExternal1",
        generated::store_msg_external1,
        generated::load_msg_external1,
    );

    let prefix_error = generated::BodyPayload1::from_cell(&cell_from_text("x{FF}", Vec::new()))
        .expect_err("incorrect body prefix must fail")
        .to_string();
    let owner_matches = matches!(
        decoded,
        generated::UnionTy197::Variant1(generated::SayStoreInChain { contents, .. })
            if matches!(
                contents.r#ref.as_ref(),
                generated::UnionTy192::Variant1(generated::BodyPayload2 { owner_address, .. })
                    if owner_address == &zero
            )
    );
    expect![[r#"
        (
            "Incorrect prefix for 'BodyPayload1': expected 0b001, got 0b111",
            true,
        )
    "#]]
    .assert_debug_eq(&(prefix_error, owner_matches));
}

#[test]
fn msg_transfer() {
    let masterchain = friendly_address("Ef8o6AM9sUZ8rOqLFY8PYeaC3gbopZR1BMkE8fcD0r5NnmCi");
    let zero = friendly_address("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c");

    run(
        generated::MsgTransfer {
            params: generated::UnionTy51::Variant0(generated::EitherLeft {
                value: generated::UnionTy175::Variant0(generated::TransferParams1 {
                    dest_int: masterchain,
                    amount: BigInt::from(80_000_000),
                    dest_ext: external_address(1234, 80),
                }),
            }),
        },
        &expect![
            "x{FB3701FF3CA4FF28E8033DB1467CACEA8B158F0F61E682DE06E8A5947504C904F1F703D2BE4D9E404C4B4004A0000000000000000009A5_}"
        ],
    );
    run(
        generated::MsgTransfer {
            params: generated::UnionTy51::Variant0(generated::EitherLeft {
                value: generated::UnionTy175::Variant1(generated::TransferParams2 {
                    int_vector: (
                        BigInt::from(123),
                        Some(BigInt::from(1_234_567_890_123_456_i64)),
                        BigInt::from(1_234_567_890_123_456_i64),
                    ),
                    needs_more: CellRef::new(true),
                }),
            }),
        },
        &expect!["x{FB3701FF48000003DDC118B54F22AEB0000118B54F22AEB02_}\n x{C_}"],
    );
    run(
        generated::MsgTransfer {
            params: generated::UnionTy51::Variant1(generated::EitherRight {
                value: CellRef::new(generated::UnionTy175::Variant0(
                    generated::TransferParams1 {
                        dest_int: zero,
                        amount: BigInt::from(80_000_000),
                        dest_ext: external_address(1234, 70),
                    },
                )),
            }),
        },
        &expect![
            "x{FB3701FFC_}\n x{7948000000000000000000000000000000000000000000000000000000000000000000809896800918000000000000004D2}"
        ],
    );
    let decoded = run(
        generated::MsgTransfer {
            params: generated::UnionTy51::Variant1(generated::EitherRight {
                value: CellRef::new(generated::UnionTy175::Variant1(
                    generated::TransferParams2 {
                        int_vector: (BigInt::from(123), None, BigInt::from(0)),
                        needs_more: CellRef::new(false),
                    },
                )),
            }),
        },
        &expect!["x{FB3701FFC_}\n x{90000007B00000000000000004_}\n  x{4_}"],
    );

    let is_right_params2 = matches!(
        decoded.params,
        generated::UnionTy51::Variant1(generated::EitherRight { value })
            if matches!(value.r#ref.as_ref(), generated::UnionTy175::Variant1(_))
    );
    expect![[r#"
        true
    "#]]
    .assert_debug_eq(&is_right_params2);
}

#[test]
fn union_8_16_32() {
    run_alias(
        generated::UnionTy199::Variant0(BigInt::from(15)),
        &expect!["x{03E_}"],
        "Union_8_16_32",
        generated::store_union_8_16_32,
        generated::load_union_8_16_32,
    );
    run_alias(
        generated::UnionTy199::Variant1(BigInt::from(15)),
        &expect!["x{4003E_}"],
        "Union_8_16_32",
        generated::store_union_8_16_32,
        generated::load_union_8_16_32,
    );
    run_alias(
        generated::UnionTy199::Variant2(BigInt::from(15)),
        &expect!["x{80000003E_}"],
        "Union_8_16_32",
        generated::store_union_8_16_32,
        generated::load_union_8_16_32,
    );
}

#[test]
fn union_8_16_32_n() {
    for (value, expected) in [
        (
            generated::UnionTy201::Variant0(BigInt::from(15)),
            expect!["x{81F_}"],
        ),
        (
            generated::UnionTy201::Variant1(BigInt::from(15)),
            expect!["x{A001F_}"],
        ),
        (
            generated::UnionTy201::Variant2(BigInt::from(15)),
            expect!["x{C0000001F_}"],
        ),
        (generated::UnionTy201::Variant3(()), expect!["x{4_}"]),
    ] {
        run_alias(
            value,
            &expected,
            "Union_8_16_32_n",
            generated::store_union_8_16_32_n,
            generated::load_union_8_16_32_n,
        );
    }
}

#[test]
fn union_structs_8_16_32() {
    run_alias(
        generated::UnionTy226::Variant0(BigInt::from(15)),
        &expect!["x{03E_}"],
        "UnionStructs_8_16_32",
        generated::store_union_structs_8_16_32,
        generated::load_union_structs_8_16_32,
    );
    run_alias(
        generated::UnionTy226::Variant1(generated::Test416 {
            a: BigInt::from(15),
        }),
        &expect!["x{4003E_}"],
        "UnionStructs_8_16_32",
        generated::store_union_structs_8_16_32,
        generated::load_union_structs_8_16_32,
    );
    run_alias(
        generated::UnionTy226::Variant2(generated::Test432 {
            a: BigInt::from(15),
        }),
        &expect!["x{80000003E_}"],
        "UnionStructs_8_16_32",
        generated::store_union_structs_8_16_32,
        generated::load_union_structs_8_16_32,
    );
    let error = load_alias(
        &cell_from_text("x{FF}", Vec::new()),
        generated::load_union_structs_8_16_32,
    )
    .expect_err("unknown union prefix must fail")
    .to_string();
    expect!["Incorrect prefix for 'UnionStructs_8_16_32': none of variants matched"]
        .assert_eq(&error);
}

#[test]
fn union_structs_8_16_32_n() {
    run_alias(
        generated::UnionTy228::Variant0(generated::Test48 {
            a: BigInt::from(15),
        }),
        &expect!["x{81F_}"],
        "UnionStructs_8_16_32_n",
        generated::store_union_structs_8_16_32_n,
        generated::load_union_structs_8_16_32_n,
    );
    run_alias(
        generated::UnionTy228::Variant1(BigInt::from(15)),
        &expect!["x{A001F_}"],
        "UnionStructs_8_16_32_n",
        generated::store_union_structs_8_16_32_n,
        generated::load_union_structs_8_16_32_n,
    );
    run_alias(
        generated::UnionTy228::Variant2(BigInt::from(15)),
        &expect!["x{C0000001F_}"],
        "UnionStructs_8_16_32_n",
        generated::store_union_structs_8_16_32_n,
        generated::load_union_structs_8_16_32_n,
    );
    run_alias(
        generated::UnionTy228::Variant3(()),
        &expect!["x{4_}"],
        "UnionStructs_8_16_32_n",
        generated::store_union_structs_8_16_32_n,
        generated::load_union_structs_8_16_32_n,
    );
}

#[test]
fn u105() {
    let error = load_alias(&cell_from_text("x{C}", Vec::new()), generated::load_u105)
        .expect_err("unknown union prefix must fail")
        .to_string();
    expect!["Incorrect prefix for 'U105': none of variants matched"].assert_eq(&error);
}

#[test]
fn u106() {
    let decoded = load_alias(
        &cell_from_text("x{4003E_}", Vec::new()),
        generated::load_u106,
    )
    .expect("int16 variant must decode");
    expect![[r#"
        Variant1(
            15,
        )
    "#]]
    .assert_debug_eq(&decoded);
}

#[test]
fn stor_with_str() {
    register_codecs();
    let original = generated::StorWithStr {
        a: BigInt::from(10),
        str: owned_slice("x{ABCD}"),
        b: BigInt::from(20),
    };
    let decoded = run(original.clone(), &expect!["x{0000000A02ABCD00000014}"]);
    let bits = decoded
        .str
        .as_slice()
        .expect("string slice must be readable")
        .size_bits();
    expect![[r#"
        (
            true,
            "x{ABCD}",
            16,
        )
    "#]]
    .assert_debug_eq(&(original.b == decoded.b, cell_tree(&decoded.str.cell), bits));
}

#[test]
fn telegram_string() {
    register_codecs();
    let empty = owned_slice("x{}");
    run_alias(
        empty.clone(),
        &expect!["x{00}"],
        "TelegramString",
        generated::store_telegram_string,
        generated::load_telegram_string,
    );

    let mut builder = CellBuilder::new();
    generated::store_telegram_string(&empty, &mut builder).expect("first string must encode");
    generated::store_telegram_string(&empty, &mut builder).expect("second string must encode");
    expect![[r#"
        16
    "#]]
    .assert_debug_eq(&builder.size_bits());
}

#[test]
fn point_with_custom_int() {
    register_codecs();
    let point = generated::PointWithCustomInt::from_cell(&cell_from_text("x{0102}", Vec::new()))
        .expect("point must decode");
    expect![[r#"
        PointWithCustomInt {
            a: 1,
            b: 2,
        }
    "#]]
    .assert_debug_eq(&point);
}

#[test]
fn with_my_border() {
    register_codecs();
    let values = [55, 8, -5].map(|b| {
        let value = generated::WithMyBorder {
            a: BigInt::from(0),
            b: BigInt::from(b),
        };
        let cell = value.to_cell().expect("bordered value must encode");
        generated::WithMyBorder::from_cell(&cell)
            .expect("bordered value must decode")
            .b
    });
    expect![[r#"
        [
            10,
            0,
            -1,
        ]
    "#]]
    .assert_debug_eq(&values);
}

#[test]
fn with_fake_writer() {
    register_codecs();
    let value = generated::WithFakeWriter {
        a: BigInt::from(10),
        fake: (),
        b: BigInt::from(20),
    };
    let cell = value.to_cell().expect("value must encode");
    let mut slice = cell.as_slice().expect("cell must be readable");
    let refs = slice.size_refs();
    slice.skip_first(8, 0).expect("first byte must exist");
    let fake = slice.load_uint(32).expect("fake value must exist");
    expect![[r#"
        (
            "x{0A0000007B14}\n x{}",
            1,
            123,
        )
    "#]]
    .assert_debug_eq(&(cell_tree(&cell), refs, fake));
}

#[test]
fn tensor3_skipping1() {
    register_codecs();
    let value = (BigInt::from(1), BigInt::from(2), BigInt::from(3));
    let mut builder = CellBuilder::new();
    generated::store_tensor3_skipping1(&value, &mut builder).expect("tensor must encode");
    let cell = builder.build().expect("tensor cell must build");
    let mut slice = cell.as_slice().expect("tensor cell must be readable");
    let decoded = generated::load_tensor3_skipping1(&mut slice).expect("tensor must decode");
    acton_client::cell::ensure_empty(&slice).expect("tensor slice must be exhausted");
    expect![[r#"
        (
            1,
            0,
            3,
        )
    "#]]
    .assert_debug_eq(&decoded);
}

#[test]
fn color() {
    register_codecs();
    let cell = generated::Color(BigInt::from(3))
        .to_cell()
        .expect("color must encode");
    expect!["x{1C_}"].assert_eq(&cell_tree(&cell));
    let decoded = generated::Color::from_cell(&cell).expect("color must decode");
    expect![[r#"
        (
            Color(
                3,
            ),
            false,
            false,
            false,
        )
    "#]]
    .assert_debug_eq(&(
        decoded.clone(),
        decoded == generated::Color::red(),
        decoded == generated::Color::green(),
        decoded == generated::Color::blue(),
    ));
}

#[test]
fn only_pack_and_only_unpack() {
    register_codecs();
    let value = BigInt::from(0x80_u8);
    let mut first = CellBuilder::new();
    generated::store_only_with_pack(&value, &mut first).expect("first value must encode");
    let first_bits = first.size_bits();
    let first_cell = first.build().expect("first cell must build");
    let dynamic_abi = new_dynamic_abi();
    let only_with_pack_idx = dynamic_abi
        .declaration_type_index("OnlyWithPack")
        .expect("OnlyWithPack alias must exist");
    let mut second = CellBuilder::new();
    dynamic_abi
        .pack_into_builder(
            only_with_pack_idx,
            &DynamicValue::Number(value.clone()),
            &mut second,
        )
        .expect("dynamic OnlyWithPack value must encode");
    let second_cell = second.build().expect("second cell must build");

    let mut first_slice = first_cell.as_slice().expect("first slice must be readable");
    let number = generated::load_only_with_unpack(&mut first_slice)
        .expect("standalone custom value must decode");
    let first_remaining = (first_slice.size_bits(), first_slice.size_refs());
    let mut struct_slice = first_cell
        .as_slice()
        .expect("struct slice must be readable");
    let wrapped = generated::HasOnlyWithUnpack::load_from(&mut struct_slice)
        .expect("wrapped custom value must decode");
    let struct_remaining = (struct_slice.size_bits(), struct_slice.size_refs());
    let mut second_slice = second_cell
        .as_slice()
        .expect("second slice must be readable");
    let has_only_with_unpack_idx = dynamic_abi
        .declaration_type_index("HasOnlyWithUnpack")
        .expect("HasOnlyWithUnpack struct must exist");
    let dynamic = dynamic_abi
        .unpack_from_slice(has_only_with_unpack_idx, &mut second_slice)
        .expect("dynamic wrapped custom value must decode");
    let dynamic_number = dynamic
        .field("wu")
        .expect("dynamic wrapper must contain wu");
    let DynamicValue::Number(dynamic_number) = dynamic_number else {
        panic!("dynamic wu must be a number");
    };
    let second_remaining = (second_slice.size_bits(), second_slice.size_refs());

    expect![[r#"
        (
            16,
            "x{FF80}",
            128,
            (
                0,
                0,
            ),
            128,
            (
                0,
                0,
            ),
            128,
            (
                0,
                0,
            ),
        )
    "#]]
    .assert_debug_eq(&(
        first_bits,
        cell_tree(&first_cell),
        number,
        first_remaining,
        wrapped.wu,
        struct_remaining,
        dynamic_number.clone(),
        second_remaining,
    ));
}

#[test]
fn e_fits_2_bits() {
    let zero = generated::EFits2Bits::from_cell(&cell_from_text("x{2_}", Vec::new()))
        .expect("zero enum must decode");
    let one = run(generated::EFits2Bits::one(), &expect!["x{6_}"]);
    expect![[r#"
        (
            EFits2Bits(
                0,
            ),
            true,
            true,
            true,
            EFits2Bits(
                1,
            ),
        )
    "#]]
    .assert_debug_eq(&(
        zero.clone(),
        zero == generated::EFits2Bits::zero(),
        zero != generated::EFits2Bits::one(),
        one == generated::EFits2Bits::one(),
        one,
    ));
}

#[test]
fn e_start_from_1() {
    let one_cell = generated::EStartFrom1::one()
        .to_cell()
        .expect("ONE must encode");
    let three_cell = generated::EStartFrom1::three()
        .to_cell()
        .expect("THREE must encode");
    let one = generated::EStartFrom1::from_cell(&one_cell).expect("ONE must decode");
    let three = generated::EStartFrom1::from_cell(&three_cell).expect("THREE must decode");
    expect![[r#"
        (
            EStartFrom1(
                1,
            ),
            EStartFrom1(
                3,
            ),
        )
    "#]]
    .assert_debug_eq(&(one, three));
}

#[test]
fn e_start_from_m2() {
    let m2 = generated::EStartFromM2::from_cell(&cell_from_text("x{D_}", Vec::new()))
        .expect("M2 must decode");
    let p3 = generated::EStartFromM2::from_cell(&cell_from_text("x{7_}", Vec::new()))
        .expect("P3 must decode");
    let encoded_m2 = generated::EStartFromM2::m2()
        .to_cell()
        .expect("M2 must encode");
    let encoded_p3 = generated::EStartFromM2::p3()
        .to_cell()
        .expect("P3 must encode");
    expect![[r#"
        (
            EStartFromM2(
                -2,
            ),
            EStartFromM2(
                3,
            ),
            "x{D_}",
            "x{7_}",
        )
    "#]]
    .assert_debug_eq(&(m2, p3, cell_tree(&encoded_m2), cell_tree(&encoded_p3)));
}

#[test]
fn e_fits_8_bits() {
    let zero_cell = generated::EFits8Bits::e0()
        .to_cell()
        .expect("E0 must encode");
    let arbitrary_cell = generated::EFits8Bits(BigInt::from(99))
        .to_cell()
        .expect("arbitrary enum value must encode");
    let zero = generated::EFits8Bits::from_cell(&zero_cell).expect("E0 must decode");
    let arbitrary = generated::EFits8Bits::from_cell(&arbitrary_cell)
        .expect("arbitrary enum value must decode");
    expect![[r#"
        (
            EFits8Bits(
                0,
            ),
            EFits8Bits(
                99,
            ),
        )
    "#]]
    .assert_debug_eq(&(zero, arbitrary));
}

#[test]
fn e_min_max() {
    let max_uint = (BigInt::from(1_u8) << 256_usize) - BigInt::from(1_u8);
    let min_int = -&max_uint - BigInt::from(1_u8);
    let build_signed_257 = |value: &BigInt| {
        let mut builder = CellBuilder::new();
        acton_client::cell::store_fixed_int(&mut builder, value, 257, true)
            .expect("257-bit signed boundary must encode");
        builder.build().expect("257-bit signed cell must build")
    };
    let min_decoded =
        generated::EMinMax::from_cell(&build_signed_257(&min_int)).expect("minimum must decode");
    let max_decoded =
        generated::EMinMax::from_cell(&build_signed_257(&max_uint)).expect("maximum must decode");
    expect![[r#"
        (
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&(
        min_decoded == generated::EMinMax::min_int(),
        max_decoded == generated::EMinMax::max_int(),
    ));
}

#[test]
fn e_0_max() {
    let zero = BigInt::from(0_u8);
    let max_uint = (BigInt::from(1_u8) << 256_usize) - BigInt::from(1_u8);
    let build_unsigned_256 = |value: &BigInt| {
        let mut builder = CellBuilder::new();
        acton_client::cell::store_fixed_int(&mut builder, value, 256, false)
            .expect("256-bit unsigned boundary must encode");
        builder.build().expect("256-bit unsigned cell must build")
    };
    let zero_decoded =
        generated::E0Max::from_cell(&build_unsigned_256(&zero)).expect("zero must decode");
    let max_decoded =
        generated::E0Max::from_cell(&build_unsigned_256(&max_uint)).expect("maximum must decode");
    expect![[r#"
        (
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&(
        zero_decoded == generated::E0Max::zero(),
        max_decoded == generated::E0Max::max_int(),
    ));
}

#[test]
fn with_enums_union() {
    let first = run(
        generated::WithEnumsUnion {
            u: generated::UnionTy224::Variant0(generated::EFits8Bits::e110()),
        },
        &expect!["x{9BA_}"],
    );
    run(
        generated::WithEnumsUnion {
            u: generated::UnionTy224::Variant1(generated::EStartFromM2::m2()),
        },
        &expect!["x{F4_}"],
    );
    run(
        generated::WithEnumsUnion {
            u: generated::UnionTy224::Variant2(()),
        },
        &expect!["x{4_}"],
    );

    let second_value = generated::WithEnumsUnion {
        u: generated::UnionTy224::Variant0(generated::EFits8Bits(BigInt::from(220))),
    };
    let second = generated::WithEnumsUnion::from_cell(
        &second_value.to_cell().expect("second value must encode"),
    )
    .expect("second value must decode");
    let third_value = generated::WithEnumsUnion {
        u: generated::UnionTy224::Variant1(generated::EStartFromM2::zero()),
    };
    let third = generated::WithEnumsUnion::from_cell(
        &third_value.to_cell().expect("third value must encode"),
    )
    .expect("third value must decode");
    let fourth_value = generated::WithEnumsUnion {
        u: generated::UnionTy224::Variant1(generated::EStartFromM2(BigInt::from(3))),
    };
    let fourth = generated::WithEnumsUnion::from_cell(
        &fourth_value.to_cell().expect("fourth value must encode"),
    )
    .expect("fourth value must decode");

    let checks = (
        matches!(&first.u, generated::UnionTy224::Variant0(value) if value == &generated::EFits8Bits::e110()),
        matches!(&second.u, generated::UnionTy224::Variant0(value) if value == &generated::EFits8Bits::e220()),
        matches!(&third.u, generated::UnionTy224::Variant1(value) if value == &generated::EStartFromM2::zero()),
        matches!(&fourth.u, generated::UnionTy224::Variant1(value) if value == &generated::EStartFromM2::p3()),
        matches!(&fourth.u, generated::UnionTy224::Variant0(value) if value == &generated::EFits8Bits::e220()),
    );
    expect![[r#"
        (
            true,
            true,
            true,
            true,
            false,
        )
    "#]]
    .assert_debug_eq(&checks);
}

#[test]
fn role() {
    let decoded = run_alias(
        (generated::Role::admin(), generated::Role::user()),
        &expect!["x{0001}"],
        "TwoRoles",
        generated::store_two_roles,
        generated::load_two_roles,
    );
    expect![[r#"
        (
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&(
        decoded.0 == generated::Role::admin(),
        decoded.1 == generated::Role::user(),
    ));
}

#[test]
fn encoded_vari() {
    let one = run(generated::EncodedVari::one(), &expect!["x{101}"]);
    let (one_with_trailing, one_bits, one_refs) =
        load_prefix::<generated::EncodedVari>(&cell_from_text("x{1018_}", Vec::new()))
            .expect("one with trailing data must decode");
    let (many, many_bits, many_refs) = load_prefix::<generated::EncodedVari>(&cell_from_text(
        "x{D100000000000000000000000008_}",
        Vec::new(),
    ))
    .expect("large enum value must decode");
    expect![[r#"
        (
            true,
            true,
            0,
            0,
            true,
            0,
            0,
        )
    "#]]
    .assert_debug_eq(&(
        one == generated::EncodedVari::one(),
        one_with_trailing == generated::EncodedVari::one(),
        one_bits,
        one_refs,
        many == generated::EncodedVari::many(),
        many_bits,
        many_refs,
    ));
}

#[test]
fn e_collision_names() {
    expect![[r#"
        (
            ECollisionNames(
                0,
            ),
            ECollisionNames(
                1,
            ),
            ECollisionNames(
                2,
            ),
            ECollisionNames(
                3,
            ),
        )
    "#]]
    .assert_debug_eq(&(
        generated::ECollisionNames::from_slice_(),
        generated::ECollisionNames::store_(),
        generated::ECollisionNames::to_cell_(),
        generated::ECollisionNames::to_cell__2(),
    ));
}

#[test]
fn with_maps0() {
    let address = friendly_address("kf-Dfdg-YQXaR2Q97gZJ4fGBtmV1DHOU1y1RPyyZZtRy_Ikh");
    let mut value = generated::WithMaps0 {
        m1: Dictionary::new(),
        m2: Dictionary::new(),
        m3: Dictionary::new(),
        m4: Dictionary::new(),
    };
    value.m1.insert(BigInt::from(1), BigInt::from(10));
    value.m1.insert(BigInt::from(2), BigInt::from(20));
    value
        .m2
        .insert(BigInt::from(-1), (BigInt::from(10), BigInt::from(10)));
    value
        .m2
        .insert(BigInt::from(-2), (BigInt::from(20), BigInt::from(20)));
    value.m3.insert(BigInt::from(65_535), AnyAddr::None);
    value
        .m3
        .insert(BigInt::from(999), AnyAddr::Std(address.clone()));
    value.m4.insert(address.clone(), owned_slice("x{01}"));

    let decoded = run(
        value,
        &expect![
            "x{F}\n x{CE}\n  x{50000000A}\n  x{400000014}\n x{EF}\n  x{05052_}\n  x{0282A_}\n x{2_}\n  x{BC1F3CFF837DD83E6105DA47643DEE0649E1F181B665750C7394D72D513F2C9966D472FC}\n  x{FE4_}\n x{A173FE0DF760F98417691D90F7B8192787C606D995D431CE535CB544FCB2659B51CBF006_}"
        ],
    );

    let m4_bits = dict_get(&decoded.m4, &address)
        .expect("address entry must exist")
        .as_slice()
        .expect("dictionary slice must be readable")
        .size_bits();
    expect![[r#"
        (
            Some(
                20,
            ),
            None,
            Some(
                (
                    20,
                    20,
                ),
            ),
            Some(
                None,
            ),
            8,
        )
    "#]]
    .assert_debug_eq(&(
        dict_get(&decoded.m1, &BigInt::from(2)),
        dict_get(&decoded.m1, &BigInt::from(3)),
        dict_get(&decoded.m2, &BigInt::from(-2)),
        dict_get(&decoded.m3, &BigInt::from(65_535)),
        m4_bits,
    ));
}

#[test]
fn with_nullable_maps() {
    let mut m2 = Dictionary::new();
    m2.insert(BigInt::from(1), BigInt::from(10));
    let mut m4 = Dictionary::new();
    m4.insert(BigInt::from(1), BigInt::from(10));
    let mut m5 = Dictionary::new();
    m5.insert(BigInt::from(1), BigInt::from(32_000));
    let value = generated::WithNullableMaps {
        m1: Some(Dictionary::new()),
        m2: Some(m2),
        m3: None,
        m4: generated::UnionTy256::Variant0(m4),
        m5: generated::UnionTy260::Variant1(m5),
    };
    let cell = value.to_cell().expect("nullable maps must encode");
    let mut slice = cell.as_slice().expect("cell must be readable");
    let before = (slice.size_bits(), slice.size_refs());
    let decoded =
        generated::WithNullableMaps::load_from(&mut slice).expect("nullable maps must decode");
    acton_client::cell::ensure_empty(&slice).expect("slice must be exhausted");

    let m4_is_map = matches!(decoded.m4, generated::UnionTy256::Variant0(_));
    let (m5_is_int16_map, m5_value) = match &decoded.m5 {
        generated::UnionTy260::Variant1(map) => (true, dict_get(map, &BigInt::from(1)).cloned()),
        _ => (false, None),
    };
    expect![[r#"
        (
            (
                10,
                3,
            ),
            true,
            0,
            1,
            true,
            true,
            true,
            Some(
                32000,
            ),
        )
    "#]]
    .assert_debug_eq(&(
        before,
        decoded.m1.is_some(),
        decoded.m1.as_ref().map_or(usize::MAX, Dictionary::len),
        decoded.m2.as_ref().map_or(usize::MAX, Dictionary::len),
        decoded.m3.is_none(),
        m4_is_map,
        m5_is_int16_map,
        m5_value,
    ));
}

#[test]
fn with_more_tricky_types() {
    let mut nested = Dictionary::new();
    nested.insert(BigInt::from(0), Dictionary::new());
    let mut at_nine = Dictionary::new();
    at_nine.insert(
        BigInt::from(30_000),
        generated::JustInt32 {
            value: BigInt::from(30_000),
        },
    );
    nested.insert(BigInt::from(9), at_nine);
    let first = generated::WithMoreTrickyTypes {
        uni1: generated::UnionTy264::Variant0((BigInt::from(1), BigInt::from(2))),
        r1: CellRef::new(generated::Wrapper {
            item: BigInt::from(123),
        }),
        r2: CellRef::new(generated::UnionTy269::Variant0(AnyAddr::None)),
        nullable1: Some(generated::CounterIncrement {
            counter_id: BigInt::from(8),
            inc_by: BigInt::from(5),
        }),
        nested_m: nested,
        uni2: generated::UnionTy275::Variant1(generated::Wrapper {
            item: BigInt::from(32_000),
        }),
    };
    let decoded = run(
        first,
        &expect![
            "x{008001448D159E020000000177D00}\n x{7B}\n x{1_}\n x{C9_}\n  x{DA_}\n  x{B3}\n   x{A0EA600000EA61_}"
        ],
    );

    let zero_size = dict_get(&decoded.nested_m, &BigInt::from(0)).map(Dictionary::len);
    let one_size = dict_get(&decoded.nested_m, &BigInt::from(1)).map(Dictionary::len);
    let nine = dict_get(&decoded.nested_m, &BigInt::from(9));
    let nine_size = nine.map(Dictionary::len);
    let nested_value = nine
        .and_then(|map| dict_get(map, &BigInt::from(30_000)))
        .map(|value| value.value.clone());
    expect![[r#"
        (
            Some(
                0,
            ),
            None,
            Some(
                1,
            ),
            Some(
                30000,
            ),
        )
    "#]]
    .assert_debug_eq(&(zero_size, one_size, nine_size, nested_value));

    run(
        generated::WithMoreTrickyTypes {
            uni1: generated::UnionTy264::Variant1((
                BigInt::from(1),
                BigInt::from(2),
                BigInt::from(3),
            )),
            r1: CellRef::new(generated::Wrapper {
                item: BigInt::from(-128),
            }),
            r2: CellRef::new(generated::UnionTy269::Variant1(generated::Wrapper {
                item: AnyAddr::None,
            })),
            nullable1: None,
            nested_m: Dictionary::new(),
            uni2: generated::UnionTy275::Variant0(generated::Wrapper {
                item: BigInt::from(-5),
            }),
        },
        &expect!["x{808001000000018FB}\n x{80}\n x{9_}"],
    );
}
