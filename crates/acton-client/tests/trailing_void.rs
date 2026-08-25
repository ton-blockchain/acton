use acton_client::__private::tycho_types::cell::{CellBuilder, CellSlice};
use acton_client::{AbiError, AbiLoad, AbiStore, Cell};
use expect_test::expect;
use num_bigint::BigInt;

#[acton_client::contract(abi = "tests/fixtures/trailing-void.abi.json")]
mod generated {}

fn cell_hex(cell: &Cell) -> String {
    format!(
        "x{{{:X}}}",
        cell.as_slice()
            .expect("cell must be readable")
            .display_data()
    )
}

fn round_trip<T>(value: &T) -> (String, T)
where
    T: AbiStore + AbiLoad,
{
    let cell = value.to_cell().expect("value must encode");
    let decoded = T::from_cell(&cell).expect("value must decode");
    (cell_hex(&cell), decoded)
}

fn round_trip_alias<T>(
    value: &T,
    store: fn(&T, &mut CellBuilder) -> Result<(), AbiError>,
    load: fn(&mut CellSlice<'_>) -> Result<T, AbiError>,
) -> (String, T) {
    let mut builder = CellBuilder::new();
    store(value, &mut builder).expect("value must encode");
    let cell = builder.build().expect("cell must build");
    let mut slice = cell.as_slice().expect("cell must be readable");
    let decoded = load(&mut slice).expect("value must decode");
    acton_client::cell::ensure_empty(&slice).expect("slice must be exhausted");
    (cell_hex(&cell), decoded)
}

#[test]
fn prefixed_or_void_matches_upstream() {
    let some = generated::UnionTy5::Variant0(generated::PrefixedSome {
        x: BigInt::from(0x1234_5678_u32),
    });
    let void = generated::UnionTy5::Variant1(());

    expect![[r#"
        (
            (
                "x{0112345678}",
                Variant0(
                    PrefixedSome {
                        x: 305419896,
                    },
                ),
            ),
            (
                "x{}",
                Variant1(
                    (),
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        round_trip_alias(
            &some,
            generated::store_prefixed_or_void,
            generated::load_prefixed_or_void,
        ),
        round_trip_alias(
            &void,
            generated::store_prefixed_or_void,
            generated::load_prefixed_or_void,
        ),
    ));
}

#[test]
fn plain_or_void_matches_upstream() {
    let some = generated::UnionTy8::Variant0(generated::PlainSome {
        x: BigInt::from(0x1234_5678_u32),
    });
    let void = generated::UnionTy8::Variant1(());

    expect![[r#"
        (
            (
                "x{12345678}",
                Variant0(
                    PlainSome {
                        x: 305419896,
                    },
                ),
            ),
            (
                "x{}",
                Variant1(
                    (),
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        round_trip_alias(
            &some,
            generated::store_plain_or_void,
            generated::load_plain_or_void,
        ),
        round_trip_alias(
            &void,
            generated::store_plain_or_void,
            generated::load_plain_or_void,
        ),
    ));
}

#[test]
fn int32_or_void_matches_upstream() {
    let some = generated::UnionTy10::Variant0(BigInt::from(i32::MAX));
    let void = generated::UnionTy10::Variant1(());

    expect![[r#"
        (
            (
                "x{7FFFFFFF}",
                Variant0(
                    2147483647,
                ),
            ),
            (
                "x{}",
                Variant1(
                    (),
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        round_trip_alias(
            &some,
            generated::store_int32_or_void,
            generated::load_int32_or_void,
        ),
        round_trip_alias(
            &void,
            generated::store_int32_or_void,
            generated::load_int32_or_void,
        ),
    ));
}

#[test]
fn three_way_with_void_matches_upstream() {
    let first = generated::UnionTy14::Variant0(generated::ThreeP1 { v: BigInt::from(1) });
    let second = generated::UnionTy14::Variant1(generated::ThreeP2 {
        v: BigInt::from(0x0102),
    });
    let void = generated::UnionTy14::Variant2(());

    expect![[r#"
        (
            (
                "x{006_}",
                Variant0(
                    ThreeP1 {
                        v: 1,
                    },
                ),
            ),
            (
                "x{4040A_}",
                Variant1(
                    ThreeP2 {
                        v: 258,
                    },
                ),
            ),
            (
                "x{}",
                Variant2(
                    (),
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        round_trip_alias(
            &first,
            generated::store_three_way_with_void,
            generated::load_three_way_with_void,
        ),
        round_trip_alias(
            &second,
            generated::store_three_way_with_void,
            generated::load_three_way_with_void,
        ),
        round_trip_alias(
            &void,
            generated::store_three_way_with_void,
            generated::load_three_way_with_void,
        ),
    ));
}

#[test]
fn inside_struct_with_void_matches_upstream() {
    let some = generated::InsideStructWithVoid {
        a: BigInt::from(7),
        tail: generated::UnionTy5::Variant0(generated::PrefixedSome {
            x: BigInt::from(100),
        }),
    };
    let void = generated::InsideStructWithVoid {
        a: BigInt::from(9),
        tail: generated::UnionTy5::Variant1(()),
    };

    expect![[r#"
        (
            (
                "x{070100000064}",
                InsideStructWithVoid {
                    a: 7,
                    tail: Variant0(
                        PrefixedSome {
                            x: 100,
                        },
                    ),
                },
            ),
            (
                "x{09}",
                InsideStructWithVoid {
                    a: 9,
                    tail: Variant1(
                        (),
                    ),
                },
            ),
        )
    "#]]
    .assert_debug_eq(&(round_trip(&some), round_trip(&void)));
}

#[test]
fn int32_or_null_or_void_matches_upstream() {
    let int = generated::UnionTy18::Variant0(BigInt::from(42));
    let null = generated::UnionTy18::Variant1(());
    let void = generated::UnionTy18::Variant2(());

    expect![[r#"
        (
            (
                "x{800000154_}",
                Variant0(
                    42,
                ),
            ),
            (
                "x{4_}",
                Variant1(
                    (),
                ),
            ),
            (
                "x{}",
                Variant2(
                    (),
                ),
            ),
        )
    "#]]
    .assert_debug_eq(&(
        round_trip_alias(
            &int,
            generated::store_int32_or_null_or_void,
            generated::load_int32_or_null_or_void,
        ),
        round_trip_alias(
            &null,
            generated::store_int32_or_null_or_void,
            generated::load_int32_or_null_or_void,
        ),
        round_trip_alias(
            &void,
            generated::store_int32_or_null_or_void,
            generated::load_int32_or_null_or_void,
        ),
    ));
}
