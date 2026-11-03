use expect_test::{expect, expect_file};
use localton_genesis::GenesisConfig;

#[test]
fn default_document_is_complete_and_round_trips() {
    let config = GenesisConfig::default();
    let source = config.to_toml().unwrap();
    expect_file!["../genesis.toml"].assert_eq(&source);
    expect![["true"]]
        .assert_eq(&(GenesisConfig::from_toml(&source).unwrap() == config).to_string());
}

#[test]
fn partial_tables_keep_chain_specific_defaults_and_large_integers() {
    let config = GenesisConfig::from_toml(
        r"
global_id = -42
cell_price = 9007199254740993
[gas_masterchain]
gas_credit = 123
[forwarding_masterchain]
next_frac = 1234
[critical_voting]
max_rounds = 9
[block_limits.bytes]
hard = 4000000
",
    )
    .unwrap();
    expect![["-42 9007199254740993 123 70000000 1000000 10000000 1234 4 9 262144 4000000"]]
        .assert_eq(&format!(
            "{} {} {} {} {} {} {} {} {} {} {}",
            config.global_id,
            config.cell_price,
            config.gas_masterchain.gas_credit,
            config.gas_masterchain.special_gas_limit,
            config.gas_masterchain.flat_gas_price,
            config.forwarding_masterchain.lump_price,
            config.forwarding_masterchain.next_frac,
            config.critical_voting.min_rounds,
            config.critical_voting.max_rounds,
            config.block_limits.bytes.underload,
            config.block_limits.bytes.hard,
        ));
    expect![["true"]].assert_eq(
        &(GenesisConfig::from_toml(&config.to_toml().unwrap()).unwrap() == config).to_string(),
    );
}

#[test]
fn rejects_unknown_keys_and_invalid_protocol_values() {
    let cases = [
        ("unknown root", "gas_prcie = 1"),
        ("unknown nested", "[gas]\ngsa_limit = 1"),
        ("wrong type", "workchain_enabled = 1"),
        ("positive id", "global_id = 1"),
        ("uint16 validators", "max_validators = 65536"),
        ("stake ordering", "min_validator_stake = 10000001"),
        ("split depth", "[workchain]\nmin_split = 5\nmax_split = 4"),
        ("block thresholds", "[block_limits.bytes]\nsoft = 1"),
        ("gas credit", "[gas]\ngas_credit = 1000001"),
        ("debt limits", "[gas]\nfreeze_due_limit = 1000000001"),
        ("voting", "[voting]\nmin_wins = 4"),
        ("zero consensus", "simplex_target_rate_ms = 0"),
        ("uint64 overflow", "gas_price = 18446744073709551616"),
    ];
    let output = cases
        .map(|(name, source)| format!("{name}: {}", GenesisConfig::from_toml(source).is_err()))
        .join("\n");
    expect![[r"
        unknown root: true
        unknown nested: true
        wrong type: true
        positive id: true
        uint16 validators: true
        stake ordering: true
        split depth: true
        block thresholds: true
        gas credit: true
        debt limits: true
        voting: true
        zero consensus: true
        uint64 overflow: true"]]
    .assert_eq(&output);
}

#[test]
fn explicit_timing_overrides_the_file_and_derives_election_windows() {
    let config = GenesisConfig::from_toml("simplex_target_rate_ms = 2000")
        .unwrap()
        .with_timing(Some(500), Some(40))
        .unwrap();
    expect![["500 40 30 10 10 30"]].assert_eq(&format!(
        "{} {} {} {} {} {}",
        config.simplex_target_rate_ms,
        config.elected_for_seconds,
        config.election_start_before_seconds,
        config.election_end_before_seconds,
        config.stakes_frozen_for_seconds,
        config.original_validator_set_valid_for_seconds,
    ));
}
