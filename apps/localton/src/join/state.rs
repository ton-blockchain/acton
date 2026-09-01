use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use tracing::info;

use crate::{
    cli::JoinArgs,
    storage::{Layout, NodeRole, NodeSettings, Settings},
    ton::global_config::GlobalConfig,
};

use super::ports::{DEFAULT_JOIN_PORT_BASE, HostPortAllocation};

/// Prepares persistent join configuration without starting external processes.
///
/// A saved global config commits the state directory to one TON network. A
/// completed node without it is rejected because rebuilding against another network
/// could mix durable validator-engine state and keys.
pub(super) async fn prepare_join_state(layout: &Layout, args: &JoinArgs) -> Result<Settings> {
    ensure!(
        !layout.manifest.is_file(),
        "join requires its own state directory, not a bootstrap state directory"
    );

    let mut settings = if layout.settings.is_file() {
        let settings = Settings::load(&layout.settings)?;
        ensure!(
            settings.node.role == NodeRole::Joined,
            "join state cannot contain the bootstrap genesis node; use a new --state-dir"
        );
        if let Some(requested) = args.node.as_deref() {
            ensure!(
                requested == settings.node.name,
                "joined node name is persisted; restart with --node {} or omit --node",
                settings.node.name
            );
        }
        ensure!(
            settings.node.public_ip == args.advertise_ip,
            "node `{}` advertises {}; use the original --advertise-ip {}",
            settings.node.name,
            settings.node.public_ip,
            settings.node.public_ip
        );
        ensure!(
            !args.validator || settings.node.validator,
            "validator mode is fixed by the first join attempt; recreate the state directory or enable validation after initialization"
        );

        settings
    } else {
        let name = if let Some(requested) = args.node.as_deref() {
            ensure!(requested != "genesis", "join cannot own the genesis node");
            requested.to_owned()
        } else {
            let digest = Sha256::digest(layout.root.to_string_lossy().as_bytes());
            format!("node-{}", hex::encode(&digest[..6]))
        };
        let allocation =
            HostPortAllocation::find(args.port_base.unwrap_or(DEFAULT_JOIN_PORT_BASE))?;
        let mut node = NodeSettings::joined(name, args.advertise_ip, allocation.node);
        node.validator = args.validator;
        node.participate_in_elections = args.validator;

        info!(
            node = node.name,
            port_range_start = allocation.start,
            port_range_end = allocation.end,
            console_port = node.console_port,
            adnl_port = node.adnl_port,
            liteserver_port = node.liteserver_port,
            out_port = node.out_port,
            dht_port = node.dht_port,
            "allocated persistent join node ports"
        );

        Settings::for_join(node)
    };
    let node_name = settings.node.name.clone();

    // Once downloaded, the global config pins this state directory to one TON
    // network. A restart reuses it even if the source URL later changes.
    let global_config_exists = layout.global_config.is_file();
    let global_config = if global_config_exists {
        info!("reusing persisted TON global config");
        GlobalConfig::load(&layout.global_config)?
    } else {
        ensure!(
            !layout.node.manifest.is_file(),
            "joining node `{node_name}` is initialized without a global config"
        );
        fetch_global_config(&args.global_config_url).await?
    };

    global_config.validate_advertise_ip(args.advertise_ip)?;

    if !global_config_exists {
        global_config.save_atomic(&layout.global_config)?;
        info!(url = %args.global_config_url, "installed TON global config");
    }

    // The first persisted settings own initialization-time identity. Retries can
    // restart the node but cannot reinterpret an existing state directory.
    settings.node.enabled = true;

    // settings.json is written last so it never advertises a network configuration
    // that failed validation or could not be persisted.
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;

    Ok(settings)
}

/// Downloads a complete global config before validating its join-specific fields.
async fn fetch_global_config(source: &str) -> Result<GlobalConfig> {
    let bytes = reqwest::Client::new()
        .get(source)
        .send()
        .await
        .with_context(|| format!("failed to request global config {source}"))?
        .error_for_status()
        .with_context(|| format!("global config request was rejected by {source}"))?
        .bytes()
        .await
        .context("failed to download global config")?;

    let config = GlobalConfig::from_json_bytes(&bytes).context("global config is invalid")?;
    config.validate_for_node_join()?;

    Ok(config)
}
