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
    runtime::{self, ProcessRegistry},
    storage::Settings,
    storage::{Layout, Manifest, endpoint},
    storage::{NodeRuntime, RuntimeState, ServiceRuntime},
    ton::accounts::{ImportedAccount, parse_imported_accounts},
};

use super::{LauncherControl, files::absolute_path, genesis, nodes, persistence, readiness};

/// Everything fixed before child processes are started.
///
/// Keeping the state lock in this value guarantees exclusive ownership of the
/// network directory for the complete launcher lifetime.
struct PreparedLaunch {
    layout: Layout,
    binaries: TonBinaries,
    timeout: Duration,
    settings: Settings,
    manifest: Manifest,
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
    let launch = prepare(args).await?;

    // Step 2: publish that the launcher owns this state directory.
    // `ready` is still false: at this point no claim is made that TON can answer
    // requests or produce blocks.
    let mut runtime = RuntimeState::load(&launch.layout.runtime)?;
    runtime.mark_launcher_started();
    runtime.save_atomic(&launch.layout.runtime)?;

    // Step 3: start the minimum process set: local DHT and genesis validator.
    // DHT makes the network identity discoverable over ADNL; the validator then
    // uses the prepared zerostate and global config to begin the masterchain.
    let processes =
        match nodes::start_core(&launch.layout, &launch.binaries, &launch.settings).await {
            Ok(processes) => processes,
            Err(error) => {
                mark_launcher_stopped(&launch.layout)?;
                return Err(error);
            }
        };

    // Steps 4–10 run under one cleanup boundary. From here on, every exit stops
    // all registered children and records that the network is no longer live.
    let result = run_managed_network(&launch, &processes, &mut runtime).await;
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
    let imported_accounts = parse_imported_accounts(&args.add_account)?;
    let state_root = absolute_path(&args.state_dir)?;
    let layout = Layout::new(state_root);
    layout.create_dirs()?;
    let state_lock = persistence::acquire_lock(&layout.lock)?;
    let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
    let timeout = Duration::from_secs(args.startup_timeout);
    let settings = prepare_settings(&layout, &args, &imported_accounts)?;

    // This is the one-time bootstrap boundary for a new state directory.
    // Without a manifest, `prepare_persistent_network` enters
    // `genesis::initialize`, which creates zerostates, validator and liteserver
    // keys, DHT/validator databases, global config, and finally the manifest.
    // Every later startup step assumes those artifacts already form one complete
    // network, so process creation must never bypass this call.
    let manifest =
        prepare_persistent_network(&layout, &binaries, &settings, &imported_accounts, timeout)
            .await?;

    Ok(PreparedLaunch {
        layout,
        binaries,
        timeout,
        settings,
        manifest,
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
    settings.services.ton_http_api.enabled |= args.ton_http_api;
    if args.no_config_http {
        settings.services.config_http.enabled = false;
    }
    if args.no_admin_http {
        settings.services.admin_http.enabled = false;
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
    if let Some(command) = args.ton_http_api_command.clone() {
        settings.services.ton_http_api.command = Some(command);
    }
    if let Some(static_config) = args.ton_http_api_static_config.clone() {
        settings.services.ton_http_api.static_config = Some(static_config);
    }
    Ok(settings)
}

/// Reuses an existing network or creates a new immutable zerostate.
///
/// The manifest acts as bootstrap's commit marker. If it exists, the launcher
/// verifies the referenced artifacts and rejects different imported accounts.
/// Without a manifest, genesis creation produces every required artifact and
/// writes the manifest only as its final step.
async fn prepare_persistent_network(
    layout: &Layout,
    binaries: &TonBinaries,
    settings: &Settings,
    imported_accounts: &[ImportedAccount],
    timeout: Duration,
) -> Result<Manifest> {
    if layout.manifest.is_file() {
        let manifest = Manifest::load(&layout.manifest)?;
        persistence::validate_persisted_state(layout, &manifest)?;
        persistence::validate_requested_imported_accounts(&manifest, imported_accounts)?;
        info!("reusing persistent local TON state");
        Ok(manifest)
    } else {
        genesis::initialize(layout, binaries, settings, imported_accounts, timeout).await
    }
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
    record_core_processes(runtime, processes).await;
    runtime.save_atomic(&launch.layout.runtime)?;

    // Step 4: prove actual block production through the liteserver.
    // A listening process is insufficient: observing two increasing seqnos
    // proves that validator consensus advances the masterchain over time.
    readiness::wait_for_blocks(
        &launch.layout,
        &launch.binaries,
        &launch.manifest,
        processes,
        launch.timeout,
    )
    .await?;

    // Step 5: initialize and start enabled non-genesis nodes.
    // They copy the already working genesis static state but receive independent
    // databases, ADNL identities, control keys, and optional liteservers.
    nodes::start_additional(
        &launch.layout,
        &launch.binaries,
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
        &launch.binaries,
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
        launch.binaries.clone(),
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
    if let Err(error) = print_connection_details(&launch.manifest) {
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
    runtime.masterchain_seqno = readiness::lite_client_seqno(&launch.binaries, &launch.manifest)
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
async fn record_core_processes(runtime: &mut RuntimeState, processes: &ProcessRegistry) {
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
            runtime.nodes.insert(
                process.name,
                NodeRuntime {
                    initialized: true,
                    running: true,
                    pid: process.pid,
                    status: "running".to_owned(),
                    ..NodeRuntime::default()
                },
            );
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
    persistence::validate_persisted_state(&layout, &manifest)?;
    print_connection_details(&manifest)
}

fn print_connection_details(manifest: &Manifest) -> Result<()> {
    let global = dunce::canonicalize(&manifest.global_config).with_context(|| {
        format!(
            "global config is missing: {}",
            manifest.global_config.display()
        )
    })?;
    println!("Liteserver endpoint: {}", endpoint());
    println!("Liteserver public key: {}", manifest.liteserver_public_key);
    println!("Global config: {}", global.display());
    Ok(())
}
