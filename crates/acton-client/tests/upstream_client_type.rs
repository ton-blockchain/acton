use acton_client::__private::tycho_types::cell::CellBuilder;
use acton_client::{
    AbiLoad, AbiStore, BigInt, Cell, CellRef, ContractProvider, DynamicAbi, DynamicValue,
    OwnedSlice, SendOptions, StdAddr, Tuple,
};
use expect_test::expect;
use tolk_source_map::abi::ABIDeclaration;
use tolk_source_map::types_kernel::Ty;

mod support;

use support::{TvmContractProvider, TvmSender};

#[acton_client::contract(abi = "tests/fixtures/upstream/client-type-anno.abi.json")]
mod generated {}

const ABI_JSON: &str = include_str!("fixtures/upstream/client-type-anno.abi.json");
fn old_owner() -> StdAddr {
    StdAddr {
        anycast: None,
        workchain: 0,
        address: Default::default(),
    }
}

fn make_slice(hex: u16) -> OwnedSlice {
    let mut builder = CellBuilder::new();
    acton_client::cell::store_fixed_int(&mut builder, &BigInt::from(hex), 16, false)
        .expect("test slice must encode");
    OwnedSlice::full(builder.build().expect("test slice cell must build"))
}

fn slice_hex(slice: &OwnedSlice) -> String {
    format!(
        "x{{{:X}}}",
        slice
            .as_slice()
            .expect("slice must be readable")
            .display_data()
    )
}

fn cell_hex(cell: &Cell) -> String {
    format!(
        "x{{{:X}}}",
        cell.as_slice()
            .expect("cell must be readable")
            .display_data()
    )
}

fn dynamic_abi() -> DynamicAbi {
    DynamicAbi::from_json(ABI_JSON).expect("upstream ABI must parse")
}

fn notification_payload(value: &generated::NotificationForNewOwner) -> (&'static str, String) {
    match &value.payload {
        generated::UnionTy19::Variant0(payload) => ("PayloadInline", slice_hex(&payload.value)),
        generated::UnionTy19::Variant1(payload) => {
            ("PayloadInRef", slice_hex(payload.value.r#ref.as_ref()))
        }
    }
}

async fn contract() -> generated::ClientTypeAnno<TvmContractProvider> {
    let contract = generated::ClientTypeAnno::from_storage(&generated::ClientTypeStorage {
        dummy: BigInt::from(0),
    })
    .expect("ClientTypeAnno state init must build");
    let provider = TvmContractProvider::new(contract.address().clone())
        .expect("local TVM provider must initialize");
    let contract = contract.with_provider(provider);
    contract
        .send_deploy(
            &TvmSender::new("deployer", 0xd0),
            BigInt::from(50_000_000_u64),
            SendOptions::default(),
        )
        .await
        .expect("deploy transaction must execute");
    contract
}

fn dynamic_slice_hex(value: &DynamicValue, field: &str) -> String {
    let Some(DynamicValue::Slice(slice)) = value.field(field) else {
        panic!("dynamic field '{field}' must be a slice")
    };
    slice_hex(slice)
}

fn dynamic_number(value: &DynamicValue, field: &str) -> String {
    let Some(DynamicValue::Number(number)) = value.field(field) else {
        panic!("dynamic field '{field}' must be a number")
    };
    number.to_string()
}

fn dynamic_payload_tag(value: &DynamicValue) -> String {
    value
        .field("payload")
        .and_then(DynamicValue::tag)
        .expect("dynamic payload must preserve its union label")
        .to_owned()
}

async fn client_payload_get_method_result(is_dynamic: bool) -> (String, String) {
    if is_dynamic {
        let abi = dynamic_abi();
        let contract = contract().await;
        let address = contract.address();
        let provider = contract.provider();
        let payload = abi
            .call_get_method(provider, address, "clientPayload", &[])
            .await
            .expect("dynamic getter must run and decode");
        let echo_arg = DynamicValue::structure(
            "ClientPayload",
            [("note", DynamicValue::from(make_slice(0xBEEF)))],
        );
        let echo = abi
            .call_get_method(provider, address, "echoPayload", &[echo_arg])
            .await
            .expect("dynamic echo getter must run and decode");
        (
            dynamic_slice_hex(&payload, "note"),
            dynamic_slice_hex(&echo, "note"),
        )
    } else {
        let echo_arg = generated::ClientPayloadStackTy12 {
            note: make_slice(0xBEEF),
        };
        let contract = contract().await;
        let payload = contract
            .get_client_payload()
            .await
            .expect("wrapper getter must decode");
        let echo = contract
            .get_echo_payload(&echo_arg)
            .await
            .expect("wrapper echo getter must decode");
        (slice_hex(&payload.note), slice_hex(&echo.note))
    }
}

async fn notification_get_method_result(is_dynamic: bool) -> (String, bool, String, String) {
    if is_dynamic {
        let abi = dynamic_abi();
        let contract = contract().await;
        let address = contract.address();
        let provider = contract.provider();
        let notification = abi
            .call_get_method(provider, address, "notificationForNewOwner", &[])
            .await
            .expect("dynamic getter must run and decode");
        let echo_arg = DynamicValue::structure(
            "NotificationForNewOwner",
            [
                ("queryId", DynamicValue::from(77_u8)),
                ("oldOwnerAddress", DynamicValue::from(old_owner())),
                ("payload", DynamicValue::from(make_slice(0xF00D))),
            ],
        );
        let echo = abi
            .call_get_method(
                provider,
                address,
                "echoNotificationForNewOwner",
                &[echo_arg],
            )
            .await
            .expect("dynamic echo getter must run and decode");
        (
            dynamic_number(&notification, "queryId"),
            notification.field("oldOwnerAddress") == Some(&DynamicValue::from(old_owner())),
            dynamic_slice_hex(&notification, "payload"),
            format!(
                "{}:{}",
                dynamic_number(&echo, "queryId"),
                dynamic_slice_hex(&echo, "payload")
            ),
        )
    } else {
        let echo_arg = generated::NotificationForNewOwnerStackTy15 {
            query_id: BigInt::from(77),
            old_owner_address: old_owner(),
            payload: make_slice(0xF00D),
        };
        let contract = contract().await;
        let notification = contract
            .get_notification_for_new_owner()
            .await
            .expect("wrapper getter must decode");
        let echo = contract
            .get_echo_notification_for_new_owner(&echo_arg)
            .await
            .expect("wrapper echo getter must decode");
        (
            notification.query_id.to_string(),
            notification.old_owner_address == old_owner(),
            slice_hex(&notification.payload),
            format!("{}:{}", echo.query_id, slice_hex(&echo.payload)),
        )
    }
}

#[test]
fn exports_client_ty_idx_on_fields_and_descriptions_on_declarations() {
    let abi = dynamic_abi();
    let Some(ABIDeclaration::Struct { description, .. }) = abi
        .abi()
        .declarations
        .iter()
        .find(|declaration| {
            matches!(declaration, ABIDeclaration::Struct { name, .. } if name == "ClientPayload")
        })
    else {
        panic!("ClientPayload struct must be present")
    };
    let Some(ABIDeclaration::Struct { fields, .. }) =
        abi.abi().declarations.iter().find(|declaration| {
            matches!(declaration, ABIDeclaration::Struct { name, .. } if name == "NotificationForNewOwner")
        })
    else {
        panic!("NotificationForNewOwner struct must be present")
    };
    let payload = fields
        .iter()
        .find(|field| field.name == "payload")
        .expect("payload field must be present");
    let declared_kind = match &abi.abi().unique_types[payload.ty_idx] {
        Ty::Remaining => "remaining",
        other => panic!("expected remaining, got {other:?}"),
    };
    let client_kind = match &abi.abi().unique_types[payload
        .client_ty_idx
        .expect("payload must have a client type")]
    {
        Ty::Union { .. } => "union",
        other => panic!("expected union, got {other:?}"),
    };

    expect![[r#"
        (
            "Payload with a client-only field type.",
            "remaining",
            "union",
        )
    "#]]
    .assert_debug_eq(&(description, declared_kind, client_kind));
}

#[test]
fn uses_client_ty_idx_for_wrapper_and_dynamic_cell_serialization() {
    let abi = dynamic_abi();
    let ty_idx = abi
        .declaration_type_index("ClientPayload")
        .expect("ClientPayload type must be present");
    let value = generated::ClientPayload {
        note: "hello".to_owned(),
    };
    let dynamic_value =
        DynamicValue::structure("ClientPayload", [("note", DynamicValue::from("hello"))]);
    let wrapper_cell = value.to_cell().expect("wrapper value must encode");
    let contract_cell =
        generated::ClientTypeAnno::<TvmContractProvider>::create_cell_of_client_payload(&value)
            .expect("contract helper must encode");
    let dynamic_cell = abi
        .pack_to_cell(ty_idx, &dynamic_value)
        .expect("dynamic value must encode");

    let mut wrapper_slice = wrapper_cell
        .as_slice()
        .expect("wrapper cell must be readable");
    let prefix = acton_client::cell::load_fixed_int(&mut wrapper_slice, 32, false)
        .expect("prefix must decode");
    let note = acton_client::cell::load_string(&mut wrapper_slice).expect("note must decode");
    let wrapper_back =
        generated::ClientPayload::from_cell(&wrapper_cell).expect("wrapper value must decode");
    let dynamic_back = abi
        .unpack_from_cell(ty_idx, &wrapper_cell)
        .expect("dynamic value must decode");

    expect![[r#"
        (
            "305419896",
            "hello",
            true,
            true,
            true,
            true,
        )
    "#]]
    .assert_debug_eq(&(
        prefix.to_string(),
        note,
        contract_cell == wrapper_cell,
        wrapper_back.note == value.note,
        dynamic_cell == wrapper_cell,
        dynamic_back == dynamic_value,
    ));
}

#[tokio::test]
async fn keeps_ty_idx_for_client_payload_get_methods_dynamic() {
    expect![[r#"
        (
            "x{CAFE}",
            "x{BEEF}",
        )
    "#]]
    .assert_debug_eq(&client_payload_get_method_result(true).await);
}

#[tokio::test]
async fn keeps_ty_idx_for_client_payload_get_methods_wrapper() {
    expect![[r#"
        (
            "x{CAFE}",
            "x{BEEF}",
        )
    "#]]
    .assert_debug_eq(&client_payload_get_method_result(false).await);
}

#[tokio::test]
async fn debug_prints_original_stack_type_not_client_type() {
    let contract = contract().await;
    let provider = contract.provider();
    let result = provider
        .run_get_method(contract.address(), 115_947, Tuple::empty())
        .await
        .expect("getter must run");

    let abi = dynamic_abi();
    let ty_idx = abi
        .declaration_type_index("ClientPayload")
        .expect("ClientPayload type must be present");
    expect!["ClientPayload { note: slice{x{CAFE}} }"].assert_eq(
        &abi.debug_print_from_stack(result, ty_idx)
            .expect("stack must debug-print"),
    );
}

#[test]
fn serializes_notification_payload_through_client_union_type() {
    let abi = dynamic_abi();
    let ty_idx = abi
        .declaration_type_index("NotificationForNewOwner")
        .expect("NotificationForNewOwner type must be present");
    let values = [
        (
            generated::NotificationForNewOwner {
                query_id: BigInt::from(1),
                old_owner_address: old_owner(),
                payload: generated::UnionTy19::Variant0(generated::PayloadInline {
                    value: make_slice(0xCAFE),
                }),
            },
            DynamicValue::structure(
                "NotificationForNewOwner",
                [
                    ("queryId", DynamicValue::from(1_u8)),
                    ("oldOwnerAddress", DynamicValue::from(old_owner())),
                    (
                        "payload",
                        DynamicValue::structure(
                            "PayloadInline",
                            [("value", DynamicValue::from(make_slice(0xCAFE)))],
                        ),
                    ),
                ],
            ),
        ),
        (
            generated::NotificationForNewOwner {
                query_id: BigInt::from(2),
                old_owner_address: old_owner(),
                payload: generated::UnionTy19::Variant1(generated::PayloadInRef {
                    value: CellRef::new(make_slice(0xBEEF)),
                }),
            },
            DynamicValue::structure(
                "NotificationForNewOwner",
                [
                    ("queryId", DynamicValue::from(2_u8)),
                    ("oldOwnerAddress", DynamicValue::from(old_owner())),
                    (
                        "payload",
                        DynamicValue::structure(
                            "PayloadInRef",
                            [(
                                "value",
                                DynamicValue::reference(DynamicValue::from(make_slice(0xBEEF))),
                            )],
                        ),
                    ),
                ],
            ),
        ),
    ];
    let mut results = Vec::new();
    for (value, dynamic_value) in values {
        let wrapper_cell = value.to_cell().expect("wrapper value must encode");
        let wrapper_back = generated::NotificationForNewOwner::from_cell(&wrapper_cell)
            .expect("wrapper value must decode");
        let dynamic_cell = abi
            .pack_to_cell(ty_idx, &dynamic_value)
            .expect("dynamic value must encode");
        let dynamic_back = abi
            .unpack_from_cell(ty_idx, &wrapper_cell)
            .expect("dynamic value must decode");
        results.push((
            cell_hex(&wrapper_cell),
            wrapper_back.query_id.to_string(),
            wrapper_back.old_owner_address == old_owner(),
            notification_payload(&wrapper_back),
            dynamic_cell == wrapper_cell,
            dynamic_number(&dynamic_back, "queryId"),
            dynamic_payload_tag(&dynamic_back),
        ));
    }

    let helper_cell =
        generated::ClientTypeAnno::<TvmContractProvider>::create_cell_of_notification_for_new_owner(
            &generated::NotificationForNewOwner {
                query_id: BigInt::from(3),
                old_owner_address: old_owner(),
                payload: generated::UnionTy19::Variant0(generated::PayloadInline {
                    value: make_slice(0x1234),
                }),
            },
        )
        .expect("contract helper must encode");
    let mut helper_slice = helper_cell
        .as_slice()
        .expect("helper cell must be readable");
    let helper_prefix = acton_client::cell::load_fixed_int(&mut helper_slice, 32, false)
        .expect("helper prefix must decode");

    expect![[r#"
        (
            [
                (
                    "x{05138D9100000000000000018000000000000000000000000000000000000000000000000000000000000000000CAFE}",
                    "1",
                    true,
                    (
                        "PayloadInline",
                        "x{CAFE}",
                    ),
                    true,
                    "1",
                    "PayloadInline",
                ),
                (
                    "x{05138D9100000000000000028000000000000000000000000000000000000000000000000000000000000000001}",
                    "2",
                    true,
                    (
                        "PayloadInRef",
                        "x{BEEF}",
                    ),
                    true,
                    "2",
                    "PayloadInRef",
                ),
            ],
            "85167505",
        )
    "#]]
    .assert_debug_eq(&(results, helper_prefix.to_string()));
}

#[tokio::test]
async fn passes_notification_through_get_methods_using_original_stack_type_dynamic() {
    expect![[r#"
        (
            "123",
            true,
            "x{BEEF}",
            "77:x{F00D}",
        )
    "#]]
    .assert_debug_eq(&notification_get_method_result(true).await);
}

#[tokio::test]
async fn passes_notification_through_get_methods_using_original_stack_type_wrapper() {
    expect![[r#"
        (
            "123",
            true,
            "x{BEEF}",
            "77:x{F00D}",
        )
    "#]]
    .assert_debug_eq(&notification_get_method_result(false).await);
}

#[tokio::test]
async fn debug_prints_notification_stack_payload_as_slice() {
    let contract = contract().await;
    let provider = contract.provider();
    let result = provider
        .run_get_method(contract.address(), 110_603, Tuple::empty())
        .await
        .expect("getter must run");

    let abi = dynamic_abi();
    let ty_idx = abi
        .declaration_type_index("NotificationForNewOwner")
        .expect("NotificationForNewOwner type must be present");
    expect!["NotificationForNewOwner { queryId: 123, oldOwnerAddress: EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c, payload: slice{x{BEEF}} }"]
        .assert_eq(&abi.debug_print_from_stack(result, ty_idx).expect("stack must debug-print"));
}
