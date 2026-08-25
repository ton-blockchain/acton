use std::fmt::Debug;

use acton_client::__private::tycho_types::cell::{CellBuilder, DynCell};
use acton_client::__private::tycho_types::models::{AnyAddr, ExtAddr, StdAddr, StdAddrFormat};
use acton_client::{AbiLoad, AbiStore, BitString, Cell, CellRef, OwnedSlice};
use expect_test::expect;
use num_bigint::{BigInt, BigUint};

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-wrappers.abi.json")]
mod generated {}

fn bi(value: i64) -> BigInt {
    BigInt::from(value)
}

const fn maybe_nothing<T>() -> generated::UnionTy59<T> {
    generated::UnionTy59::Variant0(generated::MaybeNothing {})
}

const fn maybe_just<T>(value: T) -> generated::UnionTy59<T> {
    generated::UnionTy59::Variant1(generated::MaybeJust { value })
}

fn std_addr(value: &str) -> StdAddr {
    if value.contains(':') {
        value.parse().expect("raw address must parse")
    } else {
        StdAddr::from_str_ext(value, StdAddrFormat::any())
            .expect("user-friendly address must parse")
            .0
    }
}

fn any_std(value: &str) -> AnyAddr {
    AnyAddr::Std(std_addr(value))
}

fn any_ext(value: u64, bits: u16) -> AnyAddr {
    let padding = usize::from((8 - bits % 8) % 8);
    let mut encoded = (BigUint::from(value) << padding).to_bytes_be();
    let byte_len = usize::from(bits.div_ceil(8));
    if encoded.len() < byte_len {
        let mut padded = vec![0; byte_len - encoded.len()];
        padded.extend(encoded);
        encoded = padded;
    }
    AnyAddr::Ext(ExtAddr::new(bits, encoded).expect("external address must fit"))
}

fn bits_from_notation(notation: &str) -> Vec<bool> {
    if let Some(binary) = notation
        .strip_prefix("b{")
        .and_then(|value| value.strip_suffix('}'))
    {
        return binary
            .bytes()
            .map(|byte| match byte {
                b'0' => false,
                b'1' => true,
                _ => panic!("invalid binary cell notation"),
            })
            .collect();
    }

    let hex = notation
        .strip_prefix("x{")
        .and_then(|value| value.strip_suffix('}'))
        .expect("cell notation must use x{...} or b{...}");
    let tagged = hex.ends_with('_');
    let hex = hex.strip_suffix('_').unwrap_or(hex);
    let mut bits = Vec::with_capacity(hex.len() * 4);
    for digit in hex.bytes() {
        let digit = (digit as char).to_digit(16).expect("invalid hex digit") as u8;
        for shift in (0..4).rev() {
            bits.push((digit >> shift) & 1 != 0);
        }
    }
    if tagged {
        while bits.last() == Some(&false) {
            bits.pop();
        }
        match bits.pop() {
            Some(true) => {}
            _ => panic!("tagged notation needs a top-up bit"),
        }
    }
    bits
}

fn make_cell(notation: &str, refs: Vec<Cell>) -> Cell {
    let bits = bits_from_notation(notation);
    let mut bytes = vec![0_u8; bits.len().div_ceil(8)];
    for (index, bit) in bits.iter().copied().enumerate() {
        if bit {
            bytes[index / 8] |= 1 << (7 - index % 8);
        }
    }
    let mut builder = CellBuilder::new();
    builder
        .store_raw(&bytes, bits.len() as u16)
        .expect("cell bits must fit");
    for reference in refs {
        builder
            .store_reference(reference)
            .expect("cell reference must fit");
    }
    builder.build().expect("cell must build")
}

fn owned(notation: &str, refs: Vec<Cell>) -> OwnedSlice {
    OwnedSlice::full(make_cell(notation, refs))
}

fn bit_string(notation: &str) -> BitString {
    BitString(owned(notation, vec![]))
}

fn cell_tree(cell: &Cell) -> String {
    fn visit(cell: &DynCell, level: usize, out: &mut String) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&" ".repeat(level));
        out.push_str("x{");
        out.push_str(&cell.display_data().to_string().to_ascii_uppercase());
        out.push('}');
        for child in cell.references() {
            visit(child, level + 1, out);
        }
    }

    let mut output = String::new();
    visit(cell.as_ref(), 0, &mut output);
    output
}

fn run<T>(mut value: T) -> (Vec<String>, T)
where
    T: AbiStore + AbiLoad + Debug,
{
    let mut cells = Vec::with_capacity(4);
    let mut last_typed_cell = None;
    for _ in 0..2 {
        let cell = value.to_cell().expect("value must encode");
        cells.push(cell_tree(&cell));
        let mut slice = cell.as_slice().expect("cell must be readable");
        value = T::load_from(&mut slice).expect("value must decode");
        acton_client::cell::ensure_empty(&slice).expect("slice must be exhausted");
        last_typed_cell = Some(cell);
    }

    let abi = acton_client::DynamicAbi::from_json(include_str!(
        "fixtures/upstream/lots-of-wrappers.abi.json"
    ))
    .expect("fixture ABI must parse");
    let rust_type_name = std::any::type_name::<T>();
    let declaration_name = rust_type_name
        .rsplit("::")
        .next()
        .expect("generated type name must contain a declaration name");
    let ty_idx = abi
        .declaration_type_index(declaration_name)
        .expect("generated type must have an ABI declaration");
    let mut seed_slice = last_typed_cell
        .as_ref()
        .expect("the typed codec must produce a cell")
        .as_slice()
        .expect("typed cell must be readable");
    let mut dynamic_value = abi
        .unpack_from_slice(ty_idx, &mut seed_slice)
        .expect("typed cell must decode dynamically");
    acton_client::cell::ensure_empty(&seed_slice).expect("dynamic seed must consume the slice");
    for _ in 0..2 {
        let cell = abi
            .pack_to_cell(ty_idx, &dynamic_value)
            .expect("dynamic value must encode");
        cells.push(cell_tree(&cell));
        let mut slice = cell.as_slice().expect("dynamic cell must be readable");
        dynamic_value = abi
            .unpack_from_slice(ty_idx, &mut slice)
            .expect("dynamic value must decode");
        acton_client::cell::ensure_empty(&slice).expect("dynamic codec must consume the slice");
    }
    (cells, value)
}

fn cell_44_with_ref_45() -> Cell {
    let mut child = CellBuilder::new();
    child.store_u32(45).expect("value must fit");
    let child = child.build().expect("child must build");
    let mut root = CellBuilder::new();
    root.store_u32(44).expect("value must fit");
    root.store_reference(child).expect("reference must fit");
    root.build().expect("root must build")
}

fn slice_44_with_ref_45() -> OwnedSlice {
    OwnedSlice::full(cell_44_with_ref_45())
}

fn convert_bin_slice_to_hex(binary: &str) -> String {
    let bits = binary
        .strip_prefix("b{")
        .and_then(|value| value.strip_suffix('}'))
        .expect("binary notation must be valid");
    let mut result = String::from("x{");
    let chunks = bits.as_bytes().chunks(4);
    for chunk in chunks {
        let mut value = 0_u8;
        for bit in chunk {
            value = (value << 1) | u8::from(*bit == b'1');
        }
        if chunk.len() == 4 {
            result.push(
                char::from_digit(u32::from(value), 16)
                    .expect("a four-bit value must be a hex digit")
                    .to_ascii_uppercase(),
            );
        } else {
            value = (value << 1) | 1;
            value <<= 3 - chunk.len();
            result.push(
                char::from_digit(u32::from(value), 16)
                    .expect("a four-bit value must be a hex digit")
                    .to_ascii_uppercase(),
            );
            result.push('_');
        }
    }
    result.push('}');
    result
}

#[test]
fn utils() {
    expect![[r#"
        [
            "x{4_}",
            "x{2_}",
            "x{1_}",
            "x{0}",
            "x{04_}",
            "x{03E_}",
        ]
    "#]]
    .assert_debug_eq(&[
        convert_bin_slice_to_hex("b{0}"),
        convert_bin_slice_to_hex("b{00}"),
        convert_bin_slice_to_hex("b{000}"),
        convert_bin_slice_to_hex("b{0000}"),
        convert_bin_slice_to_hex("b{00000}"),
        convert_bin_slice_to_hex("b{0000001111}"),
    ]);
}

#[test]
fn just_uint5() {
    let values = [0, 15, 16, 31].map(|value| run(generated::JustUint5 { value: bi(value) }));
    let decoded = ["x{04_}", "x{7C_}", "x{84_}", "x{FC_}"].map(|notation| {
        generated::JustUint5::from_cell(&make_cell(notation, vec![]))
            .expect("value must decode")
            .value
    });
    expect![[r#"
        (
            [
                (
                    [
                        "x{04_}",
                        "x{04_}",
                        "x{04_}",
                        "x{04_}",
                    ],
                    JustUint5 {
                        value: 0,
                    },
                ),
                (
                    [
                        "x{7C_}",
                        "x{7C_}",
                        "x{7C_}",
                        "x{7C_}",
                    ],
                    JustUint5 {
                        value: 15,
                    },
                ),
                (
                    [
                        "x{84_}",
                        "x{84_}",
                        "x{84_}",
                        "x{84_}",
                    ],
                    JustUint5 {
                        value: 16,
                    },
                ),
                (
                    [
                        "x{FC_}",
                        "x{FC_}",
                        "x{FC_}",
                        "x{FC_}",
                    ],
                    JustUint5 {
                        value: 31,
                    },
                ),
            ],
            [
                0,
                15,
                16,
                31,
            ],
        )
    "#]]
    .assert_debug_eq(&(values, decoded));
}

#[test]
fn just_int32() {
    let round_trip = run(generated::JustInt32 { value: bi(255) });
    let cell = make_cell("x{0000007b000001c8}", vec![]);
    let mut slice = cell.as_slice().expect("cell must be readable");
    let first = generated::JustInt32::load_from(&mut slice).expect("first value must decode");
    let second = generated::JustInt32::load_from(&mut slice).expect("second value must decode");
    expect![[r#"
        (
            (
                [
                    "x{000000FF}",
                    "x{000000FF}",
                    "x{000000FF}",
                    "x{000000FF}",
                ],
                JustInt32 {
                    value: 255,
                },
            ),
            123,
            456,
        )
    "#]]
    .assert_debug_eq(&(round_trip, first.value, second.value));
}

#[test]
fn just_maybe_int32() {
    expect![[r#"
        (
            (
                [
                    "x{8000007FC_}",
                    "x{8000007FC_}",
                    "x{8000007FC_}",
                    "x{8000007FC_}",
                ],
                JustMaybeInt32 {
                    value: Some(
                        255,
                    ),
                },
            ),
            (
                [
                    "x{4_}",
                    "x{4_}",
                    "x{4_}",
                    "x{4_}",
                ],
                JustMaybeInt32 {
                    value: None,
                },
            ),
        )
    "#]]
    .assert_debug_eq(&(
        run(generated::JustMaybeInt32 {
            value: Some(bi(255)),
        }),
        run(generated::JustMaybeInt32::default()),
    ));
}

#[test]
fn two_ints32_and_coins() {
    expect![[r#"
        (
            (
                [
                    "x{0000007B0}",
                    "x{0000007B0}",
                    "x{0000007B0}",
                    "x{0000007B0}",
                ],
                TwoInts32AndCoins {
                    op: 123,
                    amount: 0,
                },
            ),
            (
                [
                    "x{0000007B43B9ACA00}",
                    "x{0000007B43B9ACA00}",
                    "x{0000007B43B9ACA00}",
                    "x{0000007B43B9ACA00}",
                ],
                TwoInts32AndCoins {
                    op: 123,
                    amount: 1000000000,
                },
            ),
        )
    "#]]
    .assert_debug_eq(&(
        run(generated::TwoInts32AndCoins {
            op: bi(123),
            amount: bi(0),
        }),
        run(generated::TwoInts32AndCoins::create(bi(123))),
    ));
}

#[test]
fn two_ints32_and64() {
    expect![[r#"
        (
            [
                "x{0000007B00000000000000FF}",
                "x{0000007B00000000000000FF}",
                "x{0000007B00000000000000FF}",
                "x{0000007B00000000000000FF}",
            ],
            TwoInts32And64 {
                op: 123,
                query_id: 255,
            },
        )
    "#]]
    .assert_debug_eq(&run(generated::TwoInts32And64 {
        op: bi(123),
        query_id: bi(255),
    }));
}

#[test]
fn two_ints32_and_ref64() {
    expect![[r#"
        (
            [
                "x{0000007B}\n x{00000000000000FF}",
                "x{0000007B}\n x{00000000000000FF}",
                "x{0000007B}\n x{00000000000000FF}",
                "x{0000007B}\n x{00000000000000FF}",
            ],
            TwoInts32AndRef64 {
                op: 123,
                query_id_ref: CellRef {
                    ref: 255,
                },
            },
        )
    "#]]
    .assert_debug_eq(&run(generated::TwoInts32AndRef64 {
        op: bi(123),
        query_id_ref: CellRef::new(bi(255)),
    }));
}

#[test]
fn two_ints32_and_maybe64() {
    expect![[r#"
        [
            (
                [
                    "x{0000007B800000000000007FE_}",
                    "x{0000007B800000000000007FE_}",
                    "x{0000007B800000000000007FE_}",
                    "x{0000007B800000000000007FE_}",
                ],
                TwoInts32AndMaybe64 {
                    op: 123,
                    query_id: Some(
                        255,
                    ),
                    demo_bool_field: true,
                },
            ),
            (
                [
                    "x{0000007B6_}",
                    "x{0000007B6_}",
                    "x{0000007B6_}",
                    "x{0000007B6_}",
                ],
                TwoInts32AndMaybe64 {
                    op: 123,
                    query_id: None,
                    demo_bool_field: true,
                },
            ),
            (
                [
                    "x{0000007B2_}",
                    "x{0000007B2_}",
                    "x{0000007B2_}",
                    "x{0000007B2_}",
                ],
                TwoInts32AndMaybe64 {
                    op: 123,
                    query_id: None,
                    demo_bool_field: false,
                },
            ),
        ]
    "#]]
    .assert_debug_eq(&[
        run(generated::TwoInts32AndMaybe64 {
            op: bi(123),
            query_id: Some(bi(255)),
            demo_bool_field: true,
        }),
        run(generated::TwoInts32AndMaybe64 {
            op: bi(123),
            query_id: None,
            demo_bool_field: true,
        }),
        run(generated::TwoInts32AndMaybe64 {
            op: bi(123),
            query_id: None,
            demo_bool_field: false,
        }),
    ]);
}

#[test]
fn just_address() {
    expect![[r#"
        (
            [
                "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
            ],
            JustAddress {
                addr: StdAddr {
                    anycast: None,
                    workchain: 0,
                    address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                },
            },
        )
    "#]]
    .assert_debug_eq(&run(generated::JustAddress {
        addr: std_addr("0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e"),
    }));
}

#[test]
fn two_ints32_and64_sep_by_address() {
    expect![[r#"
        [
            (
                [
                    "x{0000007B80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000000000000201_}",
                    "x{0000007B80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000000000000201_}",
                    "x{0000007B80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000000000000201_}",
                    "x{0000007B80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000000000000201_}",
                ],
                TwoInts32And64SepByAddress {
                    op: 123,
                    addr_e: Std(
                        StdAddr {
                            anycast: None,
                            workchain: 0,
                            address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                        },
                    ),
                    query_id: 256,
                },
            ),
            (
                [
                    "x{0000007B41423000000000000007FC_}",
                    "x{0000007B41423000000000000007FC_}",
                    "x{0000007B41423000000000000007FC_}",
                    "x{0000007B41423000000000000007FC_}",
                ],
                TwoInts32And64SepByAddress {
                    op: 123,
                    addr_e: Ext(
                        ExtAddr {
                            data_bit_len: Uint9(
                                10,
                            ),
                            data: [
                                17,
                                128,
                            ],
                        },
                    ),
                    query_id: 255,
                },
            ),
            (
                [
                    "x{0000007B4280053400000000000001FD_}",
                    "x{0000007B4280053400000000000001FD_}",
                    "x{0000007B4280053400000000000001FD_}",
                    "x{0000007B4280053400000000000001FD_}",
                ],
                TwoInts32And64SepByAddress {
                    op: 123,
                    addr_e: Ext(
                        ExtAddr {
                            data_bit_len: Uint9(
                                20,
                            ),
                            data: [
                                0,
                                41,
                                160,
                            ],
                        },
                    ),
                    query_id: 254,
                },
            ),
            (
                [
                    "x{0000007B000000000000003F6_}",
                    "x{0000007B000000000000003F6_}",
                    "x{0000007B000000000000003F6_}",
                    "x{0000007B000000000000003F6_}",
                ],
                TwoInts32And64SepByAddress {
                    op: 123,
                    addr_e: None,
                    query_id: 253,
                },
            ),
        ]
    "#]].assert_debug_eq(&[
        run(generated::TwoInts32And64SepByAddress {
            op: bi(123),
            addr_e: any_std("0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e"),
            query_id: bi(256),
        }),
        run(generated::TwoInts32And64SepByAddress {
            op: bi(123),
            addr_e: any_ext(70, 10),
            query_id: bi(255),
        }),
        run(generated::TwoInts32And64SepByAddress {
            op: bi(123),
            addr_e: any_ext(666, 20),
            query_id: bi(254),
        }),
        run(generated::TwoInts32And64SepByAddress {
            op: bi(123),
            addr_e: AnyAddr::None,
            query_id: bi(253),
        }),
    ]);
}

#[test]
fn int_and_either_int8_or256() {
    expect![[r#"
        [
            (
                [
                    "x{0000007B284_}",
                    "x{0000007B284_}",
                    "x{0000007B284_}",
                    "x{0000007B284_}",
                ],
                IntAndEitherInt8Or256 {
                    op: 123,
                    i8or256: Variant0(
                        80,
                    ),
                },
            ),
            (
                [
                    "x{0000007B8000000000000000000000000000000000000000000000000000000000007FFFC_}",
                    "x{0000007B8000000000000000000000000000000000000000000000000000000000007FFFC_}",
                    "x{0000007B8000000000000000000000000000000000000000000000000000000000007FFFC_}",
                    "x{0000007B8000000000000000000000000000000000000000000000000000000000007FFFC_}",
                ],
                IntAndEitherInt8Or256 {
                    op: 123,
                    i8or256: Variant1(
                        65535,
                    ),
                },
            ),
        ]
    "#]]
    .assert_debug_eq(&[
        run(generated::IntAndEitherInt8Or256 {
            op: bi(123),
            i8or256: generated::UnionTy29::Variant0(bi(80)),
        }),
        run(generated::IntAndEitherInt8Or256 {
            op: bi(123),
            i8or256: generated::UnionTy29::Variant1(bi(65_535)),
        }),
    ]);
}

#[test]
fn int_and_either32_or_ref64() {
    expect![[r#"
        [
            (
                [
                    "x{0000007BE_}\n x{000000000000022B}\n x{0000000000000378}",
                    "x{0000007BE_}\n x{000000000000022B}\n x{0000000000000378}",
                    "x{0000007BE_}\n x{000000000000022B}\n x{0000000000000378}",
                    "x{0000007BE_}\n x{000000000000022B}\n x{0000000000000378}",
                ],
                IntAndEither32OrRef64 {
                    op: 123,
                    i32or_ref: Variant1(
                        CellRef {
                            ref: Inner2 {
                                i64_in_ref: 555,
                            },
                        },
                    ),
                    query_id_maybe_ref: Some(
                        CellRef {
                            ref: Inner1 {
                                query_id_ref: 888,
                            },
                        },
                    ),
                },
            ),
            (
                [
                    "x{0000007BA_}\n x{000000000000022B}",
                    "x{0000007BA_}\n x{000000000000022B}",
                    "x{0000007BA_}\n x{000000000000022B}",
                    "x{0000007BA_}\n x{000000000000022B}",
                ],
                IntAndEither32OrRef64 {
                    op: 123,
                    i32or_ref: Variant1(
                        CellRef {
                            ref: Inner2 {
                                i64_in_ref: 555,
                            },
                        },
                    ),
                    query_id_maybe_ref: None,
                },
            ),
            (
                [
                    "x{0000007B00000115E_}\n x{0000000000000378}",
                    "x{0000007B00000115E_}\n x{0000000000000378}",
                    "x{0000007B00000115E_}\n x{0000000000000378}",
                    "x{0000007B00000115E_}\n x{0000000000000378}",
                ],
                IntAndEither32OrRef64 {
                    op: 123,
                    i32or_ref: Variant0(
                        555,
                    ),
                    query_id_maybe_ref: Some(
                        CellRef {
                            ref: Inner1 {
                                query_id_ref: 888,
                            },
                        },
                    ),
                },
            ),
            (
                [
                    "x{0000007B00000115A_}",
                    "x{0000007B00000115A_}",
                    "x{0000007B00000115A_}",
                    "x{0000007B00000115A_}",
                ],
                IntAndEither32OrRef64 {
                    op: 123,
                    i32or_ref: Variant0(
                        555,
                    ),
                    query_id_maybe_ref: None,
                },
            ),
        ]
    "#]]
    .assert_debug_eq(&[
        run(generated::IntAndEither32OrRef64 {
            op: bi(123),
            i32or_ref: generated::UnionTy33::Variant1(CellRef::new(generated::Inner2 {
                i64_in_ref: bi(555),
            })),
            query_id_maybe_ref: Some(CellRef::new(generated::Inner1 {
                query_id_ref: bi(888),
            })),
        }),
        run(generated::IntAndEither32OrRef64 {
            op: bi(123),
            i32or_ref: generated::UnionTy33::Variant1(CellRef::new(generated::Inner2 {
                i64_in_ref: bi(555),
            })),
            query_id_maybe_ref: None,
        }),
        run(generated::IntAndEither32OrRef64 {
            op: bi(123),
            i32or_ref: generated::UnionTy33::Variant0(bi(555)),
            query_id_maybe_ref: Some(CellRef::new(generated::Inner1 {
                query_id_ref: bi(888),
            })),
        }),
        run(generated::IntAndEither32OrRef64 {
            op: bi(123),
            i32or_ref: generated::UnionTy33::Variant0(bi(555)),
            query_id_maybe_ref: None,
        }),
    ]);
}

#[test]
fn int_and_either_maybe8_or256() {
    let results = [
        run(generated::IntAndEither8OrMaybe256 {
            value: generated::UnionTy51::Variant0(generated::EitherLeft { value: bi(100) }),
            op: bi(123),
        }),
        run(generated::IntAndEither8OrMaybe256 {
            value: generated::UnionTy51::Variant1(generated::EitherRight {
                value: Some(bi(10_000)),
            }),
            op: bi(123),
        }),
        run(generated::IntAndEither8OrMaybe256 {
            value: generated::UnionTy51::Variant1(generated::EitherRight { value: None }),
            op: bi(123),
        }),
    ];
    let decoded =
        generated::IntAndEither8OrMaybe256::from_cell(&make_cell("x{8000001EE_}", vec![]))
            .expect("value must decode");
    expect![[r#"
        (
            [
                (
                    [
                        "x{320000003DC_}",
                        "x{320000003DC_}",
                        "x{320000003DC_}",
                        "x{320000003DC_}",
                    ],
                    IntAndEither8OrMaybe256 {
                        value: Variant0(
                            EitherLeft {
                                value: 100,
                            },
                        ),
                        op: 123,
                    },
                ),
                (
                    [
                        "x{C0000000000000000000000000000000000000000000000000000000000009C40000001EE_}",
                        "x{C0000000000000000000000000000000000000000000000000000000000009C40000001EE_}",
                        "x{C0000000000000000000000000000000000000000000000000000000000009C40000001EE_}",
                        "x{C0000000000000000000000000000000000000000000000000000000000009C40000001EE_}",
                    ],
                    IntAndEither8OrMaybe256 {
                        value: Variant1(
                            EitherRight {
                                value: Some(
                                    10000,
                                ),
                            },
                        ),
                        op: 123,
                    },
                ),
                (
                    [
                        "x{8000001EE_}",
                        "x{8000001EE_}",
                        "x{8000001EE_}",
                        "x{8000001EE_}",
                    ],
                    IntAndEither8OrMaybe256 {
                        value: Variant1(
                            EitherRight {
                                value: None,
                            },
                        ),
                        op: 123,
                    },
                ),
            ],
            IntAndEither8OrMaybe256 {
                value: Variant1(
                    EitherRight {
                        value: None,
                    },
                ),
                op: 123,
            },
        )
    "#]].assert_debug_eq(&(results, decoded));
}

#[test]
fn int_and_maybe_maybe8() {
    let results = [
        run(generated::IntAndMaybeMaybe8 {
            value: maybe_just(maybe_just(bi(88))),
            op: bi(123),
        }),
        run(generated::IntAndMaybeMaybe8 {
            value: maybe_just(maybe_nothing()),
            op: bi(123),
        }),
        run(generated::IntAndMaybeMaybe8 {
            value: maybe_nothing(),
            op: bi(-1),
        }),
    ];
    let decoded1 = generated::IntAndMaybeMaybe8::from_cell(&make_cell("x{D60000001EE_}", vec![]))
        .expect("value must decode");
    let decoded2 = generated::IntAndMaybeMaybe8::from_cell(&make_cell("x{8000001EE_}", vec![]))
        .expect("value must decode");
    expect![[r#"
        (
            [
                (
                    [
                        "x{D60000001EE_}",
                        "x{D60000001EE_}",
                        "x{D60000001EE_}",
                        "x{D60000001EE_}",
                    ],
                    IntAndMaybeMaybe8 {
                        value: Variant1(
                            MaybeJust {
                                value: Variant1(
                                    MaybeJust {
                                        value: 88,
                                    },
                                ),
                            },
                        ),
                        op: 123,
                    },
                ),
                (
                    [
                        "x{8000001EE_}",
                        "x{8000001EE_}",
                        "x{8000001EE_}",
                        "x{8000001EE_}",
                    ],
                    IntAndMaybeMaybe8 {
                        value: Variant1(
                            MaybeJust {
                                value: Variant0(
                                    MaybeNothing,
                                ),
                            },
                        ),
                        op: 123,
                    },
                ),
                (
                    [
                        "x{7FFFFFFFC_}",
                        "x{7FFFFFFFC_}",
                        "x{7FFFFFFFC_}",
                        "x{7FFFFFFFC_}",
                    ],
                    IntAndMaybeMaybe8 {
                        value: Variant0(
                            MaybeNothing,
                        ),
                        op: -1,
                    },
                ),
            ],
            IntAndMaybeMaybe8 {
                value: Variant1(
                    MaybeJust {
                        value: Variant1(
                            MaybeJust {
                                value: 88,
                            },
                        ),
                    },
                ),
                op: 123,
            },
            IntAndMaybeMaybe8 {
                value: Variant1(
                    MaybeJust {
                        value: Variant0(
                            MaybeNothing,
                        ),
                    },
                ),
                op: 123,
            },
        )
    "#]]
    .assert_debug_eq(&(results, decoded1, decoded2));
}

#[test]
fn some_bytes_fields() {
    let results = [
        run(generated::SomeBytesFields {
            f1: bit_string("x{A4}"),
            f2: bit_string("x{7_}"),
            f3: None,
            f4: generated::UnionTy70::Variant1(bit_string(
                "x{BBA87684B3DAA58C0FCC75230C4302C9D156102139D631FF56}",
            )),
        }),
        run(generated::SomeBytesFields {
            f1: bit_string("x{E6}"),
            f2: bit_string("x{D_}"),
            f3: Some(bit_string("x{2531C}")),
            f4: generated::UnionTy70::Variant0(bit_string("x{927E88FAB2D327D9468547217}")),
        }),
    ];
    let decoded = generated::SomeBytesFields::from_cell(&make_cell(
        "x{E6D2531C493F447D596993ECA342A390BC_}",
        vec![],
    ))
    .expect("value must decode");
    expect![[r#"
        (
            [
                (
                    [
                        "x{A46DDD43B4259ED52C607E63A9186218164E8AB08109CEB18FFAB4_}",
                        "x{A46DDD43B4259ED52C607E63A9186218164E8AB08109CEB18FFAB4_}",
                        "x{A46DDD43B4259ED52C607E63A9186218164E8AB08109CEB18FFAB4_}",
                        "x{A46DDD43B4259ED52C607E63A9186218164E8AB08109CEB18FFAB4_}",
                    ],
                    SomeBytesFields {
                        f1: BitString(
                            OwnedSlice {
                                range: CellSliceRange {
                                    bits_start: 0,
                                    bits_end: 8,
                                    refs_start: 0,
                                    refs_end: 0,
                                },
                                cell: Cell {
                                    ty: Ordinary,
                                    hash: 68ee33936b7663e8bbed5b80b02f5a1df91225eb9f66ae67c08fe735f70c3f6c,
                                },
                            },
                        ),
                        f2: BitString(
                            OwnedSlice {
                                range: CellSliceRange {
                                    bits_start: 0,
                                    bits_end: 3,
                                    refs_start: 0,
                                    refs_end: 0,
                                },
                                cell: Cell {
                                    ty: Ordinary,
                                    hash: 80e0a32f40e328ee85bc81aa128b02d262a546757c0998c65a9c2431a104c301,
                                },
                            },
                        ),
                        f3: None,
                        f4: Variant1(
                            BitString(
                                OwnedSlice {
                                    range: CellSliceRange {
                                        bits_start: 0,
                                        bits_end: 200,
                                        refs_start: 0,
                                        refs_end: 0,
                                    },
                                    cell: Cell {
                                        ty: Ordinary,
                                        hash: 9941dda70df981b2e8724b31f6b1c9522405843a8fd00bbe268b19c7badacd1d,
                                    },
                                },
                            ),
                        ),
                    },
                ),
                (
                    [
                        "x{E6D2531C493F447D596993ECA342A390BC_}",
                        "x{E6D2531C493F447D596993ECA342A390BC_}",
                        "x{E6D2531C493F447D596993ECA342A390BC_}",
                        "x{E6D2531C493F447D596993ECA342A390BC_}",
                    ],
                    SomeBytesFields {
                        f1: BitString(
                            OwnedSlice {
                                range: CellSliceRange {
                                    bits_start: 0,
                                    bits_end: 8,
                                    refs_start: 0,
                                    refs_end: 0,
                                },
                                cell: Cell {
                                    ty: Ordinary,
                                    hash: 569021824ee5185bdc86fb58a52ae439406a5eaaa8a474a30c5a2fb9e7159c2d,
                                },
                            },
                        ),
                        f2: BitString(
                            OwnedSlice {
                                range: CellSliceRange {
                                    bits_start: 0,
                                    bits_end: 3,
                                    refs_start: 0,
                                    refs_end: 0,
                                },
                                cell: Cell {
                                    ty: Ordinary,
                                    hash: 7309bf377cdd0df6412f3888aef831e48fe2f7f062ec55611507f926966bb9c2,
                                },
                            },
                        ),
                        f3: Some(
                            BitString(
                                OwnedSlice {
                                    range: CellSliceRange {
                                        bits_start: 0,
                                        bits_end: 20,
                                        refs_start: 0,
                                        refs_end: 0,
                                    },
                                    cell: Cell {
                                        ty: Ordinary,
                                        hash: 8c0bd7a1dc887083fa64557e346bba5853f7f4cad627d4f2e98822608005041a,
                                    },
                                },
                            ),
                        ),
                        f4: Variant0(
                            BitString(
                                OwnedSlice {
                                    range: CellSliceRange {
                                        bits_start: 0,
                                        bits_end: 100,
                                        refs_start: 0,
                                        refs_end: 0,
                                    },
                                    cell: Cell {
                                        ty: Ordinary,
                                        hash: 04f213e32117ff6298f8e40ffd462b86133b4550210cf31ab6950baa0b8c65a2,
                                    },
                                },
                            ),
                        ),
                    },
                ),
            ],
            SomeBytesFields {
                f1: BitString(
                    OwnedSlice {
                        range: CellSliceRange {
                            bits_start: 0,
                            bits_end: 8,
                            refs_start: 0,
                            refs_end: 0,
                        },
                        cell: Cell {
                            ty: Ordinary,
                            hash: 569021824ee5185bdc86fb58a52ae439406a5eaaa8a474a30c5a2fb9e7159c2d,
                        },
                    },
                ),
                f2: BitString(
                    OwnedSlice {
                        range: CellSliceRange {
                            bits_start: 0,
                            bits_end: 3,
                            refs_start: 0,
                            refs_end: 0,
                        },
                        cell: Cell {
                            ty: Ordinary,
                            hash: 7309bf377cdd0df6412f3888aef831e48fe2f7f062ec55611507f926966bb9c2,
                        },
                    },
                ),
                f3: Some(
                    BitString(
                        OwnedSlice {
                            range: CellSliceRange {
                                bits_start: 0,
                                bits_end: 20,
                                refs_start: 0,
                                refs_end: 0,
                            },
                            cell: Cell {
                                ty: Ordinary,
                                hash: 8c0bd7a1dc887083fa64557e346bba5853f7f4cad627d4f2e98822608005041a,
                            },
                        },
                    ),
                ),
                f4: Variant0(
                    BitString(
                        OwnedSlice {
                            range: CellSliceRange {
                                bits_start: 0,
                                bits_end: 100,
                                refs_start: 0,
                                refs_end: 0,
                            },
                            cell: Cell {
                                ty: Ordinary,
                                hash: 04f213e32117ff6298f8e40ffd462b86133b4550210cf31ab6950baa0b8c65a2,
                            },
                        },
                    ),
                ),
            },
        )
    "#]].assert_debug_eq(&(results, decoded));
}

#[test]
fn int_and_rest_inline_cell() {
    let round_trip = run(generated::IntAndRestInlineCell {
        op: bi(123),
        rest: slice_44_with_ref_45(),
    });
    let cell = make_cell("x{00000000FFFF}", vec![]);
    let mut slice = cell.as_slice().expect("cell must be readable");
    let decoded =
        generated::IntAndRestInlineCell::load_from(&mut slice).expect("value must decode");
    let remaining = decoded
        .rest
        .as_slice()
        .expect("remaining slice must be readable")
        .size_bits();
    expect![[r#"
        (
            (
                [
                    "x{0000007B0000002C}\n x{0000002D}",
                    "x{0000007B0000002C}\n x{0000002D}",
                    "x{0000007B0000002C}\n x{0000002D}",
                    "x{0000007B0000002C}\n x{0000002D}",
                ],
                IntAndRestInlineCell {
                    op: 123,
                    rest: OwnedSlice {
                        range: CellSliceRange {
                            bits_start: 0,
                            bits_end: 32,
                            refs_start: 0,
                            refs_end: 1,
                        },
                        cell: Cell {
                            ty: Ordinary,
                            hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                        },
                    },
                },
            ),
            16,
            0,
        )
    "#]]
    .assert_debug_eq(&(round_trip, remaining, slice.size_bits()));
}

#[test]
fn int_and_rest_ref_cell() {
    expect![[r#"
        (
            [
                "x{0000007B}\n x{0000002C}\n  x{0000002D}",
                "x{0000007B}\n x{0000002C}\n  x{0000002D}",
                "x{0000007B}\n x{0000002C}\n  x{0000002D}",
                "x{0000007B}\n x{0000002C}\n  x{0000002D}",
            ],
            IntAndRestRefCell {
                op: 123,
                rest: Cell {
                    ty: Ordinary,
                    hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                },
            },
        )
    "#]]
    .assert_debug_eq(&run(generated::IntAndRestRefCell {
        op: bi(123),
        rest: cell_44_with_ref_45(),
    }));
}

#[test]
fn int_and_rest_either_cell_or_ref_cell() {
    let input1 = generated::IntAndRestEitherCellOrRefCell {
        op: bi(123),
        rest: generated::UnionTy75::Variant1(cell_44_with_ref_45()),
    };
    let input2 = generated::IntAndRestEitherCellOrRefCell {
        op: bi(123),
        rest: generated::UnionTy75::Variant0(slice_44_with_ref_45()),
    };
    let cell1 = input1.to_cell().expect("first value must encode");
    let decoded1 = generated::IntAndRestEitherCellOrRefCell::from_cell(&cell1)
        .expect("first value must decode");
    let first_variant = match decoded1.rest {
        generated::UnionTy75::Variant1(_) => "cell",
        generated::UnionTy75::Variant0(_) => "RemainingBitsAndRefs",
    };

    let mut builder2 = CellBuilder::new();
    input2
        .store_into(&mut builder2)
        .expect("second value must encode");
    let cell2 = builder2.build().expect("second cell must build");
    let decoded2 = generated::IntAndRestEitherCellOrRefCell::from_cell(&cell2)
        .expect("second value must decode");
    let (second_variant, second_remaining_refs) = match decoded2.rest {
        generated::UnionTy75::Variant0(rest) => ("RemainingBitsAndRefs", rest.range.size_refs()),
        generated::UnionTy75::Variant1(_) => ("cell", 0),
    };
    expect![[r#"
        (
            "x{0000007BC_}\n x{0000002C}\n  x{0000002D}",
            "cell",
            "x{0000007B000000164_}\n x{0000002D}",
            "RemainingBitsAndRefs",
            1,
        )
    "#]]
    .assert_debug_eq(&(
        cell_tree(&cell1),
        first_variant,
        cell_tree(&cell2),
        second_variant,
        second_remaining_refs,
    ));
}

#[test]
fn different_maybe_refs() {
    let empty = || CellBuilder::new().build().expect("empty cell must build");
    expect![[r#"
        [
            (
                [
                    "x{0000007B00000000000000391_}\n x{0000002C}\n  x{0000002D}",
                    "x{0000007B00000000000000391_}\n x{0000002C}\n  x{0000002D}",
                    "x{0000007B00000000000000391_}\n x{0000002C}\n  x{0000002D}",
                    "x{0000007B00000000000000391_}\n x{0000002C}\n  x{0000002D}",
                ],
                DifferentMaybeRefs {
                    op: 123,
                    ref1m: None,
                    ref2m: None,
                    ref3: Cell {
                        ty: Ordinary,
                        hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                    },
                    ref4m32: None,
                    query_id: 456,
                },
            ),
            (
                [
                    "x{0000007BA0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{000003E7}",
                    "x{0000007BA0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{000003E7}",
                    "x{0000007BA0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{000003E7}",
                    "x{0000007BA0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{000003E7}",
                ],
                DifferentMaybeRefs {
                    op: 123,
                    ref1m: Some(
                        Cell {
                            ty: Ordinary,
                            hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                        },
                    ),
                    ref2m: None,
                    ref3: Cell {
                        ty: Ordinary,
                        hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                    },
                    ref4m32: Some(
                        CellRef {
                            ref: JustInt32 {
                                value: 999,
                            },
                        },
                    ),
                    query_id: 456,
                },
            ),
            (
                [
                    "x{0000007B60000000000000391_}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E6}",
                    "x{0000007B60000000000000391_}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E6}",
                    "x{0000007B60000000000000391_}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E6}",
                    "x{0000007B60000000000000391_}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E6}",
                ],
                DifferentMaybeRefs {
                    op: 123,
                    ref1m: None,
                    ref2m: Some(
                        Cell {
                            ty: Ordinary,
                            hash: 96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7,
                        },
                    ),
                    ref3: Cell {
                        ty: Ordinary,
                        hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                    },
                    ref4m32: Some(
                        CellRef {
                            ref: JustInt32 {
                                value: 998,
                            },
                        },
                    ),
                    query_id: 456,
                },
            ),
            (
                [
                    "x{0000007BC0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{}",
                    "x{0000007BC0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{}",
                    "x{0000007BC0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{}",
                    "x{0000007BC0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{0000002C}\n  x{0000002D}\n x{}",
                ],
                DifferentMaybeRefs {
                    op: 123,
                    ref1m: Some(
                        Cell {
                            ty: Ordinary,
                            hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                        },
                    ),
                    ref2m: Some(
                        Cell {
                            ty: Ordinary,
                            hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                        },
                    ),
                    ref3: Cell {
                        ty: Ordinary,
                        hash: 96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7,
                    },
                    ref4m32: None,
                    query_id: 456,
                },
            ),
            (
                [
                    "x{0000007BE0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E5}",
                    "x{0000007BE0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E5}",
                    "x{0000007BE0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E5}",
                    "x{0000007BE0000000000000391_}\n x{0000002C}\n  x{0000002D}\n x{}\n x{0000002C}\n  x{0000002D}\n x{000003E5}",
                ],
                DifferentMaybeRefs {
                    op: 123,
                    ref1m: Some(
                        Cell {
                            ty: Ordinary,
                            hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                        },
                    ),
                    ref2m: Some(
                        Cell {
                            ty: Ordinary,
                            hash: 96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7,
                        },
                    ),
                    ref3: Cell {
                        ty: Ordinary,
                        hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                    },
                    ref4m32: Some(
                        CellRef {
                            ref: JustInt32 {
                                value: 997,
                            },
                        },
                    ),
                    query_id: 456,
                },
            ),
        ]
    "#]].assert_debug_eq(&[
        run(generated::DifferentMaybeRefs::create(
            None,
            None,
            cell_44_with_ref_45(),
            None,
            bi(456),
        )),
        run(generated::DifferentMaybeRefs::create(
            Some(cell_44_with_ref_45()),
            None,
            cell_44_with_ref_45(),
            Some(CellRef::new(generated::JustInt32 { value: bi(999) })),
            bi(456),
        )),
        run(generated::DifferentMaybeRefs::create(
            None,
            Some(empty()),
            cell_44_with_ref_45(),
            Some(CellRef::new(generated::JustInt32 { value: bi(998) })),
            bi(456),
        )),
        run(generated::DifferentMaybeRefs::create(
            Some(cell_44_with_ref_45()),
            Some(cell_44_with_ref_45()),
            empty(),
            None,
            bi(456),
        )),
        run(generated::DifferentMaybeRefs::create(
            Some(cell_44_with_ref_45()),
            Some(empty()),
            cell_44_with_ref_45(),
            Some(CellRef::new(generated::JustInt32 { value: bi(997) })),
            bi(456),
        )),
    ]);
}

#[test]
fn different_ints_with_maybe() {
    let results = [
        run(generated::DifferentIntsWithMaybe {
            ji: generated::JustInt32 { value: bi(44) },
            jmi: generated::JustMaybeInt32 {
                value: Some(bi(45)),
            },
            ji_maybe: None,
            jmi_maybe: None,
        }),
        run(generated::DifferentIntsWithMaybe {
            ji: generated::JustInt32 { value: bi(44) },
            jmi: generated::JustMaybeInt32 { value: None },
            ji_maybe: Some(generated::JustInt32 { value: bi(45) }),
            jmi_maybe: Some(generated::JustMaybeInt32 { value: None }),
        }),
        run(generated::DifferentIntsWithMaybe {
            ji: generated::JustInt32 { value: bi(44) },
            jmi: generated::JustMaybeInt32::default(),
            ji_maybe: None,
            jmi_maybe: Some(generated::JustMaybeInt32 {
                value: Some(bi(46)),
            }),
        }),
    ];
    let decoded =
        generated::DifferentIntsWithMaybe::from_cell(&make_cell("x{0000002C30000002E}", vec![]))
            .expect("value must decode");
    expect![[r#"
        (
            [
                (
                    [
                        "x{0000002C800000169_}",
                        "x{0000002C800000169_}",
                        "x{0000002C800000169_}",
                        "x{0000002C800000169_}",
                    ],
                    DifferentIntsWithMaybe {
                        ji: JustInt32 {
                            value: 44,
                        },
                        jmi: JustMaybeInt32 {
                            value: Some(
                                45,
                            ),
                        },
                        ji_maybe: None,
                        jmi_maybe: None,
                    },
                ),
                (
                    [
                        "x{0000002C4000000B6}",
                        "x{0000002C4000000B6}",
                        "x{0000002C4000000B6}",
                        "x{0000002C4000000B6}",
                    ],
                    DifferentIntsWithMaybe {
                        ji: JustInt32 {
                            value: 44,
                        },
                        jmi: JustMaybeInt32 {
                            value: None,
                        },
                        ji_maybe: Some(
                            JustInt32 {
                                value: 45,
                            },
                        ),
                        jmi_maybe: Some(
                            JustMaybeInt32 {
                                value: None,
                            },
                        ),
                    },
                ),
                (
                    [
                        "x{0000002C30000002E}",
                        "x{0000002C30000002E}",
                        "x{0000002C30000002E}",
                        "x{0000002C30000002E}",
                    ],
                    DifferentIntsWithMaybe {
                        ji: JustInt32 {
                            value: 44,
                        },
                        jmi: JustMaybeInt32 {
                            value: None,
                        },
                        ji_maybe: None,
                        jmi_maybe: Some(
                            JustMaybeInt32 {
                                value: Some(
                                    46,
                                ),
                            },
                        ),
                    },
                ),
            ],
            DifferentIntsWithMaybe {
                ji: JustInt32 {
                    value: 44,
                },
                jmi: JustMaybeInt32 {
                    value: None,
                },
                ji_maybe: None,
                jmi_maybe: Some(
                    JustMaybeInt32 {
                        value: Some(
                            46,
                        ),
                    },
                ),
            },
        )
    "#]]
    .assert_debug_eq(&(results, decoded));
}

#[test]
fn different_mix1() {
    let address = "EQCRDM9h4k3UJdOePPuyX40mCgA4vxge5Dc5vjBR8djbEKC5";
    let results = [
        run(generated::DifferentMix1 {
            ja1: generated::JustAddress {
                addr: std_addr(address),
            },
            ja2m: Some(generated::JustAddress {
                addr: std_addr(address),
            }),
            ext_nn: any_ext(1234, 30),
            imm: generated::IntAndMaybeMaybe8 {
                value: maybe_just(maybe_just(bi(78))),
                op: bi(78),
            },
            tis: generated::TwoInts32And64SepByAddress {
                op: bi(123),
                addr_e: any_ext(1234, 80),
                query_id: bi(889_128),
            },
        }),
        run(generated::DifferentMix1 {
            ja1: generated::JustAddress {
                addr: std_addr("EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c"),
            },
            ja2m: None,
            ext_nn: any_ext(0, 3),
            imm: generated::IntAndMaybeMaybe8 {
                value: maybe_just(maybe_just(bi(99))),
                op: bi(99),
            },
            tis: generated::TwoInts32And64SepByAddress {
                op: bi(1234),
                addr_e: AnyAddr::None,
                query_id: bi(889_129),
            },
        }),
    ];
    let decoded = generated::DifferentMix1::from_cell(&make_cell(
        "x{800000000000000000000000000000000000000000000000000000000000000000040636300000063000004D2000000000003644A6_}",
        vec![],
    ))
    .expect("value must decode");
    expect![[r#"
        (
            [
                (
                    [
                        "x{80122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62180122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62087800004D2D3800000138000001ED2800000000000000000269000000000006C8944_}",
                        "x{80122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62180122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62087800004D2D3800000138000001ED2800000000000000000269000000000006C8944_}",
                        "x{80122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62180122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62087800004D2D3800000138000001ED2800000000000000000269000000000006C8944_}",
                        "x{80122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62180122199EC3C49BA84BA73C79F764BF1A4C1400717E303DC86E737C60A3E3B1B62087800004D2D3800000138000001ED2800000000000000000269000000000006C8944_}",
                    ],
                    DifferentMix1 {
                        ja1: JustAddress {
                            addr: StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: 910ccf61e24dd425d39e3cfbb25f8d260a0038bf181ee43739be3051f1d8db10,
                            },
                        },
                        ja2m: Some(
                            JustAddress {
                                addr: StdAddr {
                                    anycast: None,
                                    workchain: 0,
                                    address: 910ccf61e24dd425d39e3cfbb25f8d260a0038bf181ee43739be3051f1d8db10,
                                },
                            },
                        ),
                        ext_nn: Ext(
                            ExtAddr {
                                data_bit_len: Uint9(
                                    30,
                                ),
                                data: [
                                    0,
                                    0,
                                    19,
                                    72,
                                ],
                            },
                        ),
                        imm: IntAndMaybeMaybe8 {
                            value: Variant1(
                                MaybeJust {
                                    value: Variant1(
                                        MaybeJust {
                                            value: 78,
                                        },
                                    ),
                                },
                            ),
                            op: 78,
                        },
                        tis: TwoInts32And64SepByAddress {
                            op: 123,
                            addr_e: Ext(
                                ExtAddr {
                                    data_bit_len: Uint9(
                                        80,
                                    ),
                                    data: [
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                        0,
                                        4,
                                        210,
                                    ],
                                },
                            ),
                            query_id: 889128,
                        },
                    },
                ),
                (
                    [
                        "x{800000000000000000000000000000000000000000000000000000000000000000040636300000063000004D2000000000003644A6_}",
                        "x{800000000000000000000000000000000000000000000000000000000000000000040636300000063000004D2000000000003644A6_}",
                        "x{800000000000000000000000000000000000000000000000000000000000000000040636300000063000004D2000000000003644A6_}",
                        "x{800000000000000000000000000000000000000000000000000000000000000000040636300000063000004D2000000000003644A6_}",
                    ],
                    DifferentMix1 {
                        ja1: JustAddress {
                            addr: StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: 0000000000000000000000000000000000000000000000000000000000000000,
                            },
                        },
                        ja2m: None,
                        ext_nn: Ext(
                            ExtAddr {
                                data_bit_len: Uint9(
                                    3,
                                ),
                                data: [
                                    0,
                                ],
                            },
                        ),
                        imm: IntAndMaybeMaybe8 {
                            value: Variant1(
                                MaybeJust {
                                    value: Variant1(
                                        MaybeJust {
                                            value: 99,
                                        },
                                    ),
                                },
                            ),
                            op: 99,
                        },
                        tis: TwoInts32And64SepByAddress {
                            op: 1234,
                            addr_e: None,
                            query_id: 889129,
                        },
                    },
                ),
            ],
            DifferentMix1 {
                ja1: JustAddress {
                    addr: StdAddr {
                        anycast: None,
                        workchain: 0,
                        address: 0000000000000000000000000000000000000000000000000000000000000000,
                    },
                },
                ja2m: None,
                ext_nn: Ext(
                    ExtAddr {
                        data_bit_len: Uint9(
                            3,
                        ),
                        data: [
                            0,
                        ],
                    },
                ),
                imm: IntAndMaybeMaybe8 {
                    value: Variant1(
                        MaybeJust {
                            value: Variant1(
                                MaybeJust {
                                    value: 99,
                                },
                            ),
                        },
                    ),
                    op: 99,
                },
                tis: TwoInts32And64SepByAddress {
                    op: 1234,
                    addr_e: None,
                    query_id: 889129,
                },
            },
        )
    "#]].assert_debug_eq(&(results, decoded));
}

#[test]
fn different_mix2() {
    let first = generated::DifferentMix2 {
        iae: CellRef::new(generated::IntAndEither32OrRef64 {
            op: bi(777),
            i32or_ref: generated::UnionTy33::Variant0(bi(2983)),
            query_id_maybe_ref: None,
        }),
        tic: generated::TwoInts32AndCoins {
            op: bi(123),
            amount: bi(829_290_000),
        },
        rest: slice_44_with_ref_45(),
    };
    let second = generated::DifferentMix2 {
        iae: CellRef::new(generated::IntAndEither32OrRef64 {
            op: bi(778),
            i32or_ref: generated::UnionTy33::Variant1(CellRef::new(generated::Inner2 {
                i64_in_ref: bi(9_919_992),
            })),
            query_id_maybe_ref: Some(CellRef::new(generated::Inner1 {
                query_id_ref: bi(889_477),
            })),
        }),
        tic: generated::TwoInts32AndCoins {
            op: bi(123),
            amount: bi(500_000),
        },
        rest: owned("x{}", vec![]),
    };
    expect![[r#"
        (
            (
                [
                    "x{0000007B4316DF6100000002C}\n x{00000309000005D3A_}\n x{0000002D}",
                    "x{0000007B4316DF6100000002C}\n x{00000309000005D3A_}\n x{0000002D}",
                    "x{0000007B4316DF6100000002C}\n x{00000309000005D3A_}\n x{0000002D}",
                    "x{0000007B4316DF6100000002C}\n x{00000309000005D3A_}\n x{0000002D}",
                ],
                DifferentMix2 {
                    iae: CellRef {
                        ref: IntAndEither32OrRef64 {
                            op: 777,
                            i32or_ref: Variant0(
                                2983,
                            ),
                            query_id_maybe_ref: None,
                        },
                    },
                    tic: TwoInts32AndCoins {
                        op: 123,
                        amount: 829290000,
                    },
                    rest: OwnedSlice {
                        range: CellSliceRange {
                            bits_start: 0,
                            bits_end: 32,
                            refs_start: 0,
                            refs_end: 1,
                        },
                        cell: Cell {
                            ty: Ordinary,
                            hash: b09efb8892624734260b4a330042f7f809bff49499942bb9ab6b07118c5ee88c,
                        },
                    },
                },
            ),
            (
                [
                    "x{0000007B307A120}\n x{0000030AE_}\n  x{0000000000975DF8}\n  x{00000000000D9285}",
                    "x{0000007B307A120}\n x{0000030AE_}\n  x{0000000000975DF8}\n  x{00000000000D9285}",
                    "x{0000007B307A120}\n x{0000030AE_}\n  x{0000000000975DF8}\n  x{00000000000D9285}",
                    "x{0000007B307A120}\n x{0000030AE_}\n  x{0000000000975DF8}\n  x{00000000000D9285}",
                ],
                DifferentMix2 {
                    iae: CellRef {
                        ref: IntAndEither32OrRef64 {
                            op: 778,
                            i32or_ref: Variant1(
                                CellRef {
                                    ref: Inner2 {
                                        i64_in_ref: 9919992,
                                    },
                                },
                            ),
                            query_id_maybe_ref: Some(
                                CellRef {
                                    ref: Inner1 {
                                        query_id_ref: 889477,
                                    },
                                },
                            ),
                        },
                    },
                    tic: TwoInts32AndCoins {
                        op: 123,
                        amount: 500000,
                    },
                    rest: OwnedSlice {
                        range: CellSliceRange {
                            bits_start: 0,
                            bits_end: 0,
                            refs_start: 0,
                            refs_end: 0,
                        },
                        cell: Cell {
                            ty: Ordinary,
                            hash: 96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7,
                        },
                    },
                },
            ),
        )
    "#]].assert_debug_eq(&(run(first), run(second)));
}

#[test]
fn different_mix3() {
    let results = [
        run(generated::DifferentMix3::create(
            generated::UnionTy90::Variant0(CellRef::new(generated::TwoInts32AndCoins {
                op: bi(123),
                amount: bi(80_000),
            })),
            Some(generated::TwoInts32AndCoins {
                op: bi(456),
                amount: bi(0),
            }),
        )),
        run(generated::DifferentMix3 {
            bod: generated::UnionTy90::Variant0(CellRef::new(generated::TwoInts32AndCoins {
                op: bi(124),
                amount: bi(10),
            })),
            tim: None,
            pairm: Some((bi(100_000), bi(100_000))),
        }),
        run(generated::DifferentMix3 {
            bod: generated::UnionTy90::Variant1(CellRef::new(generated::JustInt32 {
                value: bi(255),
            })),
            tim: None,
            pairm: Some((bi(90), bi(90))),
        }),
        run(generated::DifferentMix3 {
            bod: generated::UnionTy90::Variant1(CellRef::new(generated::JustInt32 {
                value: bi(510),
            })),
            tim: Some(generated::TwoInts32AndCoins {
                op: bi(567),
                amount: bi(9_392_843_922),
            }),
            pairm: Some((bi(81_923), bi(81_923))),
        }),
    ];
    let decoded4 = generated::DifferentMix3::from_cell(&make_cell(
        "x{C000008DD408BF6DB24A000280060000000000028007_}",
        vec![make_cell("x{000001FE}", vec![])],
    ))
    .expect("fourth value must decode");
    let decoded2 = generated::DifferentMix3::from_cell(&make_cell(
        "x{200030D400000000000030D41_}",
        vec![make_cell("x{0000007C10A}", vec![])],
    ))
    .expect("second value must decode");
    expect![[r#"
        (
            [
                (
                    [
                        "x{4000007201_}\n x{0000007B3013880}",
                        "x{4000007201_}\n x{0000007B3013880}",
                        "x{4000007201_}\n x{0000007B3013880}",
                        "x{4000007201_}\n x{0000007B3013880}",
                    ],
                    DifferentMix3 {
                        bod: Variant0(
                            CellRef {
                                ref: TwoInts32AndCoins {
                                    op: 123,
                                    amount: 80000,
                                },
                            },
                        ),
                        tim: Some(
                            TwoInts32AndCoins {
                                op: 456,
                                amount: 0,
                            },
                        ),
                        pairm: None,
                    },
                ),
                (
                    [
                        "x{200030D400000000000030D41_}\n x{0000007C10A}",
                        "x{200030D400000000000030D41_}\n x{0000007C10A}",
                        "x{200030D400000000000030D41_}\n x{0000007C10A}",
                        "x{200030D400000000000030D41_}\n x{0000007C10A}",
                    ],
                    DifferentMix3 {
                        bod: Variant0(
                            CellRef {
                                ref: TwoInts32AndCoins {
                                    op: 124,
                                    amount: 10,
                                },
                            },
                        ),
                        tim: None,
                        pairm: Some(
                            (
                                100000,
                                100000,
                            ),
                        ),
                    },
                ),
                (
                    [
                        "x{A000000B400000000000000B5_}\n x{000000FF}",
                        "x{A000000B400000000000000B5_}\n x{000000FF}",
                        "x{A000000B400000000000000B5_}\n x{000000FF}",
                        "x{A000000B400000000000000B5_}\n x{000000FF}",
                    ],
                    DifferentMix3 {
                        bod: Variant1(
                            CellRef {
                                ref: JustInt32 {
                                    value: 255,
                                },
                            },
                        ),
                        tim: None,
                        pairm: Some(
                            (
                                90,
                                90,
                            ),
                        ),
                    },
                ),
                (
                    [
                        "x{C000008DD408BF6DB24A000280060000000000028007_}\n x{000001FE}",
                        "x{C000008DD408BF6DB24A000280060000000000028007_}\n x{000001FE}",
                        "x{C000008DD408BF6DB24A000280060000000000028007_}\n x{000001FE}",
                        "x{C000008DD408BF6DB24A000280060000000000028007_}\n x{000001FE}",
                    ],
                    DifferentMix3 {
                        bod: Variant1(
                            CellRef {
                                ref: JustInt32 {
                                    value: 510,
                                },
                            },
                        ),
                        tim: Some(
                            TwoInts32AndCoins {
                                op: 567,
                                amount: 9392843922,
                            },
                        ),
                        pairm: Some(
                            (
                                81923,
                                81923,
                            ),
                        ),
                    },
                ),
            ],
            DifferentMix3 {
                bod: Variant1(
                    CellRef {
                        ref: JustInt32 {
                            value: 510,
                        },
                    },
                ),
                tim: Some(
                    TwoInts32AndCoins {
                        op: 567,
                        amount: 9392843922,
                    },
                ),
                pairm: Some(
                    (
                        81923,
                        81923,
                    ),
                ),
            },
            DifferentMix3 {
                bod: Variant0(
                    CellRef {
                        ref: TwoInts32AndCoins {
                            op: 124,
                            amount: 10,
                        },
                    },
                ),
                tim: None,
                pairm: Some(
                    (
                        100000,
                        100000,
                    ),
                ),
            },
        )
    "#]]
    .assert_debug_eq(&(results, decoded4, decoded2));
}

#[test]
fn with_variadic_ints() {
    let value = generated::WithVariadicInts {
        ui16: (BigInt::from(1_u8) << 120_usize) - 1,
        i16: -(BigInt::from(1_u8) << 118_usize),
        ui32: (BigInt::from(1_u8) << 248_usize) - 1,
        i32: -(BigInt::from(1_u8) << 246_usize),
    };
    expect![[r#"
        (
            (
                [
                    "x{FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC00000000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000002_}",
                    "x{FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC00000000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000002_}",
                    "x{FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC00000000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000002_}",
                    "x{FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC00000000000000000000000000000FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000000000000000000000000000000000000000000000000000000002_}",
                ],
                WithVariadicInts {
                    ui16: 1329227995784915872903807060280344575,
                    i16: -332306998946228968225951765070086144,
                    ui32: 452312848583266388373324160190187140051835877600158453279131187530910662655,
                    i32: -113078212145816597093331040047546785012958969400039613319782796882727665664,
                },
            ),
            (
                [
                    "x{00002_}",
                    "x{00002_}",
                    "x{00002_}",
                    "x{00002_}",
                ],
                WithVariadicInts {
                    ui16: 0,
                    i16: 0,
                    ui32: 0,
                    i32: 0,
                },
            ),
        )
    "#]].assert_debug_eq(&(
        run(value),
        run(generated::WithVariadicInts {
            ui16: bi(0),
            i16: bi(0),
            ui32: bi(0),
            i32: bi(0),
        }),
    ));
}

#[test]
fn edge_case_ints() {
    let edge = generated::EdgeCaseInts::default();
    let encoded = edge.to_cell().expect("edge values must encode");
    let mut manual = CellBuilder::new();
    acton_client::cell::store_fixed_int(&mut manual, &edge.max_uint, 256, false)
        .expect("max uint must fit");
    acton_client::cell::store_fixed_int(&mut manual, &edge.max_int, 257, true)
        .expect("max int must fit");
    acton_client::cell::store_fixed_int(&mut manual, &edge.min_int, 257, true)
        .expect("min int must fit");
    let manual = manual.build().expect("manual cell must build");
    let decoded = generated::EdgeCaseInts::from_cell(&encoded).expect("value must decode");
    expect![[r#"
        (
            "x{FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC0000000000000000000000000000000000000000000000000000000000000002_}",
            "x{FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC0000000000000000000000000000000000000000000000000000000000000002_}",
            true,
            EdgeCaseInts {
                max_uint: 115792089237316195423570985008687907853269984665640564039457584007913129639935,
                max_int: 115792089237316195423570985008687907853269984665640564039457584007913129639935,
                min_int: -115792089237316195423570985008687907853269984665640564039457584007913129639936,
            },
        )
    "#]].assert_debug_eq(&(
        cell_tree(&encoded),
        cell_tree(&manual),
        encoded.repr_hash() == manual.repr_hash(),
        decoded,
    ));
}

#[test]
fn write_with_builder_read_with_other() {
    let mut rest = CellBuilder::new();
    rest.store_u32(55).expect("value must fit");
    rest.store_bit(false).expect("maybe-ref tag must fit");
    let written = generated::WriteWithBuilder { f1: bi(10), rest }
        .to_cell()
        .expect("builder-backed value must encode");
    let read =
        generated::ReadWrittenWithBuilder::from_cell(&written).expect("other shape must decode");

    let written_slice = generated::WriteWithSlice {
        f1: bi(10),
        rest: owned("x{FFFF}", vec![]),
    }
    .to_cell()
    .expect("slice-backed value must encode");
    let mut slice = written_slice.as_slice().expect("cell must be readable");
    slice.skip_first(32, 0).expect("prefix must be present");
    let tail = slice.load_u16().expect("tail must be present");
    expect![[r#"
        (
            "x{0000000A000000374_}",
            ReadWrittenWithBuilder {
                f1: 10,
                some_int: 55,
                some_cell: None,
            },
            "x{0000000AFFFF}",
            65535,
        )
    "#]]
    .assert_debug_eq(&(cell_tree(&written), read, cell_tree(&written_slice), tail));
}

#[test]
fn rest_is_builder_or_remaining() {
    let addr = std_addr("9:527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8");
    let mut remaining = CellBuilder::new();
    remaining.store_u32(5).expect("value must fit");
    acton_client::cell::store_tlb(&mut remaining, &addr).expect("address must fit");
    remaining.store_bit(true).expect("maybe-ref tag must fit");
    let mut int_ref = CellBuilder::new();
    acton_client::cell::store_fixed_int(&mut int_ref, &bi(123), 32, true)
        .expect("referenced int must fit");
    remaining
        .store_reference(int_ref.build().expect("int ref must build"))
        .expect("int ref must fit");
    remaining
        .store_reference(
            generated::JustAddress { addr: addr.clone() }
                .to_cell()
                .expect("address ref must encode"),
        )
        .expect("address ref must fit");

    let value = generated::ReadWriteRest {
        f1: bi(60),
        f2: bi(50_000_000),
        rest: remaining,
    };
    let mut encoded = CellBuilder::new();
    acton_client::cell::store_fixed_int(&mut encoded, &value.f1, 32, true).expect("f1 must fit");
    acton_client::cell::store_var_int(&mut encoded, &value.f2, 4, false).expect("coins must fit");
    encoded
        .store_builder(&value.rest)
        .expect("remaining builder must fit");
    let encoded = encoded.build().expect("cell must build");
    let mut slice = encoded.as_slice().expect("cell must be readable");
    let read = generated::load_read_rest_remaining(&mut slice).expect("value must decode");
    let mut tail_slice = read.rest.as_slice().expect("remainder must be readable");
    let tail = generated::Tail224::load_from(&mut tail_slice).expect("tail must decode");
    expect![[r#"
        (
            "x{0000003C402FAF08000000005812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1}\n x{0000007B}\n x{812A4F2C9AAB9F4DD6E63E97F80FD3A04A13012FDF0A0AA33D0A730C4F37B0801B1_}",
            60,
            50000000,
            Tail224 {
                ji: JustInt32 {
                    value: 5,
                },
                addr: StdAddr {
                    anycast: None,
                    workchain: 9,
                    address: 527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8,
                },
                ref1: Some(
                    CellRef {
                        ref: 123,
                    },
                ),
                ref2: CellRef {
                    ref: JustAddress {
                        addr: StdAddr {
                            anycast: None,
                            workchain: 9,
                            address: 527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8,
                        },
                    },
                },
            },
        )
    "#]].assert_debug_eq(&(cell_tree(&encoded), read.f1, read.f2, tail));
}

#[test]
fn mid_is_builder_or_bits_n() {
    let mut bits40 = CellBuilder::new();
    let bits40_slice = owned("x{0000FFFF00}", vec![]);
    bits40
        .store_slice(
            bits40_slice
                .as_slice()
                .expect("40-bit slice must be readable"),
        )
        .expect("40 bits must fit");
    let value = generated::ReadWriteMid {
        f1: bi(5),
        mid: bits40,
        f3: bi(50_000_000),
    };
    let mut cell = CellBuilder::new();
    acton_client::cell::store_fixed_int(&mut cell, &value.f1, 32, true).expect("f1 must fit");
    cell.store_builder(&value.mid)
        .expect("middle bits must fit");
    acton_client::cell::store_var_int(&mut cell, &value.f3, 4, false).expect("coins must fit");
    let cell = cell.build().expect("cell must build");
    let mut slice = cell.as_slice().expect("cell must be readable");
    let f1 = acton_client::cell::load_fixed_int(&mut slice, 32, true).expect("f1 must be present");
    let mut middle = slice.load_prefix(40, 0).expect("middle bits must exist");
    let middle_bits = middle.size_bits();
    let middle_value = middle.load_u32().expect("first 32 middle bits must exist");
    let f3 = acton_client::cell::load_var_int(&mut slice, 4, false).expect("f3 must be present");
    expect![[r#"
        (
            "x{000000050000FFFF00402FAF080}",
            5,
            40,
            65535,
            50000000,
        )
    "#]]
    .assert_debug_eq(&(cell_tree(&cell), f1, middle_bits, middle_value, f3));
}

#[test]
fn multiple_remainers() {
    let value = generated::WithTwoRestFields::from_cell(&make_cell("x{00000001FFFF}", vec![]))
        .expect("value must decode");
    let rest1 = value.rest1.as_slice().expect("rest1 must be readable");
    let rest2 = value.rest2.as_slice().expect("rest2 must be readable");
    expect![[r"
        (
            1,
            16,
            0,
        )
    "]]
    .assert_debug_eq(&(value.i32, rest1.size_bits(), rest2.size_bits()));
}

#[test]
fn mutating_remainder() {
    let cell = make_cell("x{00000001FFFF}", vec![]);
    let mut slice = cell.as_slice().expect("cell must be readable");
    let value = generated::IntAndRestInlineCell::load_from(&mut slice).expect("value must decode");
    let rest_bits = value
        .rest
        .as_slice()
        .expect("remainder must be readable")
        .size_bits();
    expect![[r"
        (
            0,
            0,
            16,
        )
    "]]
    .assert_debug_eq(&(slice.size_bits(), slice.size_refs(), rest_bits));
}

#[test]
fn simple_enums() {
    expect![[r#"
        [
            (
                [
                    "x{9C00}",
                    "x{9C00}",
                    "x{9C00}",
                    "x{9C00}",
                ],
                WithEnums {
                    e1: EStoredAsInt8(
                        -100,
                    ),
                    e2: EStoredAsUint1(
                        0,
                    ),
                    rem: 0,
                },
            ),
            (
                [
                    "x{0000}",
                    "x{0000}",
                    "x{0000}",
                    "x{0000}",
                ],
                WithEnums {
                    e1: EStoredAsInt8(
                        0,
                    ),
                    e2: EStoredAsUint1(
                        0,
                    ),
                    rem: 0,
                },
            ),
            (
                [
                    "x{6481}",
                    "x{6481}",
                    "x{6481}",
                    "x{6481}",
                ],
                WithEnums {
                    e1: EStoredAsInt8(
                        100,
                    ),
                    e2: EStoredAsUint1(
                        1,
                    ),
                    rem: 1,
                },
            ),
        ]
    "#]]
    .assert_debug_eq(&[
        run(generated::WithEnums {
            e1: generated::EStoredAsInt8::m100(),
            e2: generated::EStoredAsUint1::zero(),
            rem: bi(0),
        }),
        run(generated::WithEnums {
            e1: generated::EStoredAsInt8::z(),
            e2: generated::EStoredAsUint1::zero(),
            rem: bi(0),
        }),
        run(generated::WithEnums {
            e1: generated::EStoredAsInt8::p100(),
            e2: generated::EStoredAsUint1::one(),
            rem: bi(1),
        }),
    ]);
}

#[test]
fn with_more_tricky_cells() {
    expect![[r#"
        (
            [
                "x{809FE_}\n x{80000000000000044_}\n x{}\n  x{2_}\n x{0000007B0000000000040000}\n x{40042_}",
                "x{809FE_}\n x{80000000000000044_}\n x{}\n  x{2_}\n x{0000007B0000000000040000}\n x{40042_}",
                "x{809FE_}\n x{80000000000000044_}\n x{}\n  x{2_}\n x{0000007B0000000000040000}\n x{40042_}",
                "x{809FE_}\n x{80000000000000044_}\n x{}\n  x{2_}\n x{0000007B0000000000040000}\n x{40042_}",
            ],
            WithMoreTrickyCells {
                before: -128,
                tricky: MoreTrickyCells {
                    c1: CellRef {
                        ref: Variant1(
                            8,
                        ),
                    },
                    c2: CellRef {
                        ref: CellRef {
                            ref: None,
                        },
                    },
                    c3: Some(
                        CellRef {
                            ref: TwoInts32And64 {
                                op: 123,
                                query_id: 262144,
                            },
                        },
                    ),
                    c4: Variant0(
                        CellRef {
                            ref: Variant1(
                                16,
                            ),
                        },
                    ),
                },
                after: 127,
            },
        )
    "#]].assert_debug_eq(&run(generated::WithMoreTrickyCells {
        before: bi(-128),
        tricky: generated::MoreTrickyCells {
            c1: CellRef::new(generated::UnionTy119::Variant1(bi(8))),
            c2: CellRef::new(CellRef::new(AnyAddr::None)),
            c3: Some(CellRef::new(generated::TwoInts32And64 {
                op: bi(123),
                query_id: BigInt::from(1_u8) << 18_usize,
            })),
            c4: generated::UnionTy133::Variant0(CellRef::new(generated::UnionTy117::Variant1(bi(
                16,
            )))),
        },
        after: bi(127),
    }));
}

#[test]
fn with_more_tricky_addresses1() {
    let addr = || std_addr("0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e");
    let results = [
        run(generated::WithMoreTrickyAddresses1 {
            a1: Some(std_addr("UQDKbjIcfM6ezt8KjKJJLshZJJSqX7XOA4ff-W72r5gqPuwA")),
            a2: generated::UnionTy137::Variant2(generated::MaybeNothing {}),
            a3: generated::UnionTy140::Variant1(generated::MaybeJust { value: addr() }),
            a4: Some(CellRef::new(None)),
            a5: generated::UnionTy143::Variant0(std_addr(
                "0:0000000000000000000000000000000000000000000000000000000000000000",
            )),
        }),
        run(generated::WithMoreTrickyAddresses1 {
            a1: None,
            a2: generated::UnionTy137::Variant0(addr()),
            a3: generated::UnionTy140::Variant0(generated::TwoInts32And64 {
                op: bi(123),
                query_id: bi(456),
            }),
            a4: None,
            a5: generated::UnionTy143::Variant1(CellRef::new(AnyAddr::None)),
        }),
    ];
    let error = generated::WithMoreTrickyAddresses1::from_cell(&make_cell("b{0011}", vec![]))
        .expect_err("invalid a2 prefix must fail")
        .to_string();
    expect![[r#"
        (
            [
                (
                    [
                        "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D6006537190E3E674F676F8546512497642C924A552FDAE701C3EFFCB77B57CC151F50000000000000000000000000000000000000000000000000000000000000000002_}\n x{2_}",
                        "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D6006537190E3E674F676F8546512497642C924A552FDAE701C3EFFCB77B57CC151F50000000000000000000000000000000000000000000000000000000000000000002_}\n x{2_}",
                        "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D6006537190E3E674F676F8546512497642C924A552FDAE701C3EFFCB77B57CC151F50000000000000000000000000000000000000000000000000000000000000000002_}\n x{2_}",
                        "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D6006537190E3E674F676F8546512497642C924A552FDAE701C3EFFCB77B57CC151F50000000000000000000000000000000000000000000000000000000000000000002_}\n x{2_}",
                    ],
                    WithMoreTrickyAddresses1 {
                        a1: Some(
                            StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        ),
                        a2: Variant2(
                            MaybeNothing,
                        ),
                        a3: Variant1(
                            MaybeJust {
                                value: StdAddr {
                                    anycast: None,
                                    workchain: 0,
                                    address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                                },
                            },
                        ),
                        a4: Some(
                            CellRef {
                                ref: None,
                            },
                        ),
                        a5: Variant0(
                            StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: 0000000000000000000000000000000000000000000000000000000000000000,
                            },
                        ),
                    },
                ),
                (
                    [
                        "x{080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000007B00000000000001C86_}\n x{2_}",
                        "x{080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000007B00000000000001C86_}\n x{2_}",
                        "x{080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000007B00000000000001C86_}\n x{2_}",
                        "x{080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547C0000007B00000000000001C86_}\n x{2_}",
                    ],
                    WithMoreTrickyAddresses1 {
                        a1: None,
                        a2: Variant0(
                            StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        ),
                        a3: Variant0(
                            TwoInts32And64 {
                                op: 123,
                                query_id: 456,
                            },
                        ),
                        a4: None,
                        a5: Variant1(
                            CellRef {
                                ref: None,
                            },
                        ),
                    },
                ),
            ],
            "Incorrect prefix for 'WithMoreTrickyAddresses1.a2': none of variants matched",
        )
    "#]].assert_debug_eq(&(results, error));
}

#[test]
fn with_more_tricky_addresses2() {
    let addr = || std_addr("0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e");
    expect![[r#"
        [
            (
                [
                    "x{C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E600000000000000F680194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                    "x{C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E600000000000000F680194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                    "x{C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E600000000000000F680194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                    "x{C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E600000000000000F680194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                ],
                WithMoreTrickyAddresses2 {
                    a1: Variant1(
                        MaybeJust {
                            value: StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        },
                    ),
                    a2: Variant1(
                        MaybeJust {
                            value: Some(
                                StdAddr {
                                    anycast: None,
                                    workchain: 0,
                                    address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                                },
                            ),
                        },
                    ),
                    a3: None,
                    a4: Variant1(
                        123,
                    ),
                    a5: Variant0(
                        StdAddr {
                            anycast: None,
                            workchain: 0,
                            address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                        },
                    ),
                },
            ),
            (
                [
                    "x{4C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E6_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                    "x{4C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E6_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                    "x{4C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E6_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                    "x{4C00CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3E6_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}",
                ],
                WithMoreTrickyAddresses2 {
                    a1: Variant0(
                        MaybeNothing,
                    ),
                    a2: Variant1(
                        MaybeJust {
                            value: None,
                        },
                    ),
                    a3: Some(
                        JustAddress {
                            addr: StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        },
                    ),
                    a4: Variant2(
                        (),
                    ),
                    a5: Variant1(
                        CellRef {
                            ref: StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        },
                    ),
                },
            ),
            (
                [
                    "x{1400CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC_}\n x{800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669_}",
                    "x{1400CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC_}\n x{800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669_}",
                    "x{1400CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC_}\n x{800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669_}",
                    "x{1400CA6E321C7CCE9ECEDF0A8CA2492EC8592494AA5FB5CE0387DFF96EF6AF982A3EC_}\n x{800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669_}",
                ],
                WithMoreTrickyAddresses2 {
                    a1: Variant0(
                        MaybeNothing,
                    ),
                    a2: Variant0(
                        MaybeNothing,
                    ),
                    a3: None,
                    a4: Variant0(
                        StdAddr {
                            anycast: None,
                            workchain: 0,
                            address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                        },
                    ),
                    a5: Variant1(
                        CellRef {
                            ref: StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: 312f4a05d7eccd82298f2ad85b0e7612e318fde2b1ed7cd44deff3ddd6bd8b34,
                            },
                        },
                    ),
                },
            ),
        ]
    "#]].assert_debug_eq(&[
        run(generated::WithMoreTrickyAddresses2 {
            a1: maybe_just(addr()),
            a2: maybe_just(Some(addr())),
            a3: None,
            a4: generated::UnionTy151::Variant1(bi(123)),
            a5: generated::UnionTy153::Variant0(addr()),
        }),
        run(generated::WithMoreTrickyAddresses2 {
            a1: maybe_nothing(),
            a2: maybe_just(None),
            a3: Some(generated::JustAddress { addr: addr() }),
            a4: generated::UnionTy151::Variant2(()),
            a5: generated::UnionTy153::Variant1(CellRef::new(addr())),
        }),
        run(generated::WithMoreTrickyAddresses2 {
            a1: maybe_nothing(),
            a2: maybe_nothing(),
            a3: None,
            a4: generated::UnionTy151::Variant0(addr()),
            a5: generated::UnionTy153::Variant1(CellRef::new(std_addr(
                "EQAxL0oF1-zNgimPKthbDnYS4xj94rHtfNRN7_Pd1r2LNNv3",
            ))),
        }),
    ]);
}

#[test]
fn with_any_address() {
    let addr = || any_std("0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e");
    expect![[r#"
        [
            (
                [
                    "x{282B20080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669}\n x{2_}\n x{4_}",
                    "x{282B20080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669}\n x{2_}\n x{4_}",
                    "x{282B20080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669}\n x{2_}\n x{4_}",
                    "x{282B20080194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D800625E940BAFD99B04531E55B0B61CEC25C631FBC563DAF9A89BDFE7BBAD7B1669}\n x{2_}\n x{4_}",
                ],
                WithAnyAddress {
                    a1: None,
                    a2: Some(
                        Ext(
                            ExtAddr {
                                data_bit_len: Uint9(
                                    10,
                                ),
                                data: [
                                    200,
                                    0,
                                ],
                            },
                        ),
                    ),
                    a3: Variant0(
                        None,
                    ),
                    a4: Variant0(
                        StdAddr {
                            anycast: None,
                            workchain: 0,
                            address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                        },
                    ),
                    a5: Variant1(
                        MaybeJust {
                            value: Std(
                                StdAddr {
                                    anycast: None,
                                    workchain: 0,
                                    address: 312f4a05d7eccd82298f2ad85b0e7612e318fde2b1ed7cd44deff3ddd6bd8b34,
                                },
                            ),
                        },
                    ),
                    a6: CellRef {
                        ref: None,
                    },
                    a7: Some(
                        CellRef {
                            ref: None,
                        },
                    ),
                },
            ),
            (
                [
                    "x{408820A01300329B8C871F33A7B3B7C2A328924BB21649252A97ED7380E1F7FE5BBDABE60A8FA82023_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}\n x{9_}",
                    "x{408820A01300329B8C871F33A7B3B7C2A328924BB21649252A97ED7380E1F7FE5BBDABE60A8FA82023_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}\n x{9_}",
                    "x{408820A01300329B8C871F33A7B3B7C2A328924BB21649252A97ED7380E1F7FE5BBDABE60A8FA82023_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}\n x{9_}",
                    "x{408820A01300329B8C871F33A7B3B7C2A328924BB21649252A97ED7380E1F7FE5BBDABE60A8FA82023_}\n x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D_}\n x{9_}",
                ],
                WithAnyAddress {
                    a1: Ext(
                        ExtAddr {
                            data_bit_len: Uint9(
                                4,
                            ),
                            data: [
                                64,
                            ],
                        },
                    ),
                    a2: None,
                    a3: Variant0(
                        Ext(
                            ExtAddr {
                                data_bit_len: Uint9(
                                    10,
                                ),
                                data: [
                                    1,
                                    0,
                                ],
                            },
                        ),
                    ),
                    a4: Variant1(
                        Std(
                            StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        ),
                    ),
                    a5: Variant1(
                        MaybeJust {
                            value: Ext(
                                ExtAddr {
                                    data_bit_len: Uint9(
                                        8,
                                    ),
                                    data: [
                                        8,
                                    ],
                                },
                            ),
                        },
                    ),
                    a6: CellRef {
                        ref: Std(
                            StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        ),
                    },
                    a7: Some(
                        CellRef {
                            ref: Some(
                                None,
                            ),
                        },
                    ),
                },
            ),
            (
                [
                    "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D000000000000007B8C_}\n x{415904_}\n x{A0AC82_}",
                    "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D000000000000007B8C_}\n x{415904_}\n x{A0AC82_}",
                    "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D000000000000007B8C_}\n x{415904_}\n x{A0AC82_}",
                    "x{80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D80194DC6438F99D3D9DBE151944925D90B2492954BF6B9C070FBFF2DDED5F30547D000000000000007B8C_}\n x{415904_}\n x{A0AC82_}",
                ],
                WithAnyAddress {
                    a1: Std(
                        StdAddr {
                            anycast: None,
                            workchain: 0,
                            address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                        },
                    ),
                    a2: Some(
                        Std(
                            StdAddr {
                                anycast: None,
                                workchain: 0,
                                address: ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e,
                            },
                        ),
                    ),
                    a3: Variant1(
                        123,
                    ),
                    a4: Variant1(
                        None,
                    ),
                    a5: Variant0(
                        MaybeNothing,
                    ),
                    a6: CellRef {
                        ref: Ext(
                            ExtAddr {
                                data_bit_len: Uint9(
                                    10,
                                ),
                                data: [
                                    200,
                                    0,
                                ],
                            },
                        ),
                    },
                    a7: Some(
                        CellRef {
                            ref: Some(
                                Ext(
                                    ExtAddr {
                                        data_bit_len: Uint9(
                                            10,
                                        ),
                                        data: [
                                            200,
                                            0,
                                        ],
                                    },
                                ),
                            ),
                        },
                    ),
                },
            ),
            (
                [
                    "x{2084_}\n x{2_}",
                    "x{2084_}\n x{2_}",
                    "x{2084_}\n x{2_}",
                    "x{2084_}\n x{2_}",
                ],
                WithAnyAddress {
                    a1: None,
                    a2: Some(
                        None,
                    ),
                    a3: Variant0(
                        None,
                    ),
                    a4: Variant1(
                        None,
                    ),
                    a5: Variant0(
                        MaybeNothing,
                    ),
                    a6: CellRef {
                        ref: None,
                    },
                    a7: None,
                },
            ),
        ]
    "#]].assert_debug_eq(&[
        run(generated::WithAnyAddress {
            a1: AnyAddr::None,
            a2: Some(any_ext(800, 10)),
            a3: generated::UnionTy156::Variant0(AnyAddr::None),
            a4: generated::UnionTy157::Variant0(std_addr(
                "0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e",
            )),
            a5: maybe_just(any_std("EQAxL0oF1-zNgimPKthbDnYS4xj94rHtfNRN7_Pd1r2LNNv3")),
            a6: CellRef::new(AnyAddr::None),
            a7: Some(CellRef::new(None)),
        }),
        run(generated::WithAnyAddress {
            a1: any_ext(4, 4),
            a2: None,
            a3: generated::UnionTy156::Variant0(any_ext(4, 10)),
            a4: generated::UnionTy157::Variant1(addr()),
            a5: maybe_just(any_ext(8, 8)),
            a6: CellRef::new(addr()),
            a7: Some(CellRef::new(Some(AnyAddr::None))),
        }),
        run(generated::WithAnyAddress {
            a1: addr(),
            a2: Some(addr()),
            a3: generated::UnionTy156::Variant1(bi(123)),
            a4: generated::UnionTy157::Variant1(AnyAddr::None),
            a5: maybe_nothing(),
            a6: CellRef::new(any_ext(800, 10)),
            a7: Some(CellRef::new(Some(any_ext(800, 10)))),
        }),
        run(generated::WithAnyAddress {
            a1: AnyAddr::None,
            a2: Some(AnyAddr::None),
            a3: generated::UnionTy156::Variant0(AnyAddr::None),
            a4: generated::UnionTy157::Variant1(AnyAddr::None),
            a5: maybe_nothing(),
            a6: CellRef::new(AnyAddr::None),
            a7: None,
        }),
    ]);
}
