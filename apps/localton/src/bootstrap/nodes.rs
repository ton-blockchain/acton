//! Startup and one-time initialization of local validator-engine nodes.
//!
//! The genesis node starts together with DHT because it is required for the
//! network to produce its first blocks. Additional configured nodes are cloned
//! from the genesis static state, receive independent identities and databases,
//! and start only after the genesis liteserver proves that the chain advances.

use std::{fs, time::Duration};

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde_json::Value;
use tracing::{info, warn};

use crate::{
    binaries::TonBinaries,
    runtime::{ManagedProcess, ProcessRegistry, run_checked},
    storage::Layout,
    storage::{NodeRuntime, RuntimeState},
    storage::{NodeSettings, Settings},
};

use super::{
    dht,
    engine_config::patch_out_port,
    files::copy_tree,
    keys::{generate_key, read_key_id_base64},
    validator,
};

/// Starts the DHT and genesis validator as the core process set.
///
/// DHT is inserted first because it provides peer discovery for the network.
/// The genesis validator is the only node required to begin masterchain block
/// production. Both processes enter one registry so failure of either is treated
/// as failure of the local network and both are stopped together.
pub(super) async fn start_core(
    layout: &Layout,
    binaries: &TonBinaries,
    settings: &Settings,
) -> Result<ProcessRegistry> {
    info!("starting local DHT and validator-engine");
    let genesis = settings
        .node("genesis")
        .context("settings contain no genesis node")?;
    let registry = ProcessRegistry::default();
    let dht = ManagedProcess::spawn(
        "dht",
        dht::command(layout, binaries, genesis),
        &layout.logs.join("dht.stdout.log"),
        &layout.logs.join("dht.stderr.log"),
    )?;
    registry.insert(dht).await?;
    let validator = ManagedProcess::spawn(
        "genesis",
        validator::command(layout, binaries, genesis, true),
        &layout.logs.join("validator.stdout.log"),
        &layout.logs.join("validator.stderr.log"),
    )?;
    registry.insert(validator).await?;
    Ok(registry)
}

/// Initializes and starts every enabled node except the genesis validator.
///
/// This runs only after genesis block production has been observed. Consequently
/// each new node copies a known-good static state and global config, then receives
/// its own database and identities before joining the shared process registry.
pub(super) async fn start_additional(
    layout: &Layout,
    binaries: &TonBinaries,
    settings: &Settings,
    timeout: Duration,
    processes: &ProcessRegistry,
    runtime: &mut RuntimeState,
) -> Result<()> {
    for node in settings
        .nodes
        .iter()
        .filter(|node| node.enabled && node.name != "genesis")
    {
        let initialized = ensure_initialized(layout, binaries, node, timeout).await?;
        let node_layout = layout.node(node);
        let mut process = ManagedProcess::spawn(
            node.name.clone(),
            validator::command(layout, binaries, node, true),
            &node_layout.logs.join("validator.stdout.log"),
            &node_layout.logs.join("validator.stderr.log"),
        )?;
        if let Err(error) =
            validator::wait_for_console(layout, binaries, node, &mut process, timeout).await
        {
            process.stop().await?;
            return Err(error)
                .context(format!("node `{}` console did not become ready", node.name));
        }
        let mut node_runtime = initialized;
        node_runtime.running = true;
        node_runtime.pid = process.id();
        node_runtime.status = "running".to_owned();
        runtime.nodes.insert(node.name.clone(), node_runtime);
        processes.insert(process).await?;
    }
    runtime.save_atomic(&layout.runtime)?;
    Ok(())
}

/// Returns reusable metadata for a node, creating its persistent state if needed.
///
/// An existing engine config is the node-level initialization marker. Otherwise
/// the function copies genesis bootstrap data, asks validator-engine to create a
/// fresh database, generates independent control/liteserver keys, and registers
/// a full-node ADNL identity through a temporary engine process.
pub(super) async fn ensure_initialized(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    timeout: Duration,
) -> Result<NodeRuntime> {
    let node_layout = layout.node(node);
    // Reuse the database and identities across launcher runs. Metadata can be
    // reconstructed from public key files and engine config if runtime.json was
    // removed or written by an older interrupted invocation.
    if node_layout.config_json().is_file() {
        return recover_initialized_node(layout, node);
    }

    info!(node = node.name, "initializing validator-engine node");
    node_layout.create_dirs()?;
    // Static zerostates are shared by content, but each node gets its own copy
    // under an independent database so later engine writes never overlap.
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
    copy_tree(
        &layout.validator_db.join("static"),
        &node_layout.db.join("static"),
    )?;

    let output = run_checked(
        &format!("{} validator-engine initialization", node.name),
        validator::command(layout, binaries, node, false),
        timeout,
    )
    .await?;
    if !output.stderr.trim().is_empty() {
        warn!(
            node = node.name,
            stderr = output.stderr.trim(),
            "validator initialization wrote to stderr"
        );
    }
    patch_out_port(&node_layout.config_json(), node.out_port)?;

    // Control server/client and liteserver use separate keys. Reusing one key for
    // all roles would couple administrative access to a public network identity.
    let server = generate_key(binaries, &node_layout.server_private_key()).await?;
    fs::copy(
        &server.private_path,
        node_layout.keyring.join(&server.id_hex),
    )?;
    let client = generate_key(binaries, &node_layout.client_private_key()).await?;
    let liteserver = generate_key(binaries, &node_layout.keyring.join("liteserver")).await?;
    fs::copy(
        &liteserver.private_path,
        node_layout.keyring.join(&liteserver.id_hex),
    )?;
    validator::configure_local_services(layout, node, &server, &client, &liteserver)?;

    let full_node_adnl = configure_full_node_identity(layout, binaries, node, timeout).await?;
    Ok(NodeRuntime {
        initialized: true,
        status: "initialized".to_owned(),
        console_public_key: Some(server.id_base64),
        liteserver_public_key: node.liteserver.then_some(liteserver.id_base64),
        validator_adnl: Some(full_node_adnl),
        ..NodeRuntime::default()
    })
}

/// Creates and activates the node's long-lived full-node ADNL identity.
///
/// The identity is stored inside validator-engine's database, so a temporary
/// process must expose the console while it is created, exported, registered,
/// and selected as `fullnode`. The process is stopped on both success and error.
async fn configure_full_node_identity(
    layout: &Layout,
    binaries: &TonBinaries,
    node: &NodeSettings,
    timeout: Duration,
) -> Result<String> {
    let node_layout = layout.node(node);
    let mut temporary = ManagedProcess::spawn(
        format!("{} temporary validator-engine", node.name),
        validator::command(layout, binaries, node, false),
        &node_layout.logs.join("validator-bootstrap.stdout.log"),
        &node_layout.logs.join("validator-bootstrap.stderr.log"),
    )?;
    let configured = async {
        validator::wait_for_console(layout, binaries, node, &mut temporary, timeout).await?;
        let full_node_adnl = validator::console_new_key(layout, binaries, node).await?;
        validator::console(
            layout,
            binaries,
            node,
            &format!("exportpub {full_node_adnl}"),
        )
        .await?;
        validator::console_retry(
            layout,
            binaries,
            node,
            &format!("addadnl {full_node_adnl} 0"),
        )
        .await?;
        validator::console_retry(
            layout,
            binaries,
            node,
            &format!("changefullnodeaddr {full_node_adnl}"),
        )
        .await?;
        Ok::<String, anyhow::Error>(full_node_adnl)
    }
    .await;
    temporary.stop().await?;
    configured
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
    if runtime.console_public_key.is_none() {
        runtime.console_public_key = read_key_id_base64(&node_layout.server_public_key()).ok();
    }
    if node.liteserver && runtime.liteserver_public_key.is_none() {
        runtime.liteserver_public_key =
            read_key_id_base64(&node_layout.keyring.join("liteserver.pub")).ok();
    }
    recover_config_metadata(&node_layout.config_json(), &mut runtime)?;
    Ok(runtime)
}

/// Fills identity fields that could not be recovered from standalone key files.
///
/// Values already present in runtime state win. Missing console/liteserver IDs
/// come from their config arrays; full-node ADNL is decoded from the engine's
/// base64 representation into the hexadecimal form exposed by the admin API.
fn recover_config_metadata(path: &std::path::Path, runtime: &mut NodeRuntime) -> Result<()> {
    let config: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("invalid validator config {}", path.display()))?;
    if runtime.console_public_key.is_none() {
        runtime.console_public_key = config
            .get("control")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    if runtime.liteserver_public_key.is_none() {
        runtime.liteserver_public_key = config
            .get("liteservers")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|item| item.get("id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    if runtime.validator_adnl.is_none() {
        runtime.validator_adnl = config
            .get("fullnode")
            .and_then(Value::as_str)
            .and_then(|id| BASE64.decode(id).ok())
            .map(hex::encode);
    }
    Ok(())
}
