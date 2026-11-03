use super::*;
use acton_studio::{EnvironmentEndpoints, EnvironmentStatus};
use expect_test::expect;

#[tokio::test]
async fn custom_genesis_wallets_match_startup_accounts_and_keep_explicit_ids() {
    let source = r#"[wallets.v5]
kind = "v5r1"
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;
    let document: toml::Value = toml::from_str(source).unwrap();
    let mut config = ActonConfig {
        wallets: Some(document["wallets"].clone().try_into().unwrap()),
        ..Default::default()
    };
    let wallets = &mut config.wallets.as_mut().unwrap().wallets;
    let mut explicit = wallets["v5"].clone();
    explicit.wallet_id = Some(42);
    wallets.insert("explicit".into(), explicit);
    let mut v4 = wallets["v5"].clone();
    v4.kind = "v4r2".into();
    wallets.insert("v4".into(), v4);
    let names = wallets.keys().cloned().collect();
    let runtime = ProjectWalletRuntime::new(&config).unwrap();
    let environment = full_ton_environment(-42);
    let listed = runtime.list(&environment).await.unwrap();
    let startup = runtime.prepare_localnet_accounts(names, -42).await.unwrap();
    let custom = runtime.wallets_for(&environment).unwrap();
    let mut lines = vec![format!("network: {}", environment.network.chain_id)];

    for prepared in startup {
        let shard = rston::boc::BocRepr::decode_hex::<rston::models::ShardAccount, _>(
            &prepared.shard_account_boc_hex,
        )
        .unwrap();
        let account = shard.load_account().unwrap().unwrap();
        let wallet = &custom[&prepared.name];
        let listed = listed
            .iter()
            .find(|value| value.name == prepared.name)
            .unwrap();
        let message = b"genesis wallet signing check";
        let signature = runtime
            .sign(&environment, &prepared.name, message.to_vec())
            .await
            .unwrap();
        let verifying_key =
            ed25519_dalek::VerifyingKey::from_bytes(&wallet.wallet.key_pair.public_key).unwrap();
        lines.push(format!(
            "{}: id={} startup={} listed={} changed={} signs={}",
            prepared.name,
            wallet.wallet.wallet_id,
            account.address == rston::models::IntAddr::Std(wallet.address()),
            listed.wallet_id == wallet.wallet.wallet_id,
            wallet.address() != runtime.localnet_wallets[&prepared.name].address(),
            verifying_key
                .verify_strict(message, &ed25519_dalek::Signature::from_bytes(&signature))
                .is_ok(),
        ));
    }
    expect![[r"
        network: -42
        explicit: id=42 startup=true listed=true changed=false signs=true
        v4: id=698983191 startup=true listed=true changed=false signs=true
        v5: id=2147483606 startup=true listed=true changed=true signs=true"]]
    .assert_eq(&lines.join("\n"));
}

#[test]
fn environments_use_wallets_with_their_global_id() {
    let runtime = ProjectWalletRuntime {
        config: ActonConfig::default(),
        localnet_wallets: BTreeMap::new(),
        mainnet_wallets: BTreeMap::new(),
    };
    let full_ton = full_ton_environment(-3);
    let testnet = remote_environment(PublicTonNetwork::Testnet);
    let mainnet = remote_environment(PublicTonNetwork::Mainnet);

    assert!(std::ptr::eq(
        runtime.wallets_for(&full_ton).unwrap().as_ref(),
        &runtime.localnet_wallets
    ));
    assert!(std::ptr::eq(
        runtime.wallets_for(&testnet).unwrap().as_ref(),
        &runtime.localnet_wallets
    ));
    assert!(std::ptr::eq(
        runtime.wallets_for(&mainnet).unwrap().as_ref(),
        &runtime.mainnet_wallets
    ));
}

fn full_ton_environment(global_id: i32) -> StudioEnvironment {
    StudioEnvironment::new(
        "full-ton",
        "Full TON",
        EnvironmentStatus::Running,
        EnvironmentConfig::FullTonNetwork {
            global_id,
            genesis_toml: None,
            api_v2_port: 18_080,
            api_v3_port: 18_081,
            admin_port: 18_082,
            config_port: 18_083,
            observability_port: 18_084,
            block_time_ms: None,
            election_time_seconds: None,
            imported_accounts: Vec::new(),
            accounts: Vec::new(),
            nodes: Vec::new(),
        },
        EnvironmentEndpoints::default(),
    )
}

fn remote_environment(network: PublicTonNetwork) -> StudioEnvironment {
    StudioEnvironment::new_external(
        "network",
        "Network",
        EnvironmentStatus::Running,
        EnvironmentConfig::RemoteTonNetwork { network },
        EnvironmentEndpoints::default(),
    )
}
