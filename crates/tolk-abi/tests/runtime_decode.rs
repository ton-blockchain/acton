use tolk_abi::{ContractABI, Ty, TyIdx, UnpackSchema, UnpackedValue, unpack_from_slice};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder};

// The schema is supplied as JSON, with no generated Rust contract types.
const ABI_JSON: &str = r#"{
    "abi_schema_version": "1.0",
    "contract_name": "RuntimeDecoderTest",
    "unique_types": [
        { "kind": "uintN", "n": 32 },
        { "kind": "uintN", "n": 16 },
        { "kind": "bool" },
        { "kind": "StructRef", "struct_name": "Details" },
        { "kind": "cellOf", "inner_ty_idx": 3 },
        { "kind": "StructRef", "struct_name": "Message" }
    ],
    "struct_instantiations": [],
    "alias_instantiations": [],
    "declarations": [
        {
            "kind": "struct",
            "name": "Details",
            "ty_idx": 3,
            "prefix": { "prefix_num": 122, "prefix_len": 8 },
            "fields": [
                { "name": "amount", "ty_idx": 1 },
                { "name": "enabled", "ty_idx": 2 }
            ]
        },
        {
            "kind": "struct",
            "name": "Message",
            "ty_idx": 5,
            "prefix": { "prefix_num": 305419896, "prefix_len": 32 },
            "fields": [
                { "name": "query_id", "ty_idx": 0 },
                { "name": "details", "ty_idx": 4 }
            ]
        }
    ],
    "incoming_messages": [{ "body_ty_idx": 5 }],
    "incoming_external": [],
    "outgoing_messages": [],
    "emitted_events": [],
    "storage": {},
    "get_methods": [],
    "thrown_errors": [],
    "compiler_name": "tolk",
    "compiler_version": "test"
}"#;

fn message_cell(details_prefix: u64) -> Cell {
    let mut details = CellBuilder::new();
    details.store_uint(details_prefix, 8).unwrap();
    details.store_uint(513, 16).unwrap();
    details.store_bit(true).unwrap();

    let mut message = CellBuilder::new();
    message.store_uint(0x1234_5678, 32).unwrap();
    message.store_uint(42, 32).unwrap();
    message.store_reference(details.build().unwrap()).unwrap();
    message.build().unwrap()
}

fn decode_boc(abi: &impl UnpackSchema, ty_idx: TyIdx, boc: &[u8]) -> anyhow::Result<UnpackedValue> {
    let cell = Boc::decode(boc)?;
    let mut slice = cell.as_slice()?;
    let value = unpack_from_slice(&mut slice, abi, ty_idx)?;
    assert_eq!(slice.size_bits(), 0);
    assert_eq!(slice.size_refs(), 0);
    Ok(value)
}

#[test]
fn decodes_json_abi_and_boc_with_nested_typed_reference() {
    let abi: ContractABI = serde_json::from_str(ABI_JSON).unwrap();
    let ty_idx: TyIdx = abi.incoming_messages[0].body_ty_idx;
    assert!(matches!(abi.ty_by_idx(ty_idx), Some(Ty::StructRef { .. })));

    let boc = Boc::encode(message_cell(0x7a));
    let value = decode_boc(&abi, ty_idx, &boc).unwrap();

    let UnpackedValue::Object { name, fields } = value else {
        panic!("expected Message object");
    };
    assert_eq!(name, "Message");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].0, "query_id");
    assert!(matches!(&fields[0].1, UnpackedValue::Number(value) if value == &42.into()));
    assert_eq!(fields[1].0, "details");

    let UnpackedValue::Object { name, fields } = &fields[1].1 else {
        panic!("expected Cell wrapper");
    };
    assert_eq!(name, "Cell");
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].0, "ref");

    let UnpackedValue::Object { name, fields } = &fields[0].1 else {
        panic!("expected Details object");
    };
    assert_eq!(name, "Details");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].0, "amount");
    assert!(matches!(&fields[0].1, UnpackedValue::Number(value) if value == &513.into()));
    assert_eq!(fields[1].0, "enabled");
    assert!(matches!(&fields[1].1, UnpackedValue::Bool(true)));
}

#[test]
fn reports_nested_prefix_mismatch_with_field_context() {
    let abi: ContractABI = serde_json::from_str(ABI_JSON).unwrap();
    let ty_idx = abi.incoming_messages[0].body_ty_idx;
    let boc = Boc::encode(message_cell(0x7b));

    let error = decode_boc(&abi, ty_idx, &boc).unwrap_err();

    assert_eq!(error.to_string(), "failed to decode field Message.details");
    assert_eq!(
        error.root_cause().to_string(),
        "incorrect prefix for 'Details': expected 0x7a, got 0x7b"
    );
}

#[test]
fn rejects_runtime_type_index_missing_from_json_schema() {
    let abi: ContractABI = serde_json::from_str(ABI_JSON).unwrap();
    let boc = Boc::encode(message_cell(0x7a));
    let missing_ty_idx = abi.unique_types.len();

    let error = decode_boc(&abi, missing_ty_idx, &boc).unwrap_err();

    assert_eq!(
        error.to_string(),
        format!("ABI ty_idx {missing_ty_idx} was not found")
    );
}
