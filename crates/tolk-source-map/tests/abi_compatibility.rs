use tolk_abi::{UnpackSchema, UnpackedValue};
use tolk_source_map::{SourceMap, dynamic_unpack};
use tycho_types::cell::CellBuilder;

fn source_map() -> SourceMap {
    serde_json::from_str(
        r#"{
            "files": [],
            "unique_types": [
                {"kind": "StructRef", "struct_name": "SourceMessage"},
                {"kind": "AliasRef", "alias_name": "Count"},
                {"kind": "uintN", "n": 16},
                {"kind": "EnumRef", "enum_name": "Status"},
                {"kind": "uintN", "n": 8}
            ],
            "struct_instantiations": [],
            "alias_instantiations": [],
            "declarations": [
                {
                    "kind": "struct", "name": "SourceMessage", "ty_idx": 0,
                    "ident_loc": [0, 1, 1, 1, 1],
                    "prefix": {"prefix_num": 305419896, "prefix_len": 32},
                    "fields": [
                        {"name": "count", "ty_idx": 1},
                        {"name": "status", "ty_idx": 3}
                    ]
                },
                {
                    "kind": "alias", "name": "Count", "ty_idx": 1,
                    "ident_loc": [0, 1, 1, 1, 1], "target_ty_idx": 2
                },
                {
                    "kind": "enum", "name": "Status", "ty_idx": 3,
                    "ident_loc": [0, 1, 1, 1, 1], "encoded_as_ty_idx": 4,
                    "members": [{"name": "Ready", "value": "1"}]
                }
            ],
            "functions": []
        }"#,
    )
    .unwrap()
}

#[test]
fn old_decoder_import_accepts_source_map_and_returns_standalone_abi_values() {
    let symbols = source_map();
    let schema: &dyn UnpackSchema = &symbols;
    let ty: &tolk_abi::Ty = symbols.ty_by_idx(0).unwrap();
    assert_eq!(
        tolk_abi::types_kernel::render_ty(schema, 0),
        "SourceMessage"
    );
    assert!(matches!(ty, tolk_abi::Ty::StructRef { .. }));

    let mut builder = CellBuilder::new();
    builder.store_uint(0x1234_5678, 32).unwrap();
    builder.store_uint(42, 16).unwrap();
    builder.store_uint(1, 8).unwrap();
    let cell = builder.build().unwrap();
    let mut slice = cell.as_slice().unwrap();

    let value: UnpackedValue = dynamic_unpack::unpack_from_slice(&mut slice, schema, 0).unwrap();
    let UnpackedValue::Object { name, fields } = value else {
        panic!("expected a decoded message");
    };
    assert_eq!(name, "SourceMessage");
    assert_eq!(fields.len(), 2);
    for ((name, value), (expected_name, expected_value)) in
        fields.iter().zip([("count", "42"), ("status", "1")])
    {
        assert_eq!(name, expected_name);
        let UnpackedValue::Number(value) = value else {
            panic!("expected a numeric field");
        };
        assert_eq!(value.to_string(), expected_value);
    }
    assert!(slice.is_empty());
}

#[test]
fn opcode_lookup_prefers_source_map_and_falls_back_to_standalone_abi() {
    let symbols = source_map();
    let mut abi: tolk_source_map::abi::ContractABI = tolk_abi::ContractABI::default();
    for (name, opcode) in [("AbiMessage", 0x1234_5678u32), ("AbiOnly", 0x9876_5432)] {
        abi.declarations.push(
            serde_json::from_value(serde_json::json!({
                "kind": "struct", "name": name, "ty_idx": 0,
                "prefix": {"prefix_num": opcode, "prefix_len": 32},
                "fields": []
            }))
            .unwrap(),
        );
    }

    assert_eq!(
        symbols.find_message_name_by_opcode_with_abi(Some(&abi), 0x1234_5678),
        Some("SourceMessage")
    );
    assert_eq!(
        symbols.find_message_name_by_opcode_with_abi(Some(&abi), 0x9876_5432),
        Some("AbiOnly")
    );
    assert_eq!(
        symbols.find_message_name_by_opcode_with_abi(None, 0x1234_5678),
        Some("SourceMessage")
    );
    assert_eq!(
        symbols.find_message_name_by_opcode_with_abi(None, 0x9876_5432),
        None
    );
    assert_eq!(
        symbols.find_message_name_by_opcode_with_abi(Some(&abi), 0),
        None
    );
}
