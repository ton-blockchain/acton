//! The readable, top-level startup pipeline for a local TON network.
//!
//! [`run`] is intentionally expressed as ordered lifecycle stages: prepare the
//! request, create or validate persistent state, start the core processes, prove
//! masterchain progress, add optional nodes and APIs, publish readiness, then
//! supervise everything until shutdown. Technical details live in sibling
//! modules and do not obscure this sequence.

use std::{
    fs::File,
    net::Ipv4Addr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use tracing::info;

use crate::{
    binaries::TonBinaries,
    cli::{RunArgs, StatusArgs},
    http,
    runtime::{self, ProcessRegistry, run_stage},
    storage::Settings,
    storage::{Layout, Manifest},
    storage::{NodeRuntime, RuntimeState, ServiceRuntime},
    ton::{
        accounts::{ImportedAccount, parse_imported_accounts},
        global_config::GlobalConfigFile,
        toolchain::Toolchain,
        tools::{lite_client::LiteTarget, types::DhtDatabase, validator_engine::ValidatorDatabase},
    },
};

use super::{LauncherControl, acquire_lock, files::absolute_path, genesis, nodes, readiness};

/// Everything fixed before child processes are started.
///
/// Keeping the state lock in this value guarantees exclusive ownership of the
/// network directory for the complete launcher lifetime.
struct PreparedLaunch {
    layout: Layout,
    tools: Toolchain,
    timeout: Duration,
    settings: Settings,
    manifest: Manifest,
    global_config: GlobalConfigFile,
    dht_database: DhtDatabase,
    validator_database: ValidatorDatabase,
    ton_http_api_bind: Ipv4Addr,
    _state_lock: File,
}

/// Owns the complete lifecycle of one launcher invocation.
///
/// Preparation finishes before persistent child processes are created. Once
/// DHT and the genesis validator have started, every remaining exit path—normal
/// shutdown, signal, startup error, or child failure—converges on stopping the
/// process registry and clearing the live state in `runtime.json`.
pub async fn run(args: RunArgs) -> Result<()> {
    // Step 1: turn CLI input and on-disk files into a complete launch plan.
    // This also creates genesis on the first run. No long-lived TON process is
    // started until the state directory, settings, binaries, and manifest agree.
    let state_target = args.state_dir.display().to_string();
    let launch = run_stage("launcher", "prepare", &state_target, prepare(args)).await?;

    // Step 2: publish that the launcher owns this state directory.
    // `ready` is still false: at this point no claim is made that TON can answer
    // requests or produce blocks.
    let mut runtime = RuntimeState::load(&launch.layout.runtime)?;
    runtime.mark_launcher_started();
    runtime.save_atomic(&launch.layout.runtime)?;

    // Step 3: start the minimum process set: local DHT and genesis validator.
    // DHT makes the network identity discoverable over ADNL; the validator then
    // uses the prepared zerostate and global config to begin the masterchain.
    let processes = match run_stage(
        "launcher",
        "start_core",
        &state_target,
        nodes::start_core(
            &launch.layout,
            &launch.tools,
            &launch.settings,
            launch.dht_database.clone(),
            launch.validator_database.clone(),
        ),
    )
    .await
    {
        Ok(processes) => processes,
        Err(error) => {
            mark_launcher_stopped(&launch.layout)?;
            return Err(error);
        }
    };

    // Steps 4–10 run under one cleanup boundary. From here on, every exit stops
    // all registered children and records that the network is no longer live.
    let result = run_stage(
        "launcher",
        "managed_network",
        &state_target,
        run_managed_network(&launch, &processes, &mut runtime),
    )
    .await;
    let stop_result = processes.stop_all().await;
    let state_result = mark_launcher_stopped(&launch.layout);

    result.and(stop_result).and(state_result)
}

/// Resolves everything required by the long-running part of the launcher.
///
/// This stage takes the exclusive state lock, resolves a compatible TON
/// toolchain, applies settings, and either validates a persisted network or
/// creates its genesis. Returning successfully means the disk state is complete
/// enough to start DHT and validator-engine.
async fn prepare(args: RunArgs) -> Result<PreparedLaunch> {
    let state_root = absolute_path(&args.state_dir)?;
    let layout = Layout::new(state_root);
    layout.create_dirs()?;

    let state_lock = acquire_lock(&layout.lock)?;
    let state_exists = layout.manifest.is_file();
    ensure!(
        !state_exists || args.add_account.is_empty(),
        "--add-account can only be used when creating a network; use another --state-dir"
    );
    let imported_accounts = if state_exists {
        Vec::new()
    } else {
        parse_imported_accounts(&args.add_account)?
    };
    let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
    let tools = Toolchain::official(layout.clone(), binaries);
    let timeout = Duration::from_secs(args.startup_timeout);
    let settings = prepare_settings(&layout, &args, &imported_accounts)?;

    // The manifest is bootstrap's commit marker. Existing state is validated
    // before reuse; without it, genesis initialization creates every artifact
    // and writes the manifest only after the network is complete.
    let manifest = if state_exists {
        let manifest = Manifest::load(&layout.manifest)?;
        info!("reusing persistent local TON state");
        manifest
    } else {
        genesis::initialize(&layout, &tools, &settings, &imported_accounts, timeout).await?
    };
    let global_config = GlobalConfigFile::open(layout.global_config.clone())?;
    let dht_database = DhtDatabase::open(layout.dht_db.clone())?;
    let validator_database = ValidatorDatabase::open(layout.validator_db.clone())?;

    Ok(PreparedLaunch {
        layout,
        tools,
        timeout,
        settings,
        manifest,
        global_config,
        dht_database,
        validator_database,
        ton_http_api_bind: args.ton_http_api_bind,
        _state_lock: state_lock,
    })
}

/// Builds the effective settings for this invocation.
///
/// Topology changes such as validator count and enabled services are persisted.
/// Bind and executable overrides are applied after that save because they are
/// operational choices for this run, not properties of the blockchain itself.
fn prepare_settings(
    layout: &Layout,
    args: &RunArgs,
    imported_accounts: &[ImportedAccount],
) -> Result<Settings> {
    let mut settings = Settings::load_or_create(&layout.settings)?;
    if let Some(validators) = args.validators {
        settings.enable_validator_count(validators)?;
    }
    if let Some(advertise_ip) = args.advertise_ip {
        let genesis = settings.node_mut("genesis")?;
        if layout.manifest.is_file() {
            ensure!(
                genesis.public_ip == advertise_ip,
                "genesis advertises {}; --advertise-ip cannot change after network creation",
                genesis.public_ip
            );
        } else {
            genesis.public_ip = advertise_ip;
        }
    }
    settings.services.ton_http_api.enabled |= args.ton_http_api;
    if args.no_config_http {
        settings.services.config_http.enabled = false;
    }
    if args.no_admin_http {
        settings.services.admin_http.enabled = false;
    }
    if args.no_observability {
        settings.services.observability.enabled = false;
    }
    settings.validate()?;
    ensure!(
        imported_accounts.is_empty() || settings.network.workchain_enabled,
        "--add-account requires the basechain workchain to be enabled"
    );
    settings.save_atomic(&layout.settings)?;

    // These CLI values affect only this launcher run and are not persisted.
    if let Some(bind) = args.config_http_bind {
        settings.services.config_http.bind = bind;
        settings.validate()?;
    }
    if let Some(bind) = args.admin_http_bind {
        settings.services.admin_http.bind = bind;
        settings.validate()?;
    }
    if let Some(bind) = args.observability_bind {
        settings.services.observability.bind = bind;
        settings.validate()?;
    }
    if let Some(command) = args.ton_http_api_command.clone() {
        settings.services.ton_http_api.command = Some(command);
    }
    if let Some(static_config) = args.ton_http_api_static_config.clone() {
        settings.services.ton_http_api.static_config = Some(static_config);
    }
    Ok(settings)
}

/// Advances the core process set into a ready, supervised local network.
///
/// The order is deliberate: block production is proven before optional nodes
/// clone the genesis state, and client-facing APIs start only after the chain is
/// usable. Final process cleanup remains the responsibility of [`run`].
async fn run_managed_network(
    launch: &PreparedLaunch,
    processes: &ProcessRegistry,
    runtime: &mut RuntimeState,
) -> Result<()> {
    record_core_processes(runtime, processes, &launch.manifest).await;
    runtime.save_atomic(&launch.layout.runtime)?;

    // Step 4: prove actual block production through the liteserver.
    // A listening process is insufficient: observing two increasing seqnos
    // proves that validator consensus advances the masterchain over time.
    readiness::wait_for_blocks(
        &launch.layout,
        launch.tools.lite_client_tool.as_ref(),
        &LiteTarget::new(launch.global_config.path()).with_label("genesis"),
        processes,
        launch.timeout,
    )
    .await?;

    // Step 5: initialize and start enabled non-genesis nodes.
    // They copy the already working genesis static state but receive independent
    // databases, ADNL identities, control keys, and optional liteservers.
    nodes::start_additional(
        &launch.layout,
        &launch.tools,
        &launch.settings,
        launch.timeout,
        processes,
        runtime,
    )
    .await?;

    // Step 6: start the optional TON HTTP API V2 indexer/bridge.
    // It depends on a working liteserver, so starting it before Step 4 would turn
    // normal chain bootstrap into an API readiness failure.
    http::v2::start(
        &launch.layout,
        &launch.tools.binaries,
        &launch.settings,
        launch.timeout,
        processes,
        runtime,
    )
    .await?;

    // Step 7: start launcher-owned config, admin, and public proxy listeners.
    // Unlike validator-engine and V2, these are Tokio tasks inside this process;
    // their handles are kept in `ServiceSet` for coordinated shutdown.
    let control = LauncherControl::new(
        launch.layout.clone(),
        launch.tools.clone(),
        launch.timeout,
        processes.clone(),
    );
    let services = http::start(control, &launch.settings, launch.ton_http_api_bind).await?;

    // Step 8: atomically publish endpoints and observed chain state as ready.
    // External status readers see readiness only after both blockchain and HTTP
    // dependencies have passed their startup checks.
    if let Err(error) = mark_network_ready(launch, runtime, &services).await {
        services.shutdown().await;
        return Err(error);
    }

    // Step 9: print connection data, start periodic maintenance, and supervise.
    // The launcher now blocks until Ctrl-C/SIGTERM or until any required child
    // exits; a child failure is treated as failure of the whole local network.
    if let Err(error) =
        print_connection_details(&launch.manifest, &launch.settings, &launch.global_config)
    {
        services.shutdown().await;
        return Err(error);
    }

    let background = runtime::background::start(launch.layout.clone(), &launch.settings);
    info!("local TON is producing masterchain blocks; press Ctrl-C to stop");

    let supervision = tokio::select! {
        result = readiness::supervise(processes) => result,
        signal_result = readiness::shutdown_signal() => signal_result,
    };

    // Step 10: stop in-process tasks first. The outer cleanup then terminates
    // DHT, validator-engine, and optional V2 processes as complete process groups.
    background.shutdown().await;
    services.shutdown().await;
    supervision
}

/// Publishes the externally useful runtime state after startup succeeds.
///
/// `ready` means more than "processes exist": the masterchain has advanced and
/// every requested listener has started. Endpoints and the latest observable
/// seqno are saved together so readers do not observe a partially ready network.
async fn mark_network_ready(
    launch: &PreparedLaunch,
    runtime: &mut RuntimeState,
    services: &http::ServiceSet,
) -> Result<()> {
    for (name, endpoint) in services.endpoints() {
        runtime.services.insert(
            name.clone(),
            ServiceRuntime {
                running: true,
                pid: Some(std::process::id()),
                endpoint: Some(endpoint.clone()),
                last_error: None,
            },
        );
    }
    runtime.ready = true;
    runtime.masterchain_seqno = readiness::lite_client_seqno(
        launch.tools.lite_client_tool.as_ref(),
        &LiteTarget::new(launch.global_config.path()).with_label("genesis"),
    )
    .await
    .ok();
    runtime.last_block_at = Some(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    runtime.save_atomic(&launch.layout.runtime)
}

/// Maps core child processes into the public runtime-state model.
///
/// DHT is infrastructure and is therefore represented as a service. Each
/// validator-engine process is a blockchain node. Status and admin APIs rely on
/// this distinction even though both kinds share the same process registry.
async fn record_core_processes(
    runtime: &mut RuntimeState,
    processes: &ProcessRegistry,
    manifest: &Manifest,
) {
    for process in processes.info().await {
        if process.name == "dht" {
            runtime.services.insert(
                "dht".to_owned(),
                ServiceRuntime {
                    running: true,
                    pid: process.pid,
                    endpoint: None,
                    last_error: None,
                },
            );
        } else {
            let mut node = NodeRuntime {
                initialized: true,
                running: true,
                pid: process.pid,
                status: "running".to_owned(),
                ..NodeRuntime::default()
            };
            if process.name == "genesis" {
                node.liteserver_public_key = Some(manifest.liteserver_public_key.to_base64());
                node.remember_validator_public_key(manifest.validator_public_key.to_base64());
            }
            runtime.nodes.insert(process.name, node);
        }
    }
}

/// Atomically clears launcher and child readiness after every exit path.
fn mark_launcher_stopped(layout: &Layout) -> Result<()> {
    RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_launcher_stopped();
        Ok(())
    })?;
    Ok(())
}

/// Prints connection data for a complete persisted network without starting it.
pub async fn status(args: StatusArgs) -> Result<()> {
    let state_root = absolute_path(&args.state.state_dir)?;
    let layout = Layout::new(state_root);
    let manifest = Manifest::load(&layout.manifest)?;
    let global_config = GlobalConfigFile::open(layout.global_config.clone())?;
    DhtDatabase::open(layout.dht_db.clone())?;
    ValidatorDatabase::open(layout.validator_db.clone())?;
    let settings = Settings::load_or_create(&layout.settings)?;
    print_connection_details(&manifest, &settings, &global_config)
}

fn print_connection_details(
    manifest: &Manifest,
    settings: &Settings,
    global_config: &GlobalConfigFile,
) -> Result<()> {
    let global = dunce::canonicalize(global_config.path()).with_context(|| {
        format!(
            "global config is missing: {}",
            global_config.path().display()
        )
    })?;
    let genesis = settings.node("genesis")?;
    println!(
        "Liteserver endpoint: {}:{}",
        genesis.public_ip, genesis.liteserver_port
    );
    println!(
        "Liteserver public key: {}",
        manifest.liteserver_public_key.to_base64()
    );
    println!("Global config: {}", global.display());
    Ok(())
}
