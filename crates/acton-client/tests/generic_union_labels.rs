use acton_client::__private::tycho_types::cell::CellBuilder;
use acton_client::{
    AbiLoad, AbiStore, Cell, ContractProvider, DynamicAbi, DynamicValue, OwnedSlice, SendOptions,
    StdAddr, Tuple,
};
use expect_test::expect;
use num_bigint::BigInt;
use std::str::FromStr;

#[allow(unreachable_pub, clippy::significant_drop_tightening)]
mod support;

use support::{TvmContractProvider, TvmSender};

#[acton_client::contract(abi = "tests/fixtures/upstream/generic-union-labels.abi.json")]
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
    let mut cell = value.to_cell().expect("value must encode");
    let mut decoded = T::from_cell(&cell).expect("value must decode");
    for _ in 1..2 {
        cell = decoded.to_cell().expect("decoded value must encode");
        decoded = T::from_cell(&cell).expect("value must decode again");
    }
    (cell_hex(&cell), decoded)
}

fn dynamic_abi() -> DynamicAbi {
    DynamicAbi::from_json(generated::ABI_JSON).expect("GenericUnionLabels ABI must parse")
}

fn dynamic_round_trip(type_name: &str, value: &DynamicValue) -> (String, bool) {
    let abi = dynamic_abi();
    let ty_idx = abi
        .declaration_type_index(type_name)
        .unwrap_or_else(|| panic!("dynamic ABI declaration `{type_name}` must exist"));
    let mut current = value.clone();
    let mut last_cell = Cell::default();
    let mut matches = true;
    for _ in 0..2 {
        last_cell = abi
            .pack_to_cell(ty_idx, &current)
            .expect("dynamic value must encode");
        let mut slice = last_cell.as_slice().expect("dynamic cell must be readable");
        let decoded = abi
            .unpack_from_slice(ty_idx, &mut slice)
            .expect("dynamic value must decode");
        acton_client::cell::ensure_empty(&slice).expect("dynamic slice must be exhausted");
        matches &= decoded == current;
        current = decoded;
    }
    (cell_hex(&last_cell), matches && current == *value)
}

fn empty_cell() -> Cell {
    CellBuilder::new().build().expect("empty cell must build")
}

fn relaxed_destination() -> StdAddr {
    StdAddr::from_str("0:527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8")
        .expect("upstream relaxed-message destination must parse")
}

fn make_relaxed_message() -> generated::TlbMessageRelaxedGeneric {
    generated::TlbMessageRelaxedGeneric {
        info: generated::TlbIntMsgInfoRelaxed {
            bounce: true,
            dest: relaxed_destination(),
            value: BigInt::from(50_000_000),
        },
        init: generated::UnionTy64::Variant0(generated::TlbNone {}),
        body: OwnedSlice::full(empty_cell()),
    }
}

fn make_dynamic_relaxed_message() -> DynamicValue {
    DynamicValue::structure(
        "TlbMessageRelaxedGeneric",
        [
            (
                "info",
                DynamicValue::structure(
                    "TlbIntMsgInfoRelaxed",
                    [
                        ("bounce", DynamicValue::Bool(true)),
                        ("dest", DynamicValue::from(relaxed_destination())),
                        ("value", DynamicValue::from(50_000_000_u64)),
                    ],
                ),
            ),
            (
                "init",
                DynamicValue::structure("TlbNone", Vec::<(&str, DynamicValue)>::new()),
            ),
            ("body", DynamicValue::Slice(OwnedSlice::full(empty_cell()))),
        ],
    )
}

async fn deployed_contract() -> generated::GenericUnionLabels<TvmContractProvider> {
    let contract =
        generated::GenericUnionLabels::from_storage(&generated::GenericUnionLabelsStorage {
            dummy: BigInt::from(0),
        })
        .expect("GenericUnionLabels state init must build");
    let provider = TvmContractProvider::new(contract.address().clone())
        .expect("local TVM provider must initialize");
    let contract = contract.with_provider(provider);
    let transaction = contract
        .send_deploy(
            &TvmSender::new("deployer", 0xd0),
            BigInt::from(50_000_000_u64),
            SendOptions::default(),
        )
        .await
        .expect("GenericUnionLabels deploy transaction must execute");
    assert!(
        transaction.success,
        "GenericUnionLabels deployment must succeed: {transaction:#?}"
    );
    contract
}

#[tokio::test]
async fn serializes_generic_unions_with_wrapper_and_dynamic_codecs() {
    let _contract = deployed_contract().await;
    let pair_left = generated::MsgPair {
        value: generated::GenericPair {
            value: generated::UnionTy18::Variant0(BigInt::from(10)),
        },
    };
    let pair_right = generated::MsgPair {
        value: generated::GenericPair {
            value: generated::UnionTy18::Variant1(BigInt::from(11)),
        },
    };
    let or_int16_left = generated::MsgOrInt16 {
        value: generated::GenericOrInt8 {
            value: generated::UnionTy26::Variant0(BigInt::from(13)),
        },
    };
    let or_int16_right = generated::MsgOrInt16 {
        value: generated::GenericOrInt8 {
            value: generated::UnionTy26::Variant1(BigInt::from(14)),
        },
    };
    let alias_left = generated::MsgAliasInt16 {
        value: generated::UnionTy26::Variant0(BigInt::from(16)),
    };
    let alias_right = generated::MsgAliasInt16 {
        value: generated::UnionTy26::Variant1(BigInt::from(17)),
    };
    let dynamic_values = [
        (
            "MsgPair",
            DynamicValue::structure(
                "MsgPair",
                [(
                    "value",
                    DynamicValue::structure(
                        "GenericPair",
                        [("value", DynamicValue::union("T1", DynamicValue::from(10)))],
                    ),
                )],
            ),
        ),
        (
            "MsgPair",
            DynamicValue::structure(
                "MsgPair",
                [(
                    "value",
                    DynamicValue::structure(
                        "GenericPair",
                        [("value", DynamicValue::union("T2", DynamicValue::from(11)))],
                    ),
                )],
            ),
        ),
        (
            "MsgOrInt16",
            DynamicValue::structure(
                "MsgOrInt16",
                [(
                    "value",
                    DynamicValue::structure(
                        "GenericOrInt8",
                        [("value", DynamicValue::union("T", DynamicValue::from(13)))],
                    ),
                )],
            ),
        ),
        (
            "MsgOrInt16",
            DynamicValue::structure(
                "MsgOrInt16",
                [(
                    "value",
                    DynamicValue::structure(
                        "GenericOrInt8",
                        [("value", DynamicValue::union("int8", DynamicValue::from(14)))],
                    ),
                )],
            ),
        ),
        (
            "MsgAliasInt16",
            DynamicValue::structure(
                "MsgAliasInt16",
                [("value", DynamicValue::union("T", DynamicValue::from(16)))],
            ),
        ),
        (
            "MsgAliasInt16",
            DynamicValue::structure(
                "MsgAliasInt16",
                [("value", DynamicValue::union("int8", DynamicValue::from(17)))],
            ),
        ),
    ];

    expect![[r#"
        (
            (
                "x{10000001000000054_}",
                MsgPair {
                    value: GenericPair {
                        value: Variant0(
                            10,
                        ),
                    },
                },
            ),
            (
                "x{100000018000000000000005C_}",
                MsgPair {
                    value: GenericPair {
                        value: Variant1(
                            11,
                        ),
                    },
                },
            ),
            (
                "x{100000030006C_}",
                MsgOrInt16 {
                    value: GenericOrInt8 {
                        value: Variant0(
                            13,
                        ),
                    },
                },
            ),
            (
                "x{10000003874_}",
                MsgOrInt16 {
                    value: GenericOrInt8 {
                        value: Variant1(
                            14,
                        ),
                    },
                },
            ),
            (
                "x{1000000500084_}",
                MsgAliasInt16 {
                    value: Variant0(
                        16,
                    ),
                },
            ),
            (
                "x{1000000588C_}",
                MsgAliasInt16 {
                    value: Variant1(
                        17,
                    ),
                },
            ),
            (
                [
                    "T1",
                    "T2",
                ],
                [
                    "T",
                    "int8",
                ],
            ),
        )
    "#]]
    .assert_debug_eq(&(
        round_trip(&pair_left),
        round_trip(&pair_right),
        round_trip(&or_int16_left),
        round_trip(&or_int16_right),
        round_trip(&alias_left),
        round_trip(&alias_right),
        (
            generated::UnionTy18::<BigInt, BigInt>::VARIANT_LABELS,
            generated::UnionTy26::<BigInt>::VARIANT_LABELS,
        ),
    ));

    let dynamic_results = dynamic_values
        .iter()
        .map(|(type_name, value)| dynamic_round_trip(type_name, value))
        .collect::<Vec<_>>();
    expect![[r#"
        [
            (
                "x{10000001000000054_}",
                true,
            ),
            (
                "x{100000018000000000000005C_}",
                true,
            ),
            (
                "x{100000030006C_}",
                true,
            ),
            (
                "x{10000003874_}",
                true,
            ),
            (
                "x{1000000500084_}",
                true,
            ),
            (
                "x{1000000588C_}",
                true,
            ),
        ]
    "#]]
    .assert_debug_eq(&dynamic_results);
}

async fn verify_generic_union_getters_with_wrapper() {
    let contract = deployed_contract().await;
    let pair = contract.get_pair().await.expect("getPair must decode");
    let or_int16 = contract
        .get_or_int16()
        .await
        .expect("getOrInt16 must decode");
    let alias_int16 = contract
        .get_alias_int16()
        .await
        .expect("getAliasInt16 must decode");
    let either_int32_bool = contract
        .get_either_int32_bool()
        .await
        .expect("getEitherInt32Bool must decode");
    let either_bool_bool = contract
        .get_either_bool_bool()
        .await
        .expect("getEitherBoolBool must decode");
    let raw_either = contract
        .get_raw_either_int32_bool()
        .await
        .expect("getRawEitherInt32Bool must decode");

    expect![[r#"
        (
            GenericPair {
                value: Variant0(
                    10,
                ),
            },
            GenericOrInt8 {
                value: Variant0(
                    12,
                ),
            },
            Variant0(
                14,
            ),
            Variant0(
                TlbEitherLeft {
                    value: 19,
                },
            ),
            Variant0(
                TlbEitherLeft {
                    value: true,
                },
            ),
            Variant0(
                20,
            ),
            (
                [
                    "T1",
                    "T2",
                ],
                [
                    "T",
                    "int8",
                ],
                [
                    "TlbEitherLeft",
                    "TlbEitherRight",
                ],
                [
                    "X",
                    "Y",
                ],
            ),
        )
    "#]]
    .assert_debug_eq(&(
        pair,
        or_int16,
        alias_int16,
        either_int32_bool,
        either_bool_bool,
        raw_either,
        (
            generated::UnionTy18::<BigInt, BigInt>::VARIANT_LABELS,
            generated::UnionTy26::<BigInt>::VARIANT_LABELS,
            generated::UnionTy57::<BigInt, bool>::VARIANT_LABELS,
            generated::UnionTy76::<BigInt, bool>::VARIANT_LABELS,
        ),
    ));
}

#[tokio::test]
async fn reads_generic_unions_from_getters_dynamic() {
    let abi = dynamic_abi();
    let contract = deployed_contract().await;
    let address = contract.address().clone();
    let provider = contract.provider();
    let results = vec![
        abi.call_get_method(provider, &address, "getPair", &[])
            .await
            .expect("dynamic getPair must decode"),
        abi.call_get_method(provider, &address, "getOrInt16", &[])
            .await
            .expect("dynamic getOrInt16 must decode"),
        abi.call_get_method(provider, &address, "getAliasInt16", &[])
            .await
            .expect("dynamic getAliasInt16 must decode"),
        abi.call_get_method(provider, &address, "getEitherInt32Bool", &[])
            .await
            .expect("dynamic getEitherInt32Bool must decode"),
        abi.call_get_method(provider, &address, "getEitherBoolBool", &[])
            .await
            .expect("dynamic getEitherBoolBool must decode"),
        abi.call_get_method(provider, &address, "getRawEitherInt32Bool", &[])
            .await
            .expect("dynamic getRawEitherInt32Bool must decode"),
    ];

    expect![[r#"
        [
            Object(
                [
                    (
                        "$",
                        String(
                            "GenericPair",
                        ),
                    ),
                    (
                        "value",
                        Object(
                            [
                                (
                                    "$",
                                    String(
                                        "T1",
                                    ),
                                ),
                                (
                                    "value",
                                    Number(
                                        10,
                                    ),
                                ),
                            ],
                        ),
                    ),
                ],
            ),
            Object(
                [
                    (
                        "$",
                        String(
                            "GenericOrInt8",
                        ),
                    ),
                    (
                        "value",
                        Object(
                            [
                                (
                                    "$",
                                    String(
                                        "T",
                                    ),
                                ),
                                (
                                    "value",
                                    Number(
                                        12,
                                    ),
                                ),
                            ],
                        ),
                    ),
                ],
            ),
            Object(
                [
                    (
                        "$",
                        String(
                            "T",
                        ),
                    ),
                    (
                        "value",
                        Number(
                            14,
                        ),
                    ),
                ],
            ),
            Object(
                [
                    (
                        "$",
                        String(
                            "TlbEitherLeft",
                        ),
                    ),
                    (
                        "value",
                        Number(
                            19,
                        ),
                    ),
                ],
            ),
            Object(
                [
                    (
                        "$",
                        String(
                            "TlbEitherLeft",
                        ),
                    ),
                    (
                        "value",
                        Bool(
                            true,
                        ),
                    ),
                ],
            ),
            Object(
                [
                    (
                        "$",
                        String(
                            "X",
                        ),
                    ),
                    (
                        "value",
                        Number(
                            20,
                        ),
                    ),
                ],
            ),
        ]
    "#]]
    .assert_debug_eq(&results);
}

#[tokio::test]
async fn reads_generic_unions_from_getters_wrapper() {
    verify_generic_union_getters_with_wrapper().await;
}

async fn verify_instantiated_nullable_with_wrapper() {
    let contract = deployed_contract().await;
    let point = generated::Point {
        x: BigInt::from(10),
        y: BigInt::from(20),
    };
    let some = contract
        .get_id_my_nullable_point(&Some(point))
        .await
        .expect("idMyNullablePoint(Some) must decode");
    let none = contract
        .get_id_my_nullable_point(&None)
        .await
        .expect("idMyNullablePoint(None) must decode");

    expect![[r"
        (
            (
                Some(
                    Point {
                        x: 10,
                        y: 20,
                    },
                ),
                CellRef {
                    ref: Some(
                        Point {
                            x: 10,
                            y: 20,
                        },
                    ),
                },
            ),
            (
                None,
                CellRef {
                    ref: None,
                },
            ),
        )
    "]]
    .assert_debug_eq(&(some, none));
}

#[tokio::test]
async fn reads_instantiated_nullable_from_getters_dynamic() {
    let abi = dynamic_abi();
    let contract = deployed_contract().await;
    let address = contract.address().clone();
    let provider = contract.provider();
    let point = DynamicValue::structure(
        "Point",
        [("x", DynamicValue::from(10)), ("y", DynamicValue::from(20))],
    );
    let some = abi
        .call_get_method(
            provider,
            &address,
            "idMyNullablePoint",
            std::slice::from_ref(&point),
        )
        .await
        .expect("dynamic idMyNullablePoint(Some) must decode");
    let none = abi
        .call_get_method(
            provider,
            &address,
            "idMyNullablePoint",
            &[DynamicValue::Null],
        )
        .await
        .expect("dynamic idMyNullablePoint(None) must decode");

    expect![[r#"
        (
            Array(
                [
                    Object(
                        [
                            (
                                "$",
                                String(
                                    "Point",
                                ),
                            ),
                            (
                                "x",
                                Number(
                                    10,
                                ),
                            ),
                            (
                                "y",
                                Number(
                                    20,
                                ),
                            ),
                        ],
                    ),
                    Object(
                        [
                            (
                                "ref",
                                Object(
                                    [
                                        (
                                            "$",
                                            String(
                                                "Point",
                                            ),
                                        ),
                                        (
                                            "x",
                                            Number(
                                                10,
                                            ),
                                        ),
                                        (
                                            "y",
                                            Number(
                                                20,
                                            ),
                                        ),
                                    ],
                                ),
                            ),
                        ],
                    ),
                ],
            ),
            Array(
                [
                    Null,
                    Object(
                        [
                            (
                                "ref",
                                Null,
                            ),
                        ],
                    ),
                ],
            ),
        )
    "#]]
    .assert_debug_eq(&(some, none));
}

#[tokio::test]
async fn reads_instantiated_nullable_from_getters_wrapper() {
    verify_instantiated_nullable_with_wrapper().await;
}

fn relaxed_message_summary(
    message: &generated::TlbMessageRelaxedGeneric,
) -> (bool, String, BigInt, &'static str, u16, u8) {
    let init = match &message.init {
        generated::UnionTy64::Variant0(_) => "TlbNone",
        generated::UnionTy64::Variant1(_) => "TlbJust",
    };
    let body = message.body.as_slice().expect("body must be readable");
    (
        message.info.bounce,
        message.info.dest.display_base64_url(true).to_string(),
        message.info.value.clone(),
        init,
        body.size_bits(),
        body.size_refs(),
    )
}

fn dynamic_relaxed_message_summary(
    message: &DynamicValue,
) -> (bool, bool, BigInt, String, u16, u8) {
    let info = message
        .field("info")
        .expect("dynamic relaxed message must contain info");
    let bounce = matches!(info.field("bounce"), Some(DynamicValue::Bool(true)));
    let destination_matches =
        info.field("dest") == Some(&DynamicValue::from(relaxed_destination()));
    let value = match info.field("value") {
        Some(DynamicValue::Number(value)) => value.clone(),
        _ => panic!("dynamic relaxed message value must be a number"),
    };
    let init = message
        .field("init")
        .and_then(DynamicValue::tag)
        .expect("dynamic relaxed message init must preserve its label")
        .to_owned();
    let Some(DynamicValue::Slice(body)) = message.field("body") else {
        panic!("dynamic relaxed message body must be a slice")
    };
    let body = body.as_slice().expect("dynamic body must be readable");
    (
        bounce,
        destination_matches,
        value,
        init,
        body.size_bits(),
        body.size_refs(),
    )
}

async fn verify_relaxed_message_getter_with_wrapper() {
    let message = deployed_contract()
        .await
        .get_relaxed_message()
        .await
        .expect("getRelaxedMessage must decode");

    expect![[r#"
        (
            true,
            "EQBSeWTVXPputzH0v8B-nQJQmAl--FBVGehTmGJ5vYQA2Hyk",
            50000000,
            "TlbNone",
            0,
            0,
        )
    "#]]
    .assert_debug_eq(&relaxed_message_summary(&message));
}

#[tokio::test]
async fn reads_tlb_message_relaxed_generic_from_a_getter_dynamic() {
    let contract = deployed_contract().await;
    let address = contract.address().clone();
    let provider = contract.provider();
    let message = dynamic_abi()
        .call_get_method(provider, &address, "getRelaxedMessage", &[])
        .await
        .expect("dynamic getRelaxedMessage must decode");

    expect![[r#"
        (
            true,
            true,
            50000000,
            "TlbNone",
            0,
            0,
        )
    "#]]
    .assert_debug_eq(&dynamic_relaxed_message_summary(&message));
}

#[tokio::test]
async fn reads_tlb_message_relaxed_generic_from_a_getter_wrapper() {
    verify_relaxed_message_getter_with_wrapper().await;
}

async fn verify_relaxed_message_parameter_with_wrapper() {
    let result = deployed_contract()
        .await
        .get_check_relaxed_message(&make_relaxed_message())
        .await
        .expect("checkRelaxedMessage must decode");
    expect![[r"
        777
    "]]
    .assert_debug_eq(&result);
}

#[tokio::test]
async fn passes_tlb_message_relaxed_generic_as_a_getter_parameter_dynamic() {
    let contract = deployed_contract().await;
    let address = contract.address().clone();
    let provider = contract.provider();
    let result = dynamic_abi()
        .call_get_method(
            provider,
            &address,
            "checkRelaxedMessage",
            &[make_dynamic_relaxed_message()],
        )
        .await
        .expect("dynamic checkRelaxedMessage must decode");
    expect![[r"
        Number(
            777,
        )
    "]]
    .assert_debug_eq(&result);
}

#[tokio::test]
async fn passes_tlb_message_relaxed_generic_as_a_getter_parameter_wrapper() {
    verify_relaxed_message_parameter_with_wrapper().await;
}

#[tokio::test]
async fn debug_prints_tlb_message_relaxed_generic_from_the_stack() {
    let contract = deployed_contract().await;
    let address = contract.address().clone();
    let provider = contract.provider();
    let result = provider
        .run_get_method(&address, 103_120, Tuple::empty())
        .await
        .expect("getRelaxedMessage provider call must succeed");
    let abi = dynamic_abi();
    let return_ty_idx = abi
        .find_get_method("getRelaxedMessage")
        .expect("getRelaxedMessage ABI entry must exist")
        .return_ty_idx;
    let rendered = abi
        .debug_print_from_stack(result, return_ty_idx)
        .expect("dynamic relaxed message stack must debug-print");

    expect!["TlbMessageRelaxedGeneric { info: TlbIntMsgInfoRelaxed { bounce: true, dest: EQBSeWTVXPputzH0v8B-nQJQmAl--FBVGehTmGJ5vYQA2Hyk, value: 50000000 }, init: TlbNone {}, body: slice{x{}} }"]
        .assert_eq(&rendered);
}
