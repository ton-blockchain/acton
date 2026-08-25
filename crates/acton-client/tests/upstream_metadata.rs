use expect_test::expect;
use std::str::FromStr;

#[acton_client::contract(abi = "tests/fixtures/upstream/has-not-init-storage.abi.json")]
mod has_not_initialized_storage {}

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-storage.abi.json")]
mod lots_of_storage {}

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-annotations.abi.json")]
mod lots_of_annotations {}

#[acton_client::contract(abi = "tests/fixtures/upstream/lots-of-throws.abi.json")]
mod lots_of_throws {}

#[acton_client::contract(abi = "tests/fixtures/upstream/only-header.abi.json")]
mod only_header {}

fn string_end(bytes: &[u8], start: usize) -> usize {
    debug_assert_eq!(bytes[start], b'"');
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    panic!("unterminated JSON string")
}

fn value_end(bytes: &[u8], start: usize) -> usize {
    match bytes[start] {
        b'"' => string_end(bytes, start),
        b'{' | b'[' => {
            let opening = bytes[start];
            let closing = if opening == b'{' { b'}' } else { b']' };
            let mut depth = 1_u32;
            let mut index = start + 1;
            while index < bytes.len() {
                match bytes[index] {
                    b'"' => index = string_end(bytes, index),
                    byte if byte == opening => {
                        depth += 1;
                        index += 1;
                    }
                    byte if byte == closing => {
                        depth -= 1;
                        index += 1;
                        if depth == 0 {
                            return index;
                        }
                    }
                    _ => index += 1,
                }
            }
            panic!("unterminated JSON collection")
        }
        _ => {
            let mut index = start;
            while index < bytes.len() && !matches!(bytes[index], b',' | b'}' | b']') {
                index += 1;
            }
            index
        }
    }
}

fn skip_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn json_value<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let bytes = object.as_bytes();
    let mut index = skip_whitespace(bytes, 0);
    if bytes.get(index) != Some(&b'{') {
        return None;
    }
    index += 1;

    loop {
        index = skip_whitespace(bytes, index);
        if bytes.get(index) == Some(&b'}') {
            return None;
        }
        if bytes.get(index) != Some(&b'"') {
            return None;
        }
        let key_end = string_end(bytes, index);
        let candidate = &object[index + 1..key_end - 1];
        index = skip_whitespace(bytes, key_end);
        if bytes.get(index) != Some(&b':') {
            return None;
        }
        index = skip_whitespace(bytes, index + 1);
        let end = value_end(bytes, index);
        if candidate == key {
            return Some(object[index..end].trim());
        }
        index = skip_whitespace(bytes, end);
        match bytes.get(index) {
            Some(b',') => index += 1,
            _ => return None,
        }
    }
}

fn json_items(array: &str) -> Vec<&str> {
    let bytes = array.as_bytes();
    let mut index = skip_whitespace(bytes, 0);
    assert_eq!(bytes.get(index), Some(&b'['), "expected a JSON array");
    index += 1;
    let mut result = Vec::new();
    loop {
        index = skip_whitespace(bytes, index);
        if bytes.get(index) == Some(&b']') {
            return result;
        }
        let end = value_end(bytes, index);
        result.push(array[index..end].trim());
        index = skip_whitespace(bytes, end);
        match bytes.get(index) {
            Some(b',') => index += 1,
            Some(b']') => return result,
            _ => panic!("invalid JSON array"),
        }
    }
}

fn json_string(object: &str, key: &str) -> Option<String> {
    let value = json_value(object, key)?;
    if value == "null" {
        return None;
    }
    assert!(value.starts_with('"') && value.ends_with('"'));
    let mut result = String::new();
    let mut chars = value[1..value.len() - 1].chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        match chars.next().expect("JSON escape must have a value") {
            '"' => result.push('"'),
            '\\' => result.push('\\'),
            '/' => result.push('/'),
            'b' => result.push('\u{0008}'),
            'f' => result.push('\u{000c}'),
            'n' => result.push('\n'),
            'r' => result.push('\r'),
            't' => result.push('\t'),
            escape => panic!("unsupported JSON escape in fixture: {escape}"),
        }
    }
    Some(result)
}

fn json_usize(object: &str, key: &str) -> usize {
    json_value(object, key)
        .expect("JSON key must exist")
        .parse()
        .expect("JSON value must be an unsigned integer")
}

fn compact_json(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut result = String::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let end = string_end(bytes, index);
                result.push_str(&value[index..end]);
                index = end;
            }
            byte if byte.is_ascii_whitespace() => index += 1,
            byte => {
                result.push(char::from(byte));
                index += 1;
            }
        }
    }
    result
}

fn declaration_named<'a>(abi: &'a str, name: &str) -> &'a str {
    json_items(json_value(abi, "declarations").expect("ABI must contain declarations"))
        .into_iter()
        .find(|declaration| json_string(declaration, "name").as_deref() == Some(name))
        .expect("ABI declaration must exist")
}

fn field_named<'a>(declaration: &'a str, name: &str) -> &'a str {
    json_items(json_value(declaration, "fields").expect("struct must contain fields"))
        .into_iter()
        .find(|field| json_string(field, "name").as_deref() == Some(name))
        .expect("ABI field must exist")
}

fn type_at(abi: &str, index: usize) -> &str {
    json_items(json_value(abi, "unique_types").expect("ABI must contain unique types"))[index]
}

fn type_for_message<'a>(abi: &'a str, message: &str) -> &'a str {
    type_at(abi, json_usize(message, "body_ty_idx"))
}

fn type_description(abi: &str, ty_idx: usize) -> Option<String> {
    let ty = type_at(abi, ty_idx);
    let declaration_name = match json_string(ty, "kind").as_deref() {
        Some("StructRef") => json_string(ty, "struct_name"),
        Some("AliasRef") => json_string(ty, "alias_name"),
        Some("EnumRef") => json_string(ty, "enum_name"),
        _ => None,
    }?;
    json_string(declaration_named(abi, &declaration_name), "description")
}

fn method_named<'a>(abi: &'a str, name: &str) -> &'a str {
    json_items(json_value(abi, "get_methods").expect("ABI must contain get methods"))
        .into_iter()
        .find(|method| json_string(method, "name").as_deref() == Some(name))
        .expect("ABI get method must exist")
}

fn raw_address(value: &str) -> acton_client::StdAddr {
    acton_client::StdAddr::from_str(value).expect("raw fixture address must parse")
}

// Upstream: HasNotInitializedStorage.spec.ts — "toShard and workchain".
#[test]
fn has_not_initialized_storage_to_shard_and_workchain() {
    use acton_client::__private::tycho_types::cell::CellBuilder;
    use acton_client::{DeployedAddressOptions, ToShard};
    use num_bigint::BigInt;

    let collection =
        raw_address("0:ca6e321c7cce9ecedf0a8ca2492ec8592494aa5fb5ce0387dff96ef6af982a3e");
    let zero = raw_address("0:0000000000000000000000000000000000000000000000000000000000000000");
    let masterchain =
        raw_address("-1:28e8033db1467cacea8b158f0f61e682de06e8a5947504c904f1f703d2be4d9e");
    let first = has_not_initialized_storage::HasNotInitializedStorage::from_storage_with_options(
        &has_not_initialized_storage::NftItemStorageNotInitialized {
            item_index: BigInt::from(123_u16),
            collection_address: collection.clone(),
        },
        DeployedAddressOptions {
            workchain: 9,
            to_shard: Some(ToShard {
                fixed_prefix_length: 10,
                close_to: collection.clone(),
            }),
            ..Default::default()
        },
    )
    .expect("first contract must initialize");
    let second = has_not_initialized_storage::HasNotInitializedStorage::from_storage_with_options(
        &has_not_initialized_storage::NftItemStorageNotInitialized {
            item_index: BigInt::from(456_u16),
            collection_address: collection.clone(),
        },
        DeployedAddressOptions {
            to_shard: Some(ToShard {
                fixed_prefix_length: 2,
                close_to: zero,
            }),
            override_contract_code: Some(acton_client::Cell::default()),
            ..Default::default()
        },
    )
    .expect("second contract must initialize");
    let third = has_not_initialized_storage::HasNotInitializedStorage::from_storage_with_options(
        &has_not_initialized_storage::NftItemStorageNotInitialized {
            item_index: BigInt::from(789_u16),
            collection_address: collection.clone(),
        },
        DeployedAddressOptions {
            workchain: -1,
            to_shard: Some(ToShard {
                fixed_prefix_length: 1,
                close_to: masterchain.clone(),
            }),
            override_contract_code: Some(acton_client::Cell::default()),
        },
    )
    .expect("third contract must initialize");
    let mut custom_code = CellBuilder::new();
    custom_code
        .store_u64(789)
        .expect("custom code must fit into a cell");
    let fourth = has_not_initialized_storage::HasNotInitializedStorage::from_storage_with_options(
        &has_not_initialized_storage::NftItemStorageNotInitialized {
            item_index: BigInt::from(1_000_u16),
            collection_address: collection,
        },
        DeployedAddressOptions {
            workchain: 127,
            to_shard: Some(ToShard {
                fixed_prefix_length: 0,
                close_to: masterchain,
            }),
            override_contract_code: Some(custom_code.build().expect("custom code must build")),
        },
    )
    .expect("fourth contract must initialize");

    expect![[r#"
        (
            "9:ca4a4dd3ad745b3c17f299c102ac7fcbd39548b62417c8d6c8022c238e22a919",
            "0:10cffea844f6cb04b860db26282f8027c9049b4a2da3640ea4a692af5516b330",
            "-1:6feecfe4cc67f96ee6ef9cc94fcd55727f3421d96919fc0d7a9342570b234ebb",
            "127:2f3f5831a36bf2e1b8af9676492bbed9f87aca52991614470beb3dde493e3f4b",
        )
    "#]]
    .assert_debug_eq(&(
        first.address().to_string(),
        second.address().to_string(),
        third.address().to_string(),
        fourth.address().to_string(),
    ));
}

// Upstream: HasNotInitializedStorage.spec.ts — "fromStorage takes only 2 args".
#[test]
fn has_not_initialized_storage_from_storage_takes_only_storage() {
    use num_bigint::BigInt;

    let deploy_storage = has_not_initialized_storage::NftItemStorageNotInitialized {
        item_index: BigInt::from(10_u8),
        collection_address: raw_address(
            "0:0000000000000000000000000000000000000000000000000000000000000000",
        ),
    };
    let contract =
        has_not_initialized_storage::HasNotInitializedStorage::from_storage(&deploy_storage)
            .expect("contract must initialize from deployment storage");
    let data = &contract
        .init()
        .expect("contract must expose state init")
        .data;
    let actual =
        has_not_initialized_storage::HasNotInitializedStorage::<()>::storage_from_cell(data)
            .expect("deployment storage must decode");

    expect![[r"
        NftItemStorageNotInitialized {
            item_index: 10,
            collection_address: StdAddr {
                anycast: None,
                workchain: 0,
                address: 0000000000000000000000000000000000000000000000000000000000000000,
            },
        }
    "]]
    .assert_debug_eq(&actual);
}

// Upstream: HasNotInitializedStorage.spec.ts — "ABI contains notInitializedStorage".
#[test]
fn has_not_initialized_storage_abi_contains_not_initialized_storage() {
    let abi = has_not_initialized_storage::ABI_JSON;
    let storage = json_value(abi, "storage").expect("ABI must contain storage metadata");
    let ty = type_at(abi, json_usize(storage, "storage_at_deployment_ty_idx"));

    expect![[r#"{"kind":"StructRef","struct_name":"NftItemStorageNotInitialized"}"#]]
        .assert_eq(&compact_json(ty));
}

// Upstream: LotsOfStorage.spec.ts — "fromStorage has all defaults".
#[test]
fn lots_of_storage_from_storage_has_all_defaults() {
    use num_bigint::BigInt;

    let defaults = lots_of_storage::StWithAllDefaults::default();
    let mut s1 = defaults.s1.as_slice().expect("s1 must be readable");
    let s1_bits = s1.size_bits();
    let s1_value = s1.load_uint(16).expect("s1 must contain uint16");
    let s2_bits = defaults
        .s2
        .0
        .as_slice()
        .expect("s2 must be readable")
        .size_bits();
    let s4_bits = defaults
        .s4
        .as_slice()
        .expect("s4 must be readable")
        .size_bits();
    // Upstream replaces `toCell` with a spy because this intentionally huge
    // storage type cannot fit a TON cell. Calling the real Rust factory after
    // inspecting the defaults preserves that boundary and its real failure.
    let defaults_factory_error = match lots_of_storage::LotsOfStorage::from_storage(&defaults) {
        Ok(_) => panic!("all-default storage must exceed one TON cell"),
        Err(error) => error.to_string(),
    };

    expect![[r#"
        (
            (
                "1267650600228229401496703205376",
                true,
                true,
                true,
                true,
                16,
                258,
                8,
                64,
                "kopi",
                true,
                Std(
                    StdAddr {
                        anycast: None,
                        workchain: 0,
                        address: 0000000000000000000000000000000000000000000000000000000000000000,
                    },
                ),
            ),
            (
                true,
                Some(
                    (
                        1,
                        2,
                        3,
                    ),
                ),
                "9:527964d55cfa6eb731f4bfc07e9d025098097ef8505519e853986279bd8400d8",
                true,
                (
                    1,
                    None,
                ),
                (
                    (
                        1,
                        None,
                    ),
                    (
                        "10",
                    ),
                ),
                (
                    [
                        1,
                        2,
                        3,
                    ],
                    [
                        4,
                        5,
                        6,
                    ],
                ),
                false,
                Some(
                    Inner {
                        in1: 2,
                        in2: true,
                    },
                ),
                true,
            ),
            "cell overflow",
        )
    "#]]
    .assert_debug_eq(&(
        (
            defaults.i5.to_string(),
            defaults.i7.is_none(),
            defaults.e1 == lots_of_storage::Color::green(),
            defaults.e2 == lots_of_storage::E0Max::max_int(),
            defaults.b1,
            s1_bits,
            s1_value,
            s2_bits,
            s4_bits,
            defaults.s5.as_str(),
            defaults.a3.is_none(),
            &defaults.a4,
        ),
        (
            defaults.a5.is_none(),
            &defaults.t1,
            defaults.t2.1.to_string(),
            defaults.t5.is_none(),
            &defaults.sh1,
            &defaults.sh2,
            &defaults.arr1,
            defaults.o1.in2,
            &defaults.o2,
            defaults.o3.is_none(),
        ),
        defaults_factory_error,
    ));

    let overridden = lots_of_storage::StWithAllDefaults {
        i5: BigInt::from(10),
        t1: Some((BigInt::from(10), BigInt::from(20), BigInt::from(30))),
        i7: None,
        ..lots_of_storage::StWithAllDefaults::default()
    };
    let overridden_factory_error = match lots_of_storage::LotsOfStorage::from_storage(&overridden) {
        Ok(_) => panic!("overridden storage must exceed one TON cell"),
        Err(error) => error.to_string(),
    };
    expect![[r#"
        (
            "10",
            None,
            Some(
                (
                    10,
                    20,
                    30,
                ),
            ),
            "0:0000000000000000000000000000000000000000000000000000000000000000",
            "cell overflow",
        )
    "#]]
    .assert_debug_eq(&(
        overridden.i5.to_string(),
        &overridden.i7,
        &overridden.t1,
        overridden.a2.to_string(),
        overridden_factory_error,
    ));
}

fn default_json(declaration: &str, field: &str) -> String {
    compact_json(
        json_value(field_named(declaration, field), "default_value")
            .expect("fixture field must have a default"),
    )
}

// Upstream: LotsOfStorage.spec.ts — "ABI contains default values".
#[test]
fn lots_of_storage_abi_contains_default_values() {
    let declaration = declaration_named(lots_of_storage::ABI_JSON, "StWithAllDefaults");
    let fields =
        json_items(json_value(declaration, "fields").expect("storage struct must contain fields"));

    expect![[r"
        true
    "]]
    .assert_debug_eq(
        &fields
            .iter()
            .all(|field| json_value(field, "default_value").is_some()),
    );

    expect![[r#"
        (
            (
                "{\"kind\":\"castTo\",\"inner\":{\"kind\":\"int\",\"v\":\"50000000\"},\"cast_to_ty_idx\":6}",
                "{\"kind\":\"int\",\"v\":\"1267650600228229401496703205376\"}",
                "{\"kind\":\"null\"}",
                "{\"kind\":\"bool\",\"v\":false}",
                "{\"kind\":\"slice\",\"hex\":\"0102\"}",
                "{\"kind\":\"slice\",\"hex\":\"68656c6c6f312340\"}",
                "{\"kind\":\"address\",\"addr\":\"EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c\"}",
                "{\"kind\":\"null\"}",
            ),
            (
                "{\"kind\":\"castTo\",\"inner\":{\"kind\":\"address\",\"addr\":\"EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c\"},\"cast_to_ty_idx\":25}",
                "{\"kind\":\"tensor\",\"items\":[{\"kind\":\"null\"},{\"kind\":\"tensor\",\"items\":[{\"kind\":\"int\",\"v\":\"20\"},{\"kind\":\"int\",\"v\":\"30\"}]}]}",
                "{\"kind\":\"tensor\",\"items\":[{\"kind\":\"int\",\"v\":\"907060870\"},{\"kind\":\"int\",\"v\":\"50018\"},{\"kind\":\"int\",\"v\":\"20329878786436204988385760252021328656300425018755239228739303522659023427620\"},{\"kind\":\"int\",\"v\":\"754077114\"},{\"kind\":\"int\",\"v\":\"448378203247\"}]}",
                "{\"kind\":\"castTo\",\"inner\":{\"kind\":\"shapedTuple\",\"items\":[{\"kind\":\"int\",\"v\":\"1\"},{\"kind\":\"null\"}]},\"cast_to_ty_idx\":38}",
                "{\"kind\":\"castTo\",\"inner\":{\"kind\":\"shapedTuple\",\"items\":[{\"kind\":\"tensor\",\"items\":[{\"kind\":\"int\",\"v\":\"1\"},{\"kind\":\"null\"}]},{\"kind\":\"castTo\",\"inner\":{\"kind\":\"shapedTuple\",\"items\":[{\"kind\":\"string\",\"str\":\"10\"}]},\"cast_to_ty_idx\":40}]},\"cast_to_ty_idx\":43}",
                "{\"kind\":\"object\",\"struct_name\":\"Inner\",\"fields\":[{\"kind\":\"castTo\",\"inner\":{\"kind\":\"int\",\"v\":\"2\"},\"cast_to_ty_idx\":8},{\"kind\":\"castTo\",\"inner\":{\"kind\":\"bool\",\"v\":true},\"cast_to_ty_idx\":5}]}",
                "{\"kind\":\"null\"}",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        (
            default_json(declaration, "i2"),
            default_json(declaration, "i5"),
            default_json(declaration, "i7"),
            default_json(declaration, "b3"),
            default_json(declaration, "s1"),
            default_json(declaration, "s4"),
            default_json(declaration, "a2"),
            default_json(declaration, "a3"),
        ),
        (
            default_json(declaration, "a4"),
            default_json(declaration, "t3"),
            default_json(declaration, "t4"),
            default_json(declaration, "sh1"),
            default_json(declaration, "sh2"),
            default_json(declaration, "o2"),
            default_json(declaration, "o3"),
        ),
    ));
}

fn compiler_version_matches_upstream_pattern(version: &str) -> bool {
    let parts = version.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts[0] == "1"
        && parts[1].chars().all(|character| character.is_ascii_digit())
        && !parts[1].is_empty()
        && parts[2].chars().all(|character| character.is_ascii_digit())
        && !parts[2].is_empty()
}

// Upstream: LotsOfAnnotations.spec.ts — "ABI common properties".
#[test]
fn lots_of_annotations_abi_common_properties() {
    let abi = lots_of_annotations::ABI_JSON;

    expect![[r#"
        (
            "LotsOfAnnotations",
            Some(
                "A K",
            ),
            Some(
                "1.0",
            ),
            Some(
                "some d",
            ),
            "tolk",
            true,
        )
    "#]]
    .assert_debug_eq(&(
        lots_of_annotations::CONTRACT_NAME,
        json_string(abi, "author"),
        json_string(abi, "version"),
        json_string(abi, "description"),
        lots_of_annotations::COMPILER_NAME,
        compiler_version_matches_upstream_pattern(lots_of_annotations::COMPILER_VERSION),
    ));
}

// Upstream: LotsOfAnnotations.spec.ts — "ABI for incoming messages".
#[test]
fn lots_of_annotations_abi_for_incoming_messages() {
    let abi = lots_of_annotations::ABI_JSON;
    let messages = json_items(
        json_value(abi, "incoming_messages").expect("ABI must contain incoming messages"),
    );
    let msg1 = messages
        .iter()
        .copied()
        .find(|message| {
            json_string(type_for_message(abi, message), "struct_name").as_deref() == Some("Msg1")
        })
        .expect("Msg1 incoming message must exist");
    let generic = messages
        .iter()
        .copied()
        .find(|message| json_value(type_for_message(abi, message), "type_args_ty_idx").is_some())
        .expect("generic incoming message must exist");
    let external = json_items(
        json_value(abi, "incoming_external").expect("ABI must contain external messages"),
    )[0];
    let external_type = type_for_message(abi, external);

    expect![[r#"
        (
            Some(
                "mmm1\nmmm2",
            ),
            Some(
                "mmmReset",
            ),
            "{\"kind\":\"StructRef\",\"struct_name\":\"ActualExternalShape\"}",
            Some(
                "mmmShape",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        type_description(abi, json_usize(msg1, "body_ty_idx")),
        type_description(abi, json_usize(generic, "body_ty_idx")),
        compact_json(external_type),
        type_description(abi, json_usize(external, "body_ty_idx")),
    ));
}

// Upstream: LotsOfAnnotations.spec.ts — "ABI for get methods".
#[test]
fn lots_of_annotations_abi_for_get_methods() {
    let method = method_named(lots_of_annotations::ABI_JSON, "getFirst");
    let parameter =
        json_items(json_value(method, "parameters").expect("get method must contain parameters"))
            [0];

    expect![[r#"
        (
            Some(
                "get1",
            ),
            90137,
            Some(
                "spec",
            ),
            Some(
                "some number",
            ),
            "{\"kind\":\"int\",\"v\":\"50\"}",
            [
                GetMethod {
                    name: "getFirst",
                    method_id: 90137,
                },
            ],
        )
    "#]]
    .assert_debug_eq(&(
        json_string(method, "description"),
        json_usize(method, "tvm_method_id"),
        json_string(parameter, "name"),
        json_string(parameter, "description"),
        compact_json(
            json_value(parameter, "default_value")
                .expect("get method parameter must have a default"),
        ),
        lots_of_annotations::GET_METHODS,
    ));
}

// Upstream: LotsOfAnnotations.spec.ts — "ABI for createMessage".
#[test]
fn lots_of_annotations_abi_for_create_message() {
    let abi = lots_of_annotations::ABI_JSON;
    let outgoing_types = json_items(
        json_value(abi, "outgoing_messages").expect("ABI must contain outgoing messages"),
    )
    .into_iter()
    .map(|message| compact_json(type_for_message(abi, message)))
    .collect::<Vec<_>>();

    expect![[r#"
        [
            "{\"kind\":\"intN\",\"n\":8}",
            "{\"kind\":\"StructRef\",\"struct_name\":\"Transfer\"}",
            "{\"kind\":\"StructRef\",\"struct_name\":\"Out2\"}",
            "{\"kind\":\"StructRef\",\"struct_name\":\"Out3\",\"type_args_ty_idx\":[24]}",
        ]
    "#]]
    .assert_debug_eq(&outgoing_types);
}

// Upstream: LotsOfAnnotations.spec.ts — "ABI for external logs".
#[test]
fn lots_of_annotations_abi_for_external_logs() {
    let abi = lots_of_annotations::ABI_JSON;
    let events =
        json_items(json_value(abi, "emitted_events").expect("ABI must contain emitted events"));
    let event = events[0];

    expect![[r#"
        (
            1,
            "{\"kind\":\"StructRef\",\"struct_name\":\"OutExt4\"}",
            Some(
                "mmmOut4",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        events.len(),
        compact_json(type_for_message(abi, event)),
        type_description(abi, json_usize(event, "body_ty_idx")),
    ));
}

// Upstream: LotsOfAnnotations.spec.ts — "ABI for fields".
#[test]
fn lots_of_annotations_abi_for_fields() {
    let transfer = declaration_named(lots_of_annotations::ABI_JSON, "Transfer");
    let forward_payload = field_named(transfer, "forwardPayload");

    expect![[r#"
        Some(
            "actually it's not a slice",
        )
    "#]]
    .assert_debug_eq(&json_string(forward_payload, "description"));
}

fn default_value_is_supported(abi: &str, ty_idx: usize) -> bool {
    let ty = type_at(abi, ty_idx);
    match json_string(ty, "kind").as_deref() {
        Some("arrayOf" | "lispListOf") => {
            default_value_is_supported(abi, json_usize(ty, "inner_ty_idx"))
        }
        Some("tensor" | "shapedTuple") => json_items(
            json_value(ty, "items_ty_idx").expect("tuple type must contain item indices"),
        )
        .into_iter()
        .all(|item| {
            default_value_is_supported(
                abi,
                item.parse()
                    .expect("type index must be an unsigned integer"),
            )
        }),
        Some("union" | "mapKV") => false,
        Some(_) => true,
        None => panic!("ABI type must have a kind"),
    }
}

// Upstream: LotsOfThrows.spec.ts — "const Errors".
#[test]
fn lots_of_throws_const_errors() {
    let errors = [
        ("Err.EInEnum1", lots_of_throws::errors::ERR_E_IN_ENUM1),
        ("Err.EInEnum2", lots_of_throws::errors::ERR_E_IN_ENUM2),
        ("ERR_100", lots_of_throws::errors::ERR_100),
        ("ERR_101", lots_of_throws::errors::ERR_101),
        ("ERR_102", lots_of_throws::errors::ERR_102),
        ("ERR_103", lots_of_throws::errors::ERR_103),
        ("ERR_104", lots_of_throws::errors::ERR_104),
        ("ERR_105", lots_of_throws::errors::ERR_105),
        ("AGAIN_105", lots_of_throws::errors::AGAIN_105),
        (
            "CantDeserializePoint",
            lots_of_throws::errors::CANT_DESERIALIZE_POINT,
        ),
        (
            "CantGetMapElement",
            lots_of_throws::errors::CANT_GET_MAP_ELEMENT,
        ),
    ];
    let named_in_abi = json_items(
        json_value(lots_of_throws::ABI_JSON, "thrown_errors")
            .expect("ABI must contain thrown errors"),
    )
    .into_iter()
    .filter_map(|error| json_string(error, "name"))
    .collect::<Vec<_>>();

    expect![[r#"
        (
            [
                (
                    "Err.EInEnum1",
                    80,
                ),
                (
                    "Err.EInEnum2",
                    81,
                ),
                (
                    "ERR_100",
                    100,
                ),
                (
                    "ERR_101",
                    101,
                ),
                (
                    "ERR_102",
                    102,
                ),
                (
                    "ERR_103",
                    103,
                ),
                (
                    "ERR_104",
                    104,
                ),
                (
                    "ERR_105",
                    105,
                ),
                (
                    "AGAIN_105",
                    105,
                ),
                (
                    "CantDeserializePoint",
                    9999,
                ),
                (
                    "CantGetMapElement",
                    10000,
                ),
            ],
            11,
            false,
        )
    "#]]
    .assert_debug_eq(&(
        errors,
        errors.len(),
        named_in_abi.iter().any(|name| name == "D_ERR"),
    ));
}

// Upstream: LotsOfThrows.spec.ts — "ABI contains unnamed throws".
#[test]
fn lots_of_throws_abi_contains_unnamed_throws() {
    let thrown = json_items(
        json_value(lots_of_throws::ABI_JSON, "thrown_errors")
            .expect("ABI must contain thrown errors"),
    );
    let unnamed = thrown
        .iter()
        .copied()
        .filter(|error| json_value(error, "name").is_none())
        .map(|error| {
            (
                json_usize(error, "err_code"),
                json_string(error, "kind").expect("thrown error must have a kind"),
            )
        })
        .collect::<Vec<_>>();

    expect![[r#"
        (
            [
                (
                    200,
                    "plain_int",
                ),
                (
                    1234,
                    "plain_int",
                ),
                (
                    2345,
                    "plain_int",
                ),
            ],
            false,
        )
    "#]]
    .assert_debug_eq(&(
        unnamed,
        thrown
            .iter()
            .any(|error| json_usize(error, "err_code") == 201),
    ));
}

// Upstream: LotsOfThrows.spec.ts — "ABI contains throws description".
#[test]
fn lots_of_throws_abi_contains_throws_description() {
    let thrown = json_items(
        json_value(lots_of_throws::ABI_JSON, "thrown_errors")
            .expect("ABI must contain thrown errors"),
    );
    let description = |name: &str| {
        let error = thrown
            .iter()
            .copied()
            .find(|error| json_string(error, "name").as_deref() == Some(name))
            .expect("named thrown error must exist");
        json_string(error, "description")
    };

    expect![[r#"
        (
            Some(
                "desc for 105",
            ),
            Some(
                "desc for EInEnum2",
            ),
        )
    "#]]
    .assert_debug_eq(&(description("ERR_105"), description("Err.EInEnum2")));
}

// Upstream: LotsOfThrows.spec.ts — "ABI contains external message".
#[test]
fn lots_of_throws_abi_contains_external_message() {
    let abi = lots_of_throws::ABI_JSON;
    let external = json_items(
        json_value(abi, "incoming_external").expect("ABI must contain external messages"),
    );

    expect![[r#"
        (
            1,
            Some(
                "slice",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        external.len(),
        json_string(type_for_message(abi, external[0]), "kind"),
    ));
}

// Upstream: LotsOfThrows.spec.ts — "ABI with unsupported defaults".
#[test]
fn lots_of_throws_abi_with_unsupported_defaults() {
    let abi = lots_of_throws::ABI_JSON;
    let declaration = declaration_named(abi, "WithUnsupportedDefaults");
    let fields = json_items(
        json_value(declaration, "fields").expect("struct declaration must contain fields"),
    )
    .into_iter()
    .map(|field| {
        (
            json_string(field, "name").expect("field must have a name"),
            json_value(field, "default_value").is_some(),
            default_value_is_supported(abi, json_usize(field, "ty_idx")),
        )
    })
    .collect::<Vec<_>>();

    expect![[r#"
        [
            (
                "f1",
                true,
                false,
            ),
            (
                "f2",
                true,
                false,
            ),
            (
                "f3",
                true,
                false,
            ),
            (
                "f4",
                true,
                false,
            ),
            (
                "f5",
                true,
                false,
            ),
            (
                "f6",
                true,
                false,
            ),
        ]
    "#]]
    .assert_debug_eq(&fields);
}

// Upstream: OnlyHeader.spec.ts — "ABI contains outgoing messages from contract header".
#[test]
fn only_header_abi_contains_outgoing_messages_from_contract_header() {
    let abi = only_header::ABI_JSON;
    let outgoing = json_items(
        json_value(abi, "outgoing_messages").expect("ABI must contain outgoing messages"),
    )
    .into_iter()
    .map(|message| compact_json(type_for_message(abi, message)))
    .collect::<Vec<_>>();
    let alias = declaration_named(abi, "AliasOutMsgB");
    let structure = declaration_named(abi, "OutMsgB");

    expect![[r#"
        (
            [
                "{\"kind\":\"StructRef\",\"struct_name\":\"OutMsgA\"}",
                "{\"kind\":\"AliasRef\",\"alias_name\":\"AliasOutMsgB\"}",
            ],
            Some(
                "desc OutMsgB",
            ),
            Some(
                "desc OutMsgB",
            ),
        )
    "#]]
    .assert_debug_eq(&(
        outgoing,
        json_string(alias, "description"),
        json_string(structure, "description"),
    ));
}

// Upstream: OnlyHeader.spec.ts — "ABI contains emitted events from contract header".
#[test]
fn only_header_abi_contains_emitted_events_from_contract_header() {
    let abi = only_header::ABI_JSON;
    let emitted =
        json_items(json_value(abi, "emitted_events").expect("ABI must contain emitted events"))
            .into_iter()
            .map(|message| compact_json(type_for_message(abi, message)))
            .collect::<Vec<_>>();

    expect![[r#"
        [
            "{\"kind\":\"StructRef\",\"struct_name\":\"OutMsgExtA\"}",
        ]
    "#]]
    .assert_debug_eq(&emitted);
}

// Upstream: OnlyHeader.spec.ts — "ABI contains thrown errors from contract header".
#[test]
fn only_header_abi_contains_thrown_errors_from_contract_header() {
    let abi = only_header::ABI_JSON;
    let thrown =
        json_items(json_value(abi, "thrown_errors").expect("ABI must contain thrown errors"))
            .into_iter()
            .map(compact_json)
            .collect::<Vec<_>>();
    let error_enum = declaration_named(abi, "ErrCo");
    let members = json_items(
        json_value(error_enum, "members").expect("enum declaration must contain members"),
    )
    .into_iter()
    .map(compact_json)
    .collect::<Vec<_>>();

    expect![[r#"
        (
            [
                "{\"kind\":\"enum_member\",\"name\":\"ErrCo.NotFound\",\"description\":\"desc NotFound\",\"err_code\":404}",
            ],
            [
                "{\"name\":\"NotFound\",\"value\":\"404\",\"description\":\"desc NotFound\"}",
            ],
            404,
        )
    "#]]
    .assert_debug_eq(&(
        thrown,
        members,
        only_header::errors::ERR_CO_NOT_FOUND,
    ));
}
