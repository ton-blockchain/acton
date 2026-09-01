//! Connection status for the one node owned by a Localton state directory.

use anyhow::{Context, Result};

use crate::{
    cli::StatusArgs,
    storage::{Layout, Manifest, NodeManifest, NodeRole, Settings},
    ton::{
        global_config::GlobalConfigFile,
        toolchain::absolute_path,
        tools::{
            types::{DhtDatabase, TonPublicKey},
            validator_engine::ValidatorDatabase,
        },
    },
};

/// Prints connection data after validating the durable node and role-owned state.
///
/// Every state directory owns the same `node/` layout. A genesis node additionally
/// proves that the bootstrap manifest and DHT database are complete; a joined node
/// has no bootstrap artifacts to validate.
pub(crate) fn execute(args: StatusArgs) -> Result<()> {
    let state_root = absolute_path(&args.state.state_dir)?;
    let layout = Layout::new(state_root);
    let settings = Settings::load(&layout.settings)?;
    let node_manifest = NodeManifest::load(&layout.node.manifest, &settings.node.name)?;
    let liteserver_public_key = node_manifest
        .liteserver_public_key()
        .context("node does not expose a liteserver")?;

    ValidatorDatabase::open(layout.node.db.clone())?;
    if settings.node.role == NodeRole::Genesis {
        Manifest::load(&layout.manifest)?;
        DhtDatabase::open(layout.dht_db.clone())?;
    }

    let global_config = GlobalConfigFile::open(layout.node.global_config.clone())?;
    print_connection_details(&settings, &liteserver_public_key, &global_config)
}

/// Prints the liteserver endpoint, public identity, and client configuration path.
pub(crate) fn print_connection_details(
    settings: &Settings,
    liteserver_public_key: &TonPublicKey,
    global_config: &GlobalConfigFile,
) -> Result<()> {
    let global = dunce::canonicalize(global_config.path()).with_context(|| {
        format!(
            "global config is missing: {}",
            global_config.path().display()
        )
    })?;
    let node = &settings.node;
    println!(
        "Liteserver endpoint: {}:{}",
        node.public_ip, node.liteserver_port
    );
    println!(
        "Liteserver public key: {}",
        liteserver_public_key.to_base64()
    );
    println!("Global config: {}", global.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use expect_test::expect;

    use super::*;
    use crate::{
        cli::StateArgs,
        storage::NodeRole,
        ton::tools::types::{KeyId, TonPublicKey},
    };

    #[test]
    fn joined_status_uses_the_common_node_tree_without_bootstrap_state() {
        let directory = tempfile::tempdir_in("/tmp").unwrap();
        let state_dir = directory.path().join("joined");
        let layout = Layout::new(state_dir.clone());
        layout.create_dirs().unwrap();

        let mut settings = Settings::default();
        settings.node.name = "node2".to_owned();
        settings.node.role = NodeRole::Joined;
        settings.save_atomic(&layout.settings).unwrap();

        fs::write(
            &layout.global_config,
            include_bytes!(
                "../../../../crates/ton-indexer-liteserver/fixtures/mainnet-global.config.json"
            ),
        )
        .unwrap();
        fs::copy(&layout.global_config, &layout.node.global_config).unwrap();

        let full_node = KeyId::from_bytes([3; 32]);
        fs::write(
            layout.node.db.join("config.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "@type": "engine.validator.config",
                "out_port": 3272,
                "addrs": [],
                "adnl": [],
                "dht": [],
                "validators": [],
                "collators": [],
                "fullnode": full_node,
                "fullnodeslaves": [],
                "fullnodemasters": [],
                "liteservers": [],
                "control": [],
                "shards_to_monitor": [],
                "gc": { "@type": "engine.gc", "ids": [] },
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            layout.node.keyring.join(full_node.to_keyring_filename()),
            b"key",
        )
        .unwrap();
        NodeManifest::new(
            "node2",
            TonPublicKey::from_bytes([1; 32]),
            Some(TonPublicKey::from_bytes([2; 32])),
            full_node,
            None,
            None,
        )
        .save_atomic(&layout.node.manifest)
        .unwrap();

        execute(StatusArgs {
            state: StateArgs { state_dir },
            json: false,
        })
        .unwrap();

        let actual = serde_json::json!({
            "bootstrap_manifest_exists": layout.manifest.exists(),
            "dht_exists": layout.dht_db.exists(),
            "genesis_exists": layout.genesis.exists(),
        });
        expect![[r#"
            {
              "bootstrap_manifest_exists": false,
              "dht_exists": false,
              "genesis_exists": false
            }"#]]
        .assert_eq(&serde_json::to_string_pretty(&actual).unwrap());
    }
}
