#![allow(clippy::needless_question_mark, clippy::needless_raw_string_hashes)]

use std::fmt::Write as _;
use std::sync::Arc;

use acton_client::__private::tycho_types::cell::{CellBuilder, CellSlice, DynCell};
use acton_client::__private::tycho_types::models::{StdAddr, StdAddrFormat};
use acton_client::{
    AbiError, AbiLoad, AbiStore, BigInt, BitString, Cell, CellRef, DynamicAbi, DynamicError,
    DynamicPackFn, DynamicUnpackFn, DynamicValue, OwnedSlice, register_custom_codec,
};
use expect_test::expect;

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-wrappers.abi.json")]
mod generated {}

fn int(value: i64) -> BigInt {
    BigInt::from(value)
}

fn uint_cell(value: u64, bits: u16) -> Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_uint(value, bits)
        .expect("integer must fit into the test cell");
    builder.build().expect("test cell must build")
}

fn cell_with_reference(reference: Cell) -> Cell {
    let mut builder = CellBuilder::new();
    builder
        .store_reference(reference)
        .expect("reference must fit into the test cell");
    builder.build().expect("test cell must build")
}

fn append_cell_tree(cell: &DynCell, depth: usize, output: &mut String) {
    if !output.is_empty() {
        output.push('\n');
    }
    for _ in 0..depth {
        output.push(' ');
    }
    let slice = cell.as_slice().expect("cell must be readable");
    write!(output, "x{{{:X}}}", slice.display_data()).expect("writing to a string cannot fail");
    for index in 0..cell.reference_count() {
        append_cell_tree(
            cell.reference(index).expect("cell reference must exist"),
            depth + 1,
            output,
        );
    }
}

fn cell_tree(cell: &Cell) -> String {
    let mut output = String::new();
    append_cell_tree(cell.as_ref(), 0, &mut output);
    output
}

fn round_trip<T>(value: &T) -> String
where
    T: AbiStore + AbiLoad + PartialEq,
{
    let mut cell = value.to_cell().expect("value must encode");
    let mut decoded = T::from_cell(&cell).expect("value must decode");
    let mut round_trip_matches = decoded == *value;
    for _ in 1..2 {
        cell = decoded.to_cell().expect("decoded value must encode");
        let next = T::from_cell(&cell).expect("value must decode again");
        round_trip_matches &= next == decoded;
        decoded = next;
    }
    let type_name = std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .expect("Rust type must have a name");
    let (dynamic_tree, dynamic_matches) = run_dynamic(type_name, &cell);
    format!(
        "{dynamic_tree}\nround_trip: {}",
        round_trip_matches && dynamic_matches
    )
}

fn round_trip_alias<T>(
    value: &T,
    alias_name: &str,
    store: fn(&T, &mut CellBuilder) -> Result<(), AbiError>,
    load: fn(&mut CellSlice<'_>) -> Result<T, AbiError>,
) -> String
where
    T: PartialEq,
{
    let mut builder = CellBuilder::new();
    store(value, &mut builder).expect("value must encode");
    let mut cell = builder.build().expect("cell must build");
    let mut slice = cell.as_slice().expect("cell must be readable");
    let mut decoded = load(&mut slice).expect("value must decode");
    acton_client::cell::ensure_empty(&slice).expect("slice must be exhausted");
    let mut round_trip_matches = decoded == *value;
    for _ in 1..2 {
        let mut builder = CellBuilder::new();
        store(&decoded, &mut builder).expect("decoded value must encode");
        cell = builder.build().expect("cell must build again");
        let mut slice = cell.as_slice().expect("cell must be readable");
        let next = load(&mut slice).expect("value must decode again");
        acton_client::cell::ensure_empty(&slice).expect("slice must be exhausted");
        round_trip_matches &= next == decoded;
        decoded = next;
    }
    let (dynamic_tree, dynamic_matches) = run_dynamic(alias_name, &cell);
    format!(
        "{dynamic_tree}\nround_trip: {}",
        round_trip_matches && dynamic_matches
    )
}

fn dynamic_input_error(field_path: &str, expected: &str, reason: &str) -> DynamicError {
    DynamicError::InvalidInput {
        field_path: field_path.to_owned(),
        expected: expected.to_owned(),
        reason: reason.to_owned(),
    }
}

fn dynamic_point_pack(
    type_name: &str,
    value: &DynamicValue,
    builder: &mut CellBuilder,
) -> Result<(), DynamicError> {
    let Some(DynamicValue::Number(x)) = value.field("x") else {
        return Err(dynamic_input_error(
            type_name,
            type_name,
            "field 'x' is not a number",
        ));
    };
    let Some(DynamicValue::Number(y)) = value.field("y") else {
        return Err(dynamic_input_error(
            type_name,
            type_name,
            "field 'y' is not a number",
        ));
    };
    acton_client::cell::store_fixed_int(builder, x, 8, false)?;
    acton_client::cell::store_fixed_int(builder, y, 8, false)?;
    Ok(())
}

fn dynamic_point_unpack(slice: &mut CellSlice<'_>) -> Result<DynamicValue, DynamicError> {
    Ok(DynamicValue::object([
        (
            "x",
            DynamicValue::Number(acton_client::cell::load_fixed_int(slice, 8, false)?),
        ),
        (
            "y",
            DynamicValue::Number(acton_client::cell::load_fixed_int(slice, 8, false)?),
        ),
    ]))
}

fn new_dynamic_abi() -> DynamicAbi {
    let mut abi = DynamicAbi::from_json(generated::ABI_JSON).expect("fixture ABI must parse");
    for type_name in ["CustomPoint", "CustomPointAlias"] {
        let pack_name = type_name.to_owned();
        abi.register_custom_codec(
            type_name,
            Some(
                Arc::new(move |value: &DynamicValue, builder: &mut CellBuilder| {
                    dynamic_point_pack(&pack_name, value, builder)
                }) as DynamicPackFn,
            ),
            Some(Arc::new(dynamic_point_unpack) as DynamicUnpackFn),
        )
        .expect("dynamic point codec must register");
    }
    abi
}

fn run_dynamic(type_name: &str, static_cell: &Cell) -> (String, bool) {
    let abi = new_dynamic_abi();
    let ty_idx = abi
        .declaration_type_index(type_name)
        .unwrap_or_else(|| panic!("dynamic ABI declaration `{type_name}` must exist"));
    let expected_tree = cell_tree(static_cell);
    let mut initial_slice = static_cell
        .as_slice()
        .expect("static cell must be readable");
    let mut value = abi
        .unpack_from_slice(ty_idx, &mut initial_slice)
        .expect("static cell must dynamically decode");
    acton_client::cell::ensure_empty(&initial_slice)
        .expect("initial dynamic slice must be exhausted");
    let mut dynamic_matches = true;
    let mut last_tree = String::new();
    for _ in 0..2 {
        let cell = abi
            .pack_to_cell(ty_idx, &value)
            .expect("dynamic value must encode");
        last_tree = cell_tree(&cell);
        dynamic_matches &= last_tree == expected_tree;
        let mut slice = cell.as_slice().expect("dynamic cell must be readable");
        value = abi
            .unpack_from_slice(ty_idx, &mut slice)
            .expect("dynamic value must decode");
        acton_client::cell::ensure_empty(&slice).expect("dynamic slice must be exhausted");
    }
    (last_tree, dynamic_matches)
}

fn dynamic_pack_error(
    abi: &DynamicAbi,
    ty_idx: usize,
    value: &DynamicValue,
    builder: &mut CellBuilder,
) -> String {
    abi.pack_into_builder(ty_idx, value, builder)
        .expect_err("invalid dynamic value must fail to encode")
        .to_string()
}

fn dynamic_unpack_error(abi: &DynamicAbi, ty_idx: usize, cell: &Cell) -> String {
    let mut slice = cell.as_slice().expect("invalid cell must be readable");
    abi.unpack_from_slice(ty_idx, &mut slice)
        .expect_err("invalid cell must fail to dynamically decode")
        .to_string()
}

fn load_error<T: AbiLoad>(cell: &Cell) -> String {
    match T::from_cell(cell) {
        Ok(_) => panic!("invalid cell must fail to decode"),
        Err(error) => error.to_string(),
    }
}

fn load_alias_error<T>(cell: &Cell, load: fn(&mut CellSlice<'_>) -> Result<T, AbiError>) -> String {
    let mut slice = cell.as_slice().expect("cell must be readable");
    match load(&mut slice) {
        Ok(_) => panic!("invalid cell must fail to decode"),
        Err(error) => error.to_string(),
    }
}

fn custom_point_store(
    point: &generated::CustomPoint,
    builder: &mut CellBuilder,
) -> Result<(), AbiError> {
    acton_client::cell::store_fixed_int(builder, &point.x, 8, false)?;
    acton_client::cell::store_fixed_int(builder, &point.y, 8, false)
}

fn custom_point_load(slice: &mut CellSlice<'_>) -> Result<generated::CustomPoint, AbiError> {
    Ok(generated::CustomPoint {
        x: acton_client::cell::load_fixed_int(slice, 8, false)?,
        y: acton_client::cell::load_fixed_int(slice, 8, false)?,
    })
}

#[test]
fn array_of_lots_coins() {
    let zeroes = (
        int(0),
        int(0),
        int(0),
        int(0),
        int(0),
        int(0),
        int(0),
        int(0),
        int(0),
        int(0),
    );
    let ten_ton = BigInt::from(10_000_000_000_u64);
    let tens = (
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton.clone(),
        ten_ton,
    );
    let arr = vec![
        generated::LotsOfCoins { nums: zeroes },
        generated::LotsOfCoins { nums: tens },
        generated::LotsOfCoins {
            nums: (
                int(0),
                int(0),
                int(0),
                int(0),
                int(0),
                int(0),
                int(0),
                int(0),
                int(0),
                BigInt::from(50_000_000_u64),
            ),
        },
    ];

    expect![[r#"
x{03C_}
 x{80000000004_}
  x{A812A05F2002812A05F2002812A05F2002812A05F2002812A05F2002812A05F2002812A05F2002812A05F2002812A05F2002812A05F2004_}
   x{0000000002017D78404_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &arr,
        "ArrayOfLotsCoins",
        generated::store_array_of_lots_coins,
        generated::load_array_of_lots_coins,
    ));
}

#[test]
fn with_arrays() {
    let first = generated::WithArrays::create(
        vec![int(1)],
        vec![CellRef::new(int(1))],
        vec![OwnedSlice::full(uint_cell(1, 32))],
    );
    let second = generated::WithArrays::create(
        vec![],
        vec![],
        vec![OwnedSlice::full(cell_with_reference(Cell::default()))],
    );

    expect![[r#"
x{0180C07}
 x{00C_}
 x{4_}
  x{01}
 x{00000000C_}
round_trip: true"#]]
    .assert_eq(&round_trip(&first));
    expect![[r#"
x{0000007}
 x{4_}
  x{}
round_trip: true"#]]
    .assert_eq(&round_trip(&second));
}

#[test]
fn with_arrays4() {
    let (address, _) = StdAddr::from_str_ext(
        "EQCIJLNFIko5CvpKn9oAkrDgLocDOoD4vwmHxNx_fsG_LkwW",
        StdAddrFormat::any(),
    )
    .expect("upstream address must parse");
    let default_point = generated::Point::create;
    let first = generated::WithArrays4 {
        before: None,
        a0: vec![
            default_point(),
            generated::Point {
                x: int(100),
                y: int(-100),
            },
        ],
        a1: vec![generated::WithRef {
            id: None,
            r#ref: Some(CellRef::new(address)),
        }],
        a2: None,
        after: int(100),
    };
    let second = generated::WithArrays4 {
        before: Some(int(8)),
        a0: vec![],
        a1: vec![
            generated::WithRef {
                id: Some(int(70)),
                r#ref: None,
            },
            generated::WithRef {
                id: None,
                r#ref: None,
            },
        ],
        a2: Some(vec![Some(true), Some(false)]),
        after: int(2000),
    };
    let third = generated::WithArrays4 {
        before: None,
        a0: vec![
            default_point(),
            default_point(),
            default_point(),
            default_point(),
            default_point(),
        ],
        a1: vec![],
        a2: Some(vec![]),
        after: int(9),
    };

    expect![[r#"
x{0140600000064}
 x{850A4_}
  x{324E4_}
 x{3_}
  x{8011049668A44947215F4953FB4012561C05D0E067501F17E130F89B8FEFD837E5D_}
round_trip: true"#]]
    .assert_eq(&round_trip(&first));
    expect![[r#"
x{800000040000B02800003E84_}
 x{C00000119_}
  x{1_}
 x{F_}
  x{5_}
round_trip: true"#]]
    .assert_eq(&round_trip(&second));
    expect![[r#"
x{02C010000000004C_}
 x{850A4_}
  x{850A4_}
   x{850A4_}
    x{850A4_}
     x{050A4_}
round_trip: true"#]]
    .assert_eq(&round_trip(&third));
}

#[test]
fn my_shape1() {
    let first: generated::MyShape1 = (
        Some(int(100)),
        generated::Point::create(),
        (),
        generated::EmptyE {},
        None,
        int(8),
    );
    let second: generated::MyShape1 = (
        None,
        generated::Point {
            x: int(-5),
            y: int(-15),
        },
        (),
        generated::EmptyE {},
        None,
        int(-8),
    );

    expect![[r#"
x{80000032050A091A022_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &first,
        "MyShape1",
        generated::store_my_shape1,
        generated::load_my_shape1,
    ));
    expect![[r#"
x{7DF8891A3E2_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &second,
        "MyShape1",
        generated::store_my_shape1,
        generated::load_my_shape1,
    ));
}

#[test]
fn recurrent() {
    let value = generated::Recurrent {
        value: int(100),
        next_sh: (
            int(1),
            Some(CellRef::new(generated::Recurrent {
                value: int(200),
                next_sh: (
                    int(2),
                    Some(CellRef::new(generated::Recurrent {
                        value: int(300),
                        next_sh: (int(3), None),
                    })),
                ),
            })),
        ),
    };
    let cell = value.to_cell().expect("recurrent value must encode");
    let mut prefix = cell.as_slice().expect("cell must be readable");

    expect![[r#"
(
    21,
    3203,
    "x{00641C_}\n x{00C82C_}\n  x{012C34_}\nround_trip: true",
)
"#]]
    .assert_debug_eq(&(
        prefix.size_bits(),
        prefix.load_uint(21).expect("prefix must decode"),
        round_trip(&value),
    ));
}

#[test]
fn with_lisp_lists1() {
    let value = generated::WithLispLists1 {
        l_int8_1: vec![int(1), int(2), int(3)],
        l_int8_2: vec![],
        l_point_1: vec![
            generated::Point::create(),
            generated::Point {
                x: int(-60),
                y: int(60),
            },
        ],
        l_point_2: vec![],
    };

    expect![[r#"
x{}
 x{03}
  x{02}
   x{01}
    x{}
 x{}
 x{C43C}
  x{0A14}
   x{}
 x{}
round_trip: true"#]]
    .assert_eq(&round_trip(&value));
}

#[test]
fn with_lisp_lists2() {
    let value = generated::WithLispLists2 {
        before: int(10),
        l_int8: vec![int(1), int(2), int(3)],
        l_int32: vec![int(1), int(2), int(3)],
        l_int256: vec![int(1), int(2), int(3)],
        after: int(20),
    };

    expect![[r#"
x{0A14}
 x{03}
  x{02}
   x{01}
    x{}
 x{00000003}
  x{00000002}
   x{00000001}
    x{}
 x{0000000000000000000000000000000000000000000000000000000000000003}
  x{0000000000000000000000000000000000000000000000000000000000000002}
   x{0000000000000000000000000000000000000000000000000000000000000001}
    x{}
round_trip: true"#]]
    .assert_eq(&round_trip(&value));
}

#[test]
fn custom_point() {
    register_custom_codec::<generated::CustomPoint>(
        "CustomPoint",
        Some(custom_point_store),
        Some(custom_point_load),
    )
    .expect("CustomPoint codec must register");
    register_custom_codec::<generated::CustomPoint>(
        "CustomPointAlias",
        Some(custom_point_store),
        Some(custom_point_load),
    )
    .expect("CustomPointAlias codec must register");
    let first = generated::CustomPoint {
        x: int(10),
        y: int(20),
    };
    let second: generated::CustomPointAlias = generated::CustomPoint {
        x: int(100),
        y: int(200),
    };

    expect![[r#"
x{0A14}
round_trip: true"#]]
    .assert_eq(&round_trip(&first));
    expect![[r#"
x{64C8}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &second,
        "CustomPointAlias",
        generated::store_custom_point_alias,
        generated::load_custom_point_alias,
    ));
}

#[test]
fn with_strings1() {
    let first = generated::WithStrings1 {
        s1: "hello".to_owned(),
        s2: Some("auto".to_owned()),
    };
    let second = generated::WithStrings1 {
        s1: String::new(),
        s2: None,
    };

    expect![[r#"
x{C_}
 x{68656C6C6F}
 x{6175746F}
round_trip: true"#]]
    .assert_eq(&round_trip(&first));
    expect![[r#"
x{4_}
 x{}
round_trip: true"#]]
    .assert_eq(&round_trip(&second));
}

#[test]
fn with_strings2() {
    let first = generated::WithStrings2 {
        r#ref: CellRef::new("o1".to_owned()),
        refo: Some(CellRef::new(Some("oo".to_owned()))),
        arrs: vec!["1".to_owned(), "2".to_owned()],
    };
    let second = generated::WithStrings2 {
        r#ref: CellRef::new("o1".to_owned()),
        refo: Some(CellRef::new(None)),
        arrs: vec![],
    };

    expect![[r#"
x{816_}
 x{}
  x{6F31}
 x{C_}
  x{6F6F}
 x{C_}
  x{4_}
   x{32}
  x{31}
round_trip: true"#]]
    .assert_eq(&round_trip(&first));
    expect![[r#"
x{802_}
 x{}
  x{6F31}
 x{4_}
round_trip: true"#]]
    .assert_eq(&round_trip(&second));

    const LONG: &str = "12345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789012345678";
    let third = generated::WithStrings2 {
        r#ref: CellRef::new(LONG.to_owned()),
        refo: None,
        arrs: vec![LONG.to_owned(), format!("{LONG}{LONG}")],
    };
    let cell = third.to_cell().expect("long strings must encode");
    let mut root = cell.as_slice().expect("root must be readable");
    let mut string_ref = root
        .load_reference_as_slice()
        .expect("ref wrapper must exist");
    let mut string_head = string_ref
        .load_reference_as_slice()
        .expect("string head must exist");
    let remaining_refs = string_head.size_refs();
    let remaining_bits = string_head.size_bits();
    let tail_bits = string_head
        .load_reference_as_slice()
        .expect("string tail must exist")
        .size_bits();
    let decoded = generated::WithStrings2::from_cell(&cell).expect("long strings must decode");

    expect![[r#"
(
    1,
    1016,
    8,
    true,
)
"#]]
    .assert_debug_eq(&(
        remaining_refs,
        remaining_bits,
        tail_bits,
        decoded.arrs == third.arrs,
    ));
}

#[test]
fn prefixed_or_void() {
    let some = generated::UnionTy352::Variant0(generated::PrefixedSome {
        x: BigInt::from(0x1234_5678_u32),
    });
    let void = generated::UnionTy352::Variant1(());

    expect![[r#"
x{0112345678}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &some,
        "PrefixedOrVoid",
        generated::store_prefixed_or_void,
        generated::load_prefixed_or_void,
    ));
    expect![[r#"
x{}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &void,
        "PrefixedOrVoid",
        generated::store_prefixed_or_void,
        generated::load_prefixed_or_void,
    ));
}

#[test]
fn plain_or_void() {
    let some = generated::UnionTy355::Variant0(generated::PlainSome {
        x: BigInt::from(0x1234_5678_u32),
    });
    let void = generated::UnionTy355::Variant1(());

    expect![[r#"
x{12345678}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &some,
        "PlainOrVoid",
        generated::store_plain_or_void,
        generated::load_plain_or_void,
    ));
    expect![[r#"
x{}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &void,
        "PlainOrVoid",
        generated::store_plain_or_void,
        generated::load_plain_or_void,
    ));
}

#[test]
fn int32_or_void() {
    let some = generated::UnionTy357::Variant0(BigInt::from(i32::MAX));
    let void = generated::UnionTy357::Variant1(());

    expect![[r#"
x{7FFFFFFF}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &some,
        "Int32OrVoid",
        generated::store_int32_or_void,
        generated::load_int32_or_void,
    ));
    expect![[r#"
x{}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &void,
        "Int32OrVoid",
        generated::store_int32_or_void,
        generated::load_int32_or_void,
    ));
}

#[test]
fn three_way_with_void() {
    let first = generated::UnionTy361::Variant0(generated::ThreeP1 { v: int(1) });
    let second = generated::UnionTy361::Variant1(generated::ThreeP2 { v: int(0x0102) });
    let void = generated::UnionTy361::Variant2(());

    expect![[r#"
x{006_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &first,
        "ThreeWayWithVoid",
        generated::store_three_way_with_void,
        generated::load_three_way_with_void,
    ));
    expect![[r#"
x{4040A_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &second,
        "ThreeWayWithVoid",
        generated::store_three_way_with_void,
        generated::load_three_way_with_void,
    ));
    expect![[r#"
x{}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &void,
        "ThreeWayWithVoid",
        generated::store_three_way_with_void,
        generated::load_three_way_with_void,
    ));
    expect![[r#"Incorrect prefix for 'ThreeWayWithVoid': none of variants matched"#]].assert_eq(
        &load_alias_error(&uint_cell(0xff, 8), generated::load_three_way_with_void),
    );
}

#[test]
fn inside_struct_with_void() {
    let some = generated::InsideStructWithVoid {
        a: int(7),
        tail: generated::UnionTy352::Variant0(generated::PrefixedSome { x: int(100) }),
    };
    let void = generated::InsideStructWithVoid {
        a: int(9),
        tail: generated::UnionTy352::Variant1(()),
    };

    expect![[r#"
x{070100000064}
round_trip: true"#]]
    .assert_eq(&round_trip(&some));
    expect![[r#"
x{09}
round_trip: true"#]]
    .assert_eq(&round_trip(&void));
}

#[test]
fn int32_or_null_or_void() {
    let integer = generated::UnionTy364::Variant0(int(42));
    let null = generated::UnionTy364::Variant1(());
    let void = generated::UnionTy364::Variant2(());

    expect![[r#"
x{800000154_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &integer,
        "Int32OrNullOrVoid",
        generated::store_int32_or_null_or_void,
        generated::load_int32_or_null_or_void,
    ));
    expect![[r#"
x{4_}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &null,
        "Int32OrNullOrVoid",
        generated::store_int32_or_null_or_void,
        generated::load_int32_or_null_or_void,
    ));
    expect![[r#"
x{}
round_trip: true"#]]
    .assert_eq(&round_trip_alias(
        &void,
        "Int32OrNullOrVoid",
        generated::store_int32_or_null_or_void,
        generated::load_int32_or_null_or_void,
    ));
}

#[test]
fn invalid_js_naming() {
    let value = generated::InvalidJsNaming {
        foo_bar: int(100),
        rule: int(200),
        a_b: int(300),
        void: (),
    };
    let decoded = generated::InvalidJsNaming::from_cell(
        &value.to_cell().expect("renamed fields must encode"),
    )
    .expect("renamed fields must decode");

    expect![[r#"
(
    100,
    200,
    300,
)
"#]]
    .assert_debug_eq(&(decoded.foo_bar, decoded.rule, decoded.a_b));
}

#[test]
fn not_packable() {
    let empty = Cell::default();
    let two_bytes = uint_cell(0x0102, 16);
    let not_packable_load = load_error::<generated::NotPackable1>(&empty);
    let not_packable_store = generated::NotPackable1 {
        n: int(0),
        a: (int(0), int(0)),
    }
    .to_cell()
    .expect_err("unbounded int must not encode")
    .to_string();

    let not_unpackable_load = load_error::<generated::NotUnpackable1>(&empty);
    let mut nested_builder = CellBuilder::new();
    nested_builder
        .store_uint(0, 32)
        .expect("test builder value must fit");
    let not_unpackable_bits = generated::NotUnpackable1 {
        e: (int(0), (int(1), nested_builder)),
    }
    .to_cell()
    .expect("builder field must encode")
    .bit_len();

    let with_not_packable = load_error::<generated::WithNotPackable>(&two_bytes);
    let alias3 = load_alias_error(&empty, generated::load_not_packable3_alias);
    let mut alias4_builder = CellBuilder::new();
    let alias4 =
        generated::store_not_packable4_alias(&(int(0), int(0), int(0)), &mut alias4_builder)
            .expect_err("unbounded alias field must not encode")
            .to_string();
    let packable5 = load_error::<generated::NotPackable5>(&empty);
    let packable6 = load_error::<generated::NotPackable6>(&empty);
    let packable7 = load_error::<generated::NotPackable7>(&empty);
    let packable8 = load_error::<generated::NotPackable8>(&empty);

    expect![[r#"
(
    "Can't unpack 'NotPackable1' from cell, because 'NotPackable1.a[1]' is 'int'",
    "Can't pack 'NotPackable1' to cell, because 'self.a[1]' is 'int'",
    "Can't unpack 'NotUnpackable1' from cell, because 'NotUnpackable1.e[1][1]' is 'builder'",
    48,
    "Can't unpack 'tuple' from cell, because 'tuple[ith]' is 'unknown'",
    "Can't unpack 'tuple' from cell, because 'tuple[ith]' is 'unknown'",
    "Can't pack 'NotPackable4Alias' to cell, because 'self[2]' is 'int'",
    "Can't unpack 'NotPackable5' from cell, because 'NotPackable5.t[1]' is 'builder'",
    "Can't unpack 'NotPackable6' from cell, because 'NotPackable6.c' is 'continuation'",
    "Can't unpack 'NotPackable7' from cell, because 'NotPackable7.f' is 'continuation'",
    "Can't unpack 'NotPackable8' from cell, because 'NotPackable8.u' is 'unknown'",
)
"#]]
    .assert_debug_eq(&(
        not_packable_load,
        not_packable_store,
        not_unpackable_load,
        not_unpackable_bits,
        with_not_packable,
        alias3,
        alias4,
        packable5,
        packable6,
        packable7,
        packable8,
    ));
}

#[test]
fn errors_at_dynamic_serialization() {
    let abi = new_dynamic_abi();
    let ty_idx = |name: &str| {
        abi.declaration_type_index(name)
            .unwrap_or_else(|| panic!("dynamic ABI declaration `{name}` must exist"))
    };
    let just_int32 = ty_idx("JustInt32");
    let just_address = ty_idx("JustAddress");
    let not_packable5 = ty_idx("NotPackable5");
    let some_bytes_fields = ty_idx("SomeBytesFields");
    let int_and_union = ty_idx("IntAndEitherInt8Or256");
    let color = ty_idx("Color");
    // `Test4_8.a` in the upstream ABI points at unique type 27 (`int8`).
    let int8_ty_idx = 27;
    let mut builder = CellBuilder::new();
    let errors = vec![
        dynamic_pack_error(&abi, just_int32, &DynamicValue::Null, &mut builder),
        dynamic_pack_error(
            &abi,
            just_int32,
            &DynamicValue::object(Vec::<(&str, DynamicValue)>::new()),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            just_int32,
            &DynamicValue::object([("value", DynamicValue::String("123".to_owned()))]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            just_int32,
            &DynamicValue::object([("value", DynamicValue::Number(BigInt::from(1_u64 << 40)))]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            just_address,
            &DynamicValue::object([("addr", DynamicValue::Number(int(123)))]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            not_packable5,
            &DynamicValue::object([("t", DynamicValue::Number(int(1)))]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            some_bytes_fields,
            &DynamicValue::object([(
                "f1",
                DynamicValue::Array(vec![DynamicValue::Number(int(123))]),
            )]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            some_bytes_fields,
            &DynamicValue::object([(
                "f1",
                DynamicValue::object([
                    ("remainingBits", DynamicValue::Number(int(8))),
                    ("remainingRefs", DynamicValue::Number(int(0))),
                ]),
            )]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            some_bytes_fields,
            &DynamicValue::object([(
                "f1",
                DynamicValue::Bits(BitString(OwnedSlice::full(Cell::default()))),
            )]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            int_and_union,
            &DynamicValue::object([("op", DynamicValue::Number(int(1)))]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            int_and_union,
            &DynamicValue::object([
                ("op", DynamicValue::Number(int(1))),
                ("i8or256", DynamicValue::Number(int(123))),
            ]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            int_and_union,
            &DynamicValue::object([
                ("op", DynamicValue::Number(int(1))),
                (
                    "i8or256",
                    DynamicValue::object([("$", DynamicValue::String("asdf".to_owned()))]),
                ),
            ]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            int_and_union,
            &DynamicValue::object([
                ("op", DynamicValue::Number(int(1))),
                (
                    "i8or256",
                    DynamicValue::object([("$", DynamicValue::String("int8".to_owned()))]),
                ),
            ]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            int_and_union,
            &DynamicValue::object([
                ("op", DynamicValue::Number(int(1))),
                (
                    "i8or256",
                    DynamicValue::object([
                        ("$", DynamicValue::String("int8".to_owned())),
                        ("another", DynamicValue::Number(int(123))),
                    ]),
                ),
            ]),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            color,
            &DynamicValue::String("asdf".to_owned()),
            &mut builder,
        ),
        dynamic_pack_error(
            &abi,
            int8_ty_idx,
            &DynamicValue::Number(BigInt::from(100_500_u64)),
            &mut builder,
        ),
    ];

    expect![[r#"
[
    "invalid value passed for 'JustInt32' of type 'JustInt32': not an object",
    "invalid value passed for 'JustInt32.value' of type 'int32': not a number",
    "invalid value passed for 'JustInt32.value' of type 'int32': not a number",
    "invalid value passed for 'JustInt32.value' of type 'int32': value is out of range for 32 bits. Got 1099511627776",
    "invalid value passed for 'JustAddress.addr' of type 'address': not an address",
    "invalid value passed for 'NotPackable5.t' of type '[int8, builder]': not an array",
    "invalid value passed for 'SomeBytesFields.f1' of type 'bits8': not a bit slice",
    "invalid value passed for 'SomeBytesFields.f1' of type 'bits8': not a bit slice",
    "invalid value passed for 'SomeBytesFields.f1' of type 'bits8': expected 8 bits and 0 refs, got 0 bits and 0 refs",
    "invalid value passed for 'IntAndEitherInt8Or256.i8or256' of type 'int8 | int256': not an object with property $",
    "invalid value passed for 'IntAndEitherInt8Or256.i8or256' of type 'int8 | int256': not an object with property $",
    "invalid value passed for 'IntAndEitherInt8Or256.i8or256' of type 'int8 | int256': non-existing union variant for $ = 'asdf'",
    "invalid value passed for 'IntAndEitherInt8Or256.i8or256' of type 'int8 | int256': expected {$,value} but field 'value' not provided",
    "invalid value passed for 'IntAndEitherInt8Or256.i8or256' of type 'int8 | int256': expected {$,value} but field 'value' not provided",
    "invalid value passed for 'self' of type 'Color': not a number",
    "value is out of range for 8 bits. Got 100500",
]
"#]]
    .assert_debug_eq(&errors);
}

#[test]
fn errors_at_dynamic_deserialization() {
    let abi = new_dynamic_abi();
    let empty = Cell::default();
    let int_and_union = uint_cell(0, 32);
    let just_int32_ty_idx = abi
        .declaration_type_index("JustInt32")
        .expect("JustInt32 must exist");
    let int_and_union_ty_idx = abi
        .declaration_type_index("IntAndEitherInt8Or256")
        .expect("IntAndEitherInt8Or256 must exist");

    expect![[r#"
[
    "cannot deserialize 'JustInt32.value' dynamically: cell underflow",
    "cannot deserialize 'IntAndEitherInt8Or256.i8or256' dynamically: none of union prefixes match",
]
"#]]
    .assert_debug_eq(&vec![
        dynamic_unpack_error(&abi, just_int32_ty_idx, &empty),
        dynamic_unpack_error(&abi, int_and_union_ty_idx, &int_and_union),
    ]);
}
