use super::*;
use expect_test::expect;

#[test]
fn genesis_toml_reaches_protocol_parameters_without_unexpanded_markers() {
    let directory = tempfile::tempdir_in("/tmp").unwrap();
    let layout = Layout::new(directory.path().join("state"));
    layout.create_bootstrap_dirs().unwrap();
    let config = NetworkSettings::from_toml(
        r#"
global_id = -42
global_version = 14
global_capabilities = 1004
gas_price = 123
cell_price = 456
validator_wallet_balance = 60000
[workchain]
min_split = 1
max_split = 3
[storage]
cell_price = 789
[gas_masterchain]
gas_credit = 234
[block_limits.bytes]
hard = 4000000
[critical_voting]
max_rounds = 9
"#,
    )
    .unwrap();
    write_genesis_script(&layout, &config, false).unwrap();
    let rendered = fs::read_to_string(layout.smartcont.join("gen-zerostate.fif")).unwrap();
    let selected = rendered
        .lines()
        .filter(|line| {
            line.contains("setglobalid")
                || line.contains("add-std-workchain")
                || line.contains("config.version!")
                || line.contains("config.storage_prices!")
                || line.contains("config.gas_prices!")
                || line.contains("config.mc_gas_prices!")
                || line.contains("config.fwd_prices!")
                || line.contains("config.block_limits!")
                || line.starts_with("_(")
        })
        .filter(|line| !line.starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    expect![[r"
        -42 setglobalid   // negative value means a test instance of the blockchain
        basestate0_rhash basestate0_fhash now 0 1 3 0 add-std-workchain
        14 1004 config.version!
        1 789 1000 500000 config.storage_prices!
        123 1000000 1000000 10000 10000000 100000000 1000000000 100 40000 config.gas_prices!
        655360000 1000000 70000000 234 2500000 100000000 1000000000 100 1000000 config.mc_gas_prices!
        400000 26214400 456 98304 21845 21845 config.fwd_prices!
        262144 1048576 4000000 triple 2000000 10000000 20000000 triple 1000 5000 10000 triple config.block_limits!
        _( 2 3 2 2 1000000 10000000 1 500 )
        _( 4 9 4 2 5000000 20000000 2 1000 )"]].assert_eq(&selected);
    let placeholders = [
        "GLOBAL_ID",
        "GAS_PRICE",
        "GAS_LIMITS",
        "FORWARDING_PRICES",
        "STORAGE_PRICES",
        "BLOCK_LIMITS",
        "VOTING_PARAMS",
        "CATCHAIN_PARAMS",
        "WORKCHAIN_MIN_SPLIT",
        "VALIDATOR_WALLET_BALANCE",
    ];
    expect![["[]\n"]].assert_debug_eq(
        &placeholders
            .into_iter()
            .filter(|marker| rendered.contains(marker))
            .collect::<Vec<_>>(),
    );

    let masterchain_only = NetworkSettings {
        workchain_enabled: false,
        ..config
    };
    write_genesis_script(&layout, &masterchain_only, false).unwrap();
    let rendered = fs::read_to_string(layout.smartcont.join("gen-zerostate.fif")).unwrap();
    expect![["false"]].assert_eq(&rendered.contains("add-std-workchain").to_string());
}

#[tokio::test]
#[ignore = "requires LOCALTON_TEST_TON_BIN_DIR with official TON binaries and smartcont resources"]
async fn custom_genesis_is_accepted_by_create_state() {
    let root = std::env::var_os("LOCALTON_TEST_TON_BIN_DIR").expect("TON binary directory");
    let directory = tempfile::tempdir_in("/tmp").unwrap();
    let layout = Layout::new(directory.path().join("state"));
    layout.create_bootstrap_dirs().unwrap();
    let binaries = crate::binaries::TonBinaries { root: root.into() };
    crate::bootstrap::files::copy_tree(&binaries.smartcont_dir(), &layout.smartcont).unwrap();
    fs::write(layout.smartcont.join("validator-keys-1.pub"), [17; 32]).unwrap();
    let tools = crate::ton::toolchain::Toolchain::official(layout.clone(), binaries);
    let network = NetworkSettings::from_toml("global_id = -42\ngas_price = 123456\n[storage]\ncell_price = 789\n[workchain]\nmin_split = 1\nmax_split = 3").unwrap();
    let states = create_genesis_states(
        &layout,
        tools.create_state.as_ref(),
        &OperationContext::for_node(std::time::Duration::from_secs(30), "genesis"),
        &network,
        &[],
    )
    .await
    .unwrap();
    let state = BocRepr::decode::<ShardStateUnsplit, _>(
        fs::read(layout.zerostate.join("zerostate.boc")).unwrap(),
    )
    .unwrap();
    let extra = state.custom.unwrap().load().unwrap();
    let gas = extra.config.params.get_gas_prices(false).unwrap();
    let prices = extra
        .config
        .params
        .get_storage_prices()
        .unwrap()
        .get(0)
        .unwrap()
        .unwrap();
    expect![["-42 123456 789 true"]].assert_eq(&format!(
        "{} {} {} {}",
        state.global_id,
        gas.gas_price,
        prices.cell_price_ps,
        states.masterchain.id().root_hash() != states.basechain.id().root_hash()
    ));
}
