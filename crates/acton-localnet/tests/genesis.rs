use acton_localnet::{CreateNetwork, GenesisConfig, catalog};
use expect_test::expect;

#[tokio::test]
async fn resolved_genesis_survives_catalog_reload_and_exports_the_same_document() {
    let root = tempfile::tempdir().unwrap();
    let config = GenesisConfig::from_toml("global_id = -42\n[storage]\ncell_price = 123").unwrap();
    let created = catalog::create(
        root.path(),
        CreateNetwork {
            name: "custom".into(),
            genesis: Some(config),
            block_time_ms: Some(600),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let saved = catalog::list(root.path()).await.unwrap().remove(0);
    let genesis = saved.network.config.genesis.unwrap();
    let document = std::fs::read_to_string(created.path.join("genesis.toml")).unwrap();
    expect![["-42 123 600 true"]].assert_eq(&format!(
        "{} {} {} {}",
        genesis.global_id,
        genesis.storage.cell_price,
        genesis.simplex_target_rate_ms,
        GenesisConfig::from_toml(&document).unwrap() == genesis,
    ));
}

#[tokio::test]
async fn invalid_genesis_does_not_create_a_network_definition() {
    let root = tempfile::tempdir().unwrap();
    let invalid = GenesisConfig {
        global_id: 0,
        ..Default::default()
    };
    let error = catalog::create(
        root.path(),
        CreateNetwork {
            name: "invalid".into(),
            genesis: Some(invalid),
            ..Default::default()
        },
    )
    .await
    .err()
    .expect("invalid genesis");
    let masterchain = GenesisConfig {
        workchain_enabled: false,
        ..Default::default()
    };
    let imports_error = catalog::create(
        root.path(),
        CreateNetwork {
            name: "imports".into(),
            genesis: Some(masterchain),
            imported_account_bocs: vec!["00".into()],
            ..Default::default()
        },
    )
    .await
    .err()
    .expect("invalid genesis");
    expect![[r"
        Invalid genesis: Network ID must be negative
        Enable the workchain to add startup wallets or import accounts
        saved: 0"]]
    .assert_eq(&format!(
        "{error}\n{imports_error}\nsaved: {}",
        catalog::list(root.path()).await.unwrap().len()
    ));
}
