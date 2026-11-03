use expect_test::expect;
use localton_genesis::{GENESIS_SCHEMA_JSON, GenesisConfig};
use serde_json::{Value, json};

#[test]
fn schema_accepts_partial_tables_and_rejects_invalid_shapes_and_integer_widths() {
    let schema: Value = serde_json::from_str(GENESIS_SCHEMA_JSON).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let cases = [
        ("empty", json!({})),
        (
            "defaults",
            serde_json::to_value(GenesisConfig::default()).unwrap(),
        ),
        (
            "partial",
            json!({"gas_masterchain": {"gas_credit": 123}, "block_limits": {"gas": {"hard": 30000000}}}),
        ),
        ("large integer", json!({"cell_price": 9007199254740993_u64})),
        ("unknown root", json!({"gas_prcie": 1})),
        ("unknown nested", json!({"gas": {"gsa_limit": 1}})),
        ("wrong type", json!({"workchain_enabled": 1})),
        ("negative unsigned", json!({"storage": {"cell_price": -1}})),
        ("positive network id", json!({"global_id": 1})),
        ("uint8 overflow", json!({"voting": {"min_rounds": 256}})),
        (
            "uint32 overflow",
            json!({"block_limits": {"gas": {"hard": 4294967296_u64}}}),
        ),
        (
            "TOML integer overflow",
            json!({"cell_price": 9223372036854775808_u64}),
        ),
    ];
    let output = cases
        .map(|(name, value)| format!("{name}: {}", validator.is_valid(&value)))
        .join("\n");
    expect![[r"
        empty: true
        defaults: true
        partial: true
        large integer: true
        unknown root: false
        unknown nested: false
        wrong type: false
        negative unsigned: false
        positive network id: false
        uint8 overflow: false
        uint32 overflow: false
        TOML integer overflow: false"]]
    .assert_eq(&output);
}

#[test]
fn completion_defaults_match_the_chain_and_resource() {
    let schema: Value = serde_json::from_str(GENESIS_SCHEMA_JSON).unwrap();
    let properties = &schema["properties"];
    expect![["40000 1000000 10000000 4 262144 2000000 1000"]].assert_eq(&format!(
        "{} {} {} {} {} {} {}",
        properties["gas"]["properties"]["flat_gas_price"]["default"],
        properties["gas_masterchain"]["properties"]["flat_gas_price"]["default"],
        properties["forwarding_masterchain"]["properties"]["lump_price"]["default"],
        properties["critical_voting"]["properties"]["min_rounds"]["default"],
        properties["block_limits"]["properties"]["bytes"]["properties"]["underload"]["default"],
        properties["block_limits"]["properties"]["gas"]["properties"]["underload"]["default"],
        properties["block_limits"]["properties"]["logical_time"]["properties"]["underload"]["default"],
    ));
}
