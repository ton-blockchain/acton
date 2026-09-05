//! Shared lifecycle for validator-engine nodes.
//!
//! Bootstrap decides how a new TON network is created, while join decides which
//! remote network the node in this state directory follows. This module owns the part both flows
//! share: service keys, engine database initialization, durable node identity,
//! readiness, and transfer of a running process into the common registry.

mod database_dump;
mod validator;

pub(crate) use validator::configure_genesis_identity;

use std::{fs, io::ErrorKind, path::Path, time::Duration};

use anyhow::{Context, Result, ensure};
use tracing::{info, warn};

use crate::{
    runtime::ProcessRegistry,
    storage::{
        Layout, NodeLayout, NodeManifest, NodeRole, NodeRuntime, NodeSettings, RuntimeState,
    },
    ton::{
        toolchain::Toolchain,
        tools::{
            random_id::GenerateKeyRequest,
            types::{GeneratedKey, OperationContext},
            validator_engine::{ValidatorDatabase, ValidatorInitializeRequest},
        },
    },
};

/// Independent keys for the node's private control channel and optional liteserver.
///
/// Private material is installed by typed tool adapters. The workflow keeps the
/// generated values only long enough to configure validator-engine and publish
/// public identities in [`NodeManifest`].
pub(crate) struct NodeServiceKeys {
    pub(crate) server: GeneratedKey,
    pub(crate) client: GeneratedKey,
    pub(crate) liteserver: Option<GeneratedKey>,
}

/// Process-level validator-engine options for one node start.
///
/// These options are deliberately separate from persistent node settings: they
/// tune one process invocation without changing node identity or network policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct NodeStartOptions {
    /// Chooses whether CellDb stays on disk or is eagerly loaded into memory.
    pub(crate) celldb_mode: CellDbMode,
}

/// Process-local CellDb loading policy for validator-engine.
///
/// Small development databases benefit from eager preload, while an imported
/// database can be hundreds of gigabytes and must remain disk-backed by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CellDbMode {
    OnDisk,
    #[default]
    PreloadAll,
    InMemory,
}

/// Generates the service identities required by one configured node.
///
/// Control server and client keys are always distinct. A liteserver key exists
/// only when the node exposes that protocol, so disabled services leave no unused
/// private identity in the engine keyring.
pub(crate) async fn generate_service_keys(
    node_layout: &NodeLayout,
    tools: &Toolchain,
    node: &NodeSettings,
    context: &OperationContext,
) -> Result<NodeServiceKeys> {
    let server = tools
        .random_id
        .generate_key(
            context,
            GenerateKeyRequest::control_server(&node_layout.certs, &node_layout.keyring),
        )
        .await?;

    let client = tools
        .random_id
        .generate_key(
            context,
            GenerateKeyRequest::control_client(&node_layout.certs),
        )
        .await?;

    let liteserver = if node.liteserver {
        Some(
            tools
                .random_id
                .generate_key(
                    context,
                    GenerateKeyRequest::liteserver(&node_layout.keyring),
                )
                .await?,
        )
    } else {
        None
    };

    Ok(NodeServiceKeys {
        server,
        client,
        liteserver,
    })
}

/// Creates an engine database and installs its host-local service interfaces.
///
/// Callers must supply fully generated keys and a committed network global
/// config. The one-shot engine initializer is not idempotent; recovery belongs
/// to the caller's manifest boundary, which must discard partial node state.
pub(crate) async fn initialize_database(
    layout: &Layout,
    node_layout: &NodeLayout,
    tools: &Toolchain,
    node: &NodeSettings,
    keys: &NodeServiceKeys,
    context: &OperationContext,
) -> Result<ValidatorDatabase> {
    node_layout.create_dirs()?;

    if node_layout.global_config != layout.global_config {
        fs::copy(&layout.global_config, &node_layout.global_config).with_context(|| {
            format!(
                "failed to install node global config {}",
                node_layout.global_config.display()
            )
        })?;
    }
    fs::copy(
        &layout.global_config,
        node_layout.db.join("global.config.json"),
    )
    .with_context(|| format!("failed to install global config for node `{}`", node.name))?;

    let database = tools
        .validator_engine
        .initialize(
            context,
            ValidatorInitializeRequest::for_node(node_layout, node),
        )
        .await?;
    database.install_control_and_liteserver(
        node,
        keys.server.id,
        keys.client.id,
        keys.liteserver.as_ref().map(|key| key.id),
    )?;

    Ok(database)
}

/// Initializes or reopens a joined node without starting its persistent process.
///
/// A valid manifest makes the operation idempotent. If no manifest exists, all
/// node-owned partial state is removed before a fresh attempt. The manifest is
/// saved last, so an interruption can never make an incomplete database reusable.
pub(crate) async fn initialize_joined_node(
    layout: &Layout,
    node_layout: &NodeLayout,
    tools: &Toolchain,
    node: &NodeSettings,
    timeout: Duration,
    database_dump: Option<&Path>,
) -> Result<NodeManifest> {
    ensure!(
        node.role == NodeRole::Joined,
        "join initialization requires a node with the joined role"
    );
    if node_layout.manifest.is_file() {
        return NodeManifest::load(&node_layout.manifest, &node.name);
    }

    let started = std::time::Instant::now();
    info!(
        operation = "initialize_node",
        node = node.name,
        outcome = "pending",
        "initializing validator-engine state for joined node"
    );

    let result = async {
        clean_partial_node(&node_layout.root)?;
        node_layout.create_dirs()?;

        let context = OperationContext::for_node(timeout, &node.name);
        let keys = generate_service_keys(node_layout, tools, node, &context).await?;
        initialize_database(layout, node_layout, tools, node, &keys, &context).await?;

        let full_node_adnl = validator::configure_full_node_identity(
            node_layout,
            tools.validator_engine.as_ref(),
            tools.validator_console_tool.as_ref(),
            node,
            &context,
        )
        .await?;

        if let Some(archive) = database_dump {
            database_dump::import(archive, &node_layout.db, &node.name).await?;
        }

        let manifest = NodeManifest::new(
            &node.name,
            keys.server.public_key,
            keys.liteserver.as_ref().map(|key| key.public_key),
            full_node_adnl,
            None,
            None,
        );
        manifest.save_atomic(&node_layout.manifest)?;

        Ok(manifest)
    }
    .await;

    match &result {
        Ok(_) => info!(
            operation = "initialize_node",
            node = node.name,
            duration_ms = started.elapsed().as_millis(),
            outcome = "success",
            "joined node initialization completed"
        ),
        Err(error) => warn!(
            operation = "initialize_node",
            node = node.name,
            duration_ms = started.elapsed().as_millis(),
            outcome = "failure",
            %error,
            "joined node initialization failed"
        ),
    }

    result
}

/// Starts one completely initialized node and transfers lifecycle ownership to the registry.
///
/// Success means the authenticated validator console answered and the registry
/// owns the process. Persistent identity remains in [`NodeManifest`]; callers own
/// publication of invocation-specific fields in [`crate::storage::RuntimeState`].
pub(crate) async fn start(
    layout: &Layout,
    node_layout: &NodeLayout,
    tools: &Toolchain,
    node: &NodeSettings,
    timeout: Duration,
    options: NodeStartOptions,
    processes: &ProcessRegistry,
) -> Result<NodeRuntime> {
    ensure!(node.enabled, "node `{}` is disabled", node.name);
    ensure!(
        !processes.contains(&node.name).await,
        "node `{}` is already running",
        node.name
    );

    let started = std::time::Instant::now();
    let manifest = NodeManifest::load(&node_layout.manifest, &node.name)?;
    let database = ValidatorDatabase::open(&node_layout.db)?;
    let context = OperationContext::for_node(timeout, &node.name);

    info!(
        operation = "start_node",
        node = node.name,
        celldb_mode = ?options.celldb_mode,
        outcome = "pending",
        "starting validator-engine node"
    );
    let mut process = validator::start_persistent(
        node_layout,
        tools.validator_engine.as_ref(),
        node,
        database,
        options.celldb_mode,
    )
    .await?;

    if let Err(error) = validator::wait_for_console(
        node_layout,
        tools.validator_console_tool.as_ref(),
        node,
        &mut process,
        &context,
    )
    .await
    {
        let stop_error = process.stop().await.err();
        warn!(
            operation = "start_node",
            node = node.name,
            duration_ms = started.elapsed().as_millis(),
            outcome = "failure",
            %error,
            ?stop_error,
            "validator-engine node failed readiness"
        );

        if let Some(stop_error) = stop_error {
            return Err(error).context(format!(
                "node `{}` console did not become ready; stopping it also failed: {stop_error:#}",
                node.name
            ));
        }
        return Err(error).context(format!("node `{}` console did not become ready", node.name));
    }

    let previous = RuntimeState::load(&layout.runtime)?.node;
    let mut runtime = manifest.runtime(previous);
    runtime.running = true;
    runtime.pid = process.pid();
    runtime.status = "running".to_owned();

    processes.insert(process).await?;
    info!(
        operation = "start_node",
        node = node.name,
        celldb_mode = ?options.celldb_mode,
        duration_ms = started.elapsed().as_millis(),
        outcome = "success",
        "validator-engine node started"
    );

    Ok(runtime)
}

/// Removes incomplete node state before a manifest-backed retry.
fn clean_partial_node(path: &std::path::Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to clean partial node state {}", path.display())),
    }
}
