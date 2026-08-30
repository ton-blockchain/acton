//! Startup and one-time initialization of local validator-engine nodes.
//!
//! The genesis node starts together with DHT because it is required for the
//! network to produce its first blocks. Followers are initialized through the
//! join workflow so local and remote nodes enter the network in exactly the same way.

use std::{fs, time::Duration};

use anyhow::{Context, Result};
use tracing::info;

use crate::{
    runtime::ProcessRegistry,
    storage::Layout,
    storage::{NodeRuntime, RuntimeState},
    storage::{NodeSettings, Settings},
    ton::{
        toolchain::Toolchain,
        tools::{
            dht_server::DhtStartRequest,
            random_id::{GenerateKeyRequest, read_public_key},
            types::{AdnlEndpoint, DhtDatabase, OperationContext},
            validator_engine::{ValidatorDatabase, ValidatorInitializeRequest},
            validator_engine_config::ValidatorEngineConfig,
        },
    },
};

use super::validator;

/// Starts the DHT and genesis validator as the core process set.
///
/// DHT is inserted first because it provides peer discovery for the network.
/// The genesis validator is the only node required to begin masterchain block
/// production. Both processes enter one registry so failure of either is treated
/// as failure of the local network and both are stopped together.
pub(super) async fn start_core(
    layout: &Layout,
    tools: &Toolchain,
    settings: &Settings,
    dht_database: DhtDatabase,
    validator_database: ValidatorDatabase,
    processes: &ProcessRegistry,
) -> Result<()> {
    info!("starting local DHT and validator-engine");
    let genesis = settings
        .node("genesis")
        .context("settings contain no genesis node")?;

    let context = OperationContext::for_node(Duration::from_secs(30), &genesis.name);

    let dht = tools
        .dht_server
        .start(
            &context,
            DhtStartRequest {
                global_config: layout.global_config.clone(),
                database: dht_database,
                log_path: layout.logs.join("dht-engine"),
                stdout_log: layout.logs.join("dht.stdout.log"),
                stderr_log: layout.logs.join("dht.stderr.log"),
                endpoint: AdnlEndpoint::new(genesis.public_ip, genesis.dht_port),
                threads: usize::from(genesis.threads),
                verbosity: genesis.verbosity,
            },
        )
        .await?;

    processes.insert(dht).await?;

    let validator = validator::start_persistent(
        layout,
        tools.validator_engine.as_ref(),
        genesis,
        validator_database,
    )
    .await?;
    processes.insert(validator).await
}

/// Returns reusable metadata for a node, creating its persistent state if needed.
///
/// An existing engine config is the node-level initialization marker. Otherwise
/// the function installs the global config, asks validator-engine to create a
/// fresh database, generates independent control/liteserver keys, and registers
/// a full-node ADNL identity through a temporary engine process. Every new node,
/// whether local or remote, obtains the network state from its peers over ADNL.
pub(super) async fn ensure_initialized(
    layout: &Layout,
    tools: &Toolchain,
    node: &NodeSettings,
    timeout: Duration,
) -> Result<NodeRuntime> {
    let node_layout = layout.node(node);
    // Reuse the database and identities across instance runs. Metadata can be
    // reconstructed from public key files and engine config if runtime.json was
    // removed or written by an older interrupted invocation.
    if node_layout.config_json().is_file() {
        return recover_initialized_node(layout, node);
    }

    info!(node = node.name, "initializing validator-engine node");
    node_layout.create_dirs()?;

    fs::copy(&layout.global_config, &node_layout.global_config).with_context(|| {
        format!(
            "failed to copy global config to {}",
            node_layout.global_config.display()
        )
    })?;
    fs::copy(
        &layout.global_config,
        node_layout.db.join("global.config.json"),
    )?;

    let context = OperationContext::for_node(timeout, &node.name);
    let validator_database = tools
        .validator_engine
        .initialize(&context, ValidatorInitializeRequest::for_node(layout, node))
        .await?;

    // Control server/client and liteserver use separate keys. Reusing one key for
    // all roles would couple administrative access to a public network identity.
    let server = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::control_server(&node_layout.certs, &node_layout.keyring),
        )
        .await?;
    let client = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::control_client(&node_layout.certs),
        )
        .await?;
    let liteserver = tools
        .random_id
        .generate_key(
            &context,
            GenerateKeyRequest::liteserver(&node_layout.keyring),
        )
        .await?;
    validator_database.install_control_and_liteserver(node, server.id, client.id, liteserver.id)?;

    let full_node_adnl = validator::configure_full_node_identity(
        layout,
        tools.validator_engine.as_ref(),
        tools.validator_console_tool.as_ref(),
        node,
        &context,
    )
    .await?;
    Ok(NodeRuntime {
        initialized: true,
        status: "initialized".to_owned(),
        console_public_key: Some(server.public_key.to_base64()),
        liteserver_public_key: node
            .liteserver
            .then(|| read_public_key(&liteserver.public_path).map(|key| key.to_base64()))
            .transpose()?,
        validator_adnl: Some(full_node_adnl.to_hex()),
        ..NodeRuntime::default()
    })
}

/// Reconstructs runtime metadata without changing an initialized engine database.
///
/// `runtime.json` is operational and may be missing or stale, whereas engine
/// config and public key files are persistent sources of truth for node identity.
fn recover_initialized_node(layout: &Layout, node: &NodeSettings) -> Result<NodeRuntime> {
    let node_layout = layout.node(node);
    let mut runtime = RuntimeState::load(&layout.runtime)?
        .nodes
        .get(&node.name)
        .cloned()
        .unwrap_or_default();
    runtime.initialized = true;
    runtime.running = false;
    runtime.pid = None;
    runtime.status = "initialized".to_owned();
    runtime.console_public_key =
        Some(read_public_key(&node_layout.server_public_key())?.to_base64());
    if node.liteserver {
        runtime.liteserver_public_key =
            Some(read_public_key(&node_layout.keyring.join("liteserver.pub"))?.to_base64());
    }
    recover_config_metadata(&node_layout.config_json(), &mut runtime)?;
    Ok(runtime)
}

/// Recovers engine-owned ADNL metadata that has no standalone public-key file.
fn recover_config_metadata(path: &std::path::Path, runtime: &mut NodeRuntime) -> Result<()> {
    let config = ValidatorEngineConfig::load(path)?;
    if runtime.validator_adnl.is_none() {
        runtime.validator_adnl = Some(config.fullnode_adnl().to_hex());
    }
    Ok(())
}
