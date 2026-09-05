use std::net::Ipv4Addr;

use axum::{Json, Router, routing::get};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use expect_test::expect;
use tokio::net::TcpListener;

use crate::{
    cli::{JoinArgs, StateArgs},
    operations::validators,
    storage::{Layout, NodeRole, NodeSettings, Settings},
    ton::{global_config::GlobalConfig, tools::types::TonPublicKey},
};

use super::{state::prepare_join_state, validator::VALIDATOR_WALLET_WORKCHAIN};

fn global_config_fixture() -> serde_json::Value {
    let block = serde_json::json!({
        "@type": "ton.blockIdExt",
        "workchain": -1,
        "shard": i64::MIN,
        "seqno": 0,
        "root_hash": BASE64.encode([3_u8; 32]),
        "file_hash": BASE64.encode([4_u8; 32]),
    });
    serde_json::json!({
        "@type": "config.global",
        "dht": {
            "@type": "dht.config.global",
            "k": 3,
            "a": 3,
            "static_nodes": {
                "@type": "dht.nodes",
                "nodes": [{
                    "@type": "dht.node",
                    "id": {
                        "@type": "pub.ed25519",
                        "key": BASE64.encode([1_u8; 32]),
                    },
                    "addr_list": {
                        "@type": "adnl.addressList",
                        "addrs": [{
                            "@type": "adnl.address.udp",
                            "ip": 2_130_706_433_i32,
                            "port": 6302,
                        }],
                        "version": 0,
                        "reinit_date": 0,
                        "priority": 0,
                        "expire_at": 0,
                    },
                    "version": 0,
                    "signature": BASE64.encode([2_u8; 64]),
                }],
            },
        },
        "liteservers": [{
            "id": {
                "@type": "pub.ed25519",
                "key": BASE64.encode([5_u8; 32]),
            },
            "ip": 1,
            "port": 2,
        }],
        "validator": {
            "@type": "validator.config.global",
            "zero_state": block.clone(),
            "init_block": block,
        },
    })
}

#[test]
fn validator_wallet_is_masterchain_scoped() {
    assert_eq!(VALIDATOR_WALLET_WORKCHAIN, -1);
    let node = NodeSettings {
        role: NodeRole::Joined,
        name: "node2".to_owned(),
        ..NodeSettings::default()
    };
    assert_eq!(
        validators::validator_wallet_name(&node),
        "node2-validator-masterchain"
    );
}

#[test]
fn local_liteserver_config_preserves_upstream_network_endpoints() {
    let root = tempfile::tempdir_in("/tmp").unwrap();
    let layout = Layout::new(root.path().join("join"));
    layout.create_dirs().unwrap();
    let mut expected = global_config_fixture();
    crate::storage::write_json_atomic(&layout.global_config, &expected).unwrap();
    let upstream = std::fs::read(&layout.global_config).unwrap();
    let node_layout = layout.node.clone();
    node_layout.create_dirs().unwrap();
    let local_config = node_layout.global_config;

    let local_key = TonPublicKey::from_bytes([6_u8; 32]);
    GlobalConfig::load(&layout.global_config)
        .unwrap()
        .with_local_liteserver(38_007, local_key)
        .save_atomic(&local_config)
        .unwrap();

    assert_eq!(std::fs::read(&layout.global_config).unwrap(), upstream);
    let actual: serde_json::Value =
        serde_json::from_slice(&std::fs::read(local_config).unwrap()).unwrap();
    expected["liteservers"] = serde_json::json!([{
        "id": {
            "@type": "pub.ed25519",
            "key": local_key.to_base64(),
        },
        "ip": 2_130_706_433_i32,
        "port": 38_007,
    }]);
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn first_run_fetches_standard_global_config_and_configures_a_full_node() {
    let global_config = global_config_fixture();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/global.config.json",
                get(move || {
                    let global_config = global_config.clone();
                    async move { Json(global_config) }
                }),
            ),
        )
        .await
        .unwrap();
    });
    let root = tempfile::tempdir_in("/tmp").unwrap();
    let layout = Layout::new(root.path().join("join"));
    layout.create_dirs().unwrap();
    let args = JoinArgs {
        state: StateArgs {
            state_dir: layout.root.clone(),
        },
        node: Some("node2".to_owned()),
        global_config_url: format!("http://{address}/global.config.json"),
        faucet: None,
        advertise_ip: Ipv4Addr::new(10, 0, 0, 2),
        validator: true,
        observability_bind: Ipv4Addr::UNSPECIFIED,
        port_base: Some(41_000),
        no_observability: false,
        ton_bin_dir: None,
        celldb_in_memory: false,
        dump: None,
        startup_timeout: 1,
    };

    let prepared = prepare_join_state(&layout, &args).await.unwrap();
    server.abort();

    let settings = Settings::load(&layout.settings).unwrap();
    assert_eq!(prepared, settings);
    let node = &settings.node;
    let observability_port = settings.services.observability.port;
    let node_ports_are_contiguous = [
        node.console_port,
        node.adnl_port,
        node.liteserver_port,
        node.out_port,
        node.dht_port,
    ] == [
        node.console_port,
        node.console_port + 1,
        node.console_port + 2,
        node.console_port + 3,
        node.console_port + 4,
    ];
    let actual = serde_json::json!({
        "node": node.name,
        "allocation_starts_at_requested_base": observability_port >= 41_000,
        "node_ports_follow_observability": node.console_port == observability_port + 1,
        "node_ports_are_contiguous": node_ports_are_contiguous,
        "enabled": node.enabled,
        "validator": node.validator,
        "participate_in_elections": node.participate_in_elections,
        "advertise_ip": node.public_ip,
        "global_config_is_valid": GlobalConfig::from_json_bytes(
            &std::fs::read(&layout.global_config).unwrap()
        ).is_ok(),
        "zerostate_bundle_downloaded": layout.node.db.join("static").exists(),
        "private_keys_downloaded": layout.node.keyring.read_dir().unwrap().next().is_some(),
    });
    expect![[r#"
        {
          "advertise_ip": "10.0.0.2",
          "allocation_starts_at_requested_base": true,
          "enabled": true,
          "global_config_is_valid": true,
          "node": "node2",
          "node_ports_are_contiguous": true,
          "node_ports_follow_observability": true,
          "participate_in_elections": true,
          "private_keys_downloaded": false,
          "validator": true,
          "zerostate_bundle_downloaded": false
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual).unwrap());

    let mut retry_args = args;
    retry_args.validator = false;
    retry_args.port_base = Some(50_000);
    prepare_join_state(&layout, &retry_args).await.unwrap();
    let retried = Settings::load(&layout.settings).unwrap();
    assert_eq!(retried.services.observability.port, observability_port);
    let retried = &retried.node;
    assert!(retried.validator);
    assert!(retried.participate_in_elections);

    let node_manifest = layout.node.manifest.clone();
    std::fs::create_dir_all(node_manifest.parent().unwrap()).unwrap();
    std::fs::write(&node_manifest, "{}").unwrap();
    let mut disabled = Settings::load(&layout.settings).unwrap();
    disabled.node.participate_in_elections = false;
    disabled.save_atomic(&layout.settings).unwrap();

    prepare_join_state(&layout, &retry_args).await.unwrap();
    let restarted = Settings::load(&layout.settings).unwrap();
    let restarted = &restarted.node;
    assert!(restarted.validator);
    assert!(!restarted.participate_in_elections);
}
