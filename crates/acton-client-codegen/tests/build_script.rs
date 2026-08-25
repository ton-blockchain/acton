use acton_client_codegen::{GenerateOptions, generate, generate_from_file, generate_with_options};
use expect_test::expect;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use syn::Item;

fn counter_abi_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../acton-client/tests/fixtures/counter.abi.json")
}

#[test]
fn generated_source_is_ready_for_a_build_script() {
    let generated = generate_from_file(counter_abi_path()).expect("Counter ABI must generate");
    let file = syn::parse_file(&generated).expect("generated bindings must be valid Rust");
    let public_items = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Const(item) => Some(format!("const {}", item.ident)),
            Item::Enum(item) => Some(format!("enum {}", item.ident)),
            Item::Struct(item) => Some(format!("struct {}", item.ident)),
            Item::Type(item) => Some(format!("type {}", item.ident)),
            _ => None,
        })
        .collect::<Vec<_>>();

    expect![[r#"
        [
            "const ABI_JSON",
            "const ABI_SCHEMA_VERSION",
            "const CODE_BOC64",
            "const CONTRACT_NAME",
            "const COMPILER_NAME",
            "const COMPILER_VERSION",
            "struct Storage",
            "struct IncreaseCounter",
            "struct ResetCounter",
            "const GET_METHODS",
            "struct TolkCounter",
        ]
    "#]]
    .assert_debug_eq(&public_items);
}

#[test]
fn token_adapters_can_omit_the_embedded_abi() {
    let abi_json = fs::read_to_string(counter_abi_path()).expect("Counter ABI must be readable");
    let embedded = generate(&abi_json).expect("Counter ABI must generate");
    let external = generate_with_options(
        &abi_json,
        GenerateOptions {
            embed_abi_json: false,
        },
    )
    .expect("Counter ABI must generate without embedding JSON");

    expect![[r"
        (
            true,
            false,
        )
    "]]
    .assert_debug_eq(&(
        embedded.contains("pub const ABI_JSON"),
        external.contains("pub const ABI_JSON"),
    ));
}

#[test]
fn unsupported_schema_version_is_reported_before_generation() {
    let abi_json = fs::read_to_string(counter_abi_path()).expect("Counter ABI must be readable");
    let abi_json = abi_json.replacen(
        "\"abi_schema_version\": \"1.0\"",
        "\"abi_schema_version\": \"2.0\"",
        1,
    );
    let error = generate(&abi_json).expect_err("ABI schema 2.0 must be rejected");

    expect!["unsupported ABI schema version `2.0`; expected `1.0`"].assert_eq(&error.to_string());
}

#[test]
fn writes_generated_wrapper_to_a_file_like_upstream_cli() {
    let abi_json = r#"{
        "abi_schema_version": "1.0",
        "contract_name": "MyContract",
        "unique_types": [],
        "struct_instantiations": [],
        "alias_instantiations": [],
        "declarations": [],
        "storage": {},
        "incoming_messages": [],
        "incoming_external": [],
        "outgoing_messages": [],
        "emitted_events": [],
        "get_methods": [],
        "thrown_errors": [],
        "compiler_name": "tolk",
        "compiler_version": "dev",
        "code_boc64": "te6ccgEBAQEAAgAAAA=="
    }"#;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after Unix epoch")
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "acton-client-codegen-build-script-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir(&temp_dir).expect("temporary directory must be created");
    let abi_path = temp_dir.join("MyContract.abi.json");
    let output_path = temp_dir.join("my_contract.rs");
    fs::write(&abi_path, abi_json).expect("ABI fixture must be written");

    let generated = generate_from_file(&abi_path).expect("minimal ABI must generate");
    fs::write(&output_path, generated).expect("generated wrapper must be written");
    let written = fs::read_to_string(&output_path).expect("generated wrapper must be readable");

    expect![[r"
        (
            true,
            true,
            true,
        )
    "]]
    .assert_debug_eq(&(
        output_path.is_file(),
        written.contains("pub struct MyContract"),
        syn::parse_file(&written).is_ok(),
    ));

    fs::remove_dir_all(&temp_dir).expect("temporary directory must be removed");
}
