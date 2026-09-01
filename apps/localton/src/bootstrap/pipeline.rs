//! The readable, top-level startup pipeline for a local TON network.
//!
//! [`run`] is intentionally expressed as ordered lifecycle stages: prepare the
//! request, create or validate persistent state, start the core processes, prove
//! masterchain progress, add optional APIs, publish readiness, then
//! supervise everything until shutdown. Technical details live in sibling
//! modules and do not obscure this sequence.

use std::time::Duration;

use anyhow::{Context, Result, ensure};
use tracing::info;

use crate::{
    binaries::TonBinaries,
    cli::{BootstrapArgs, StatusArgs},
    http,
    runtime::{self, ProcessRegistry},
    storage::Settings,
    storage::{Layout, Manifest},
    storage::{NodeRuntime, RuntimeState, ServiceRuntime},
    ton::{
        accounts::parse_imported_accounts,
        global_config::GlobalConfigFile,
        toolchain::Toolchain,
        tools::{lite_client::LiteTarget, types::DhtDatabase, validator_engine::ValidatorDatabase},
    },
};

use super::{NodeController, acquire_lock, files::absolute_path, genesis, nodes, readiness};

/// Owns the complete lifecycle of one bootstrap invocation.
///
/// No persistent process starts until disk state is complete. Once DHT and the
/// genesis validator exist, the local async block captures every remaining exit
/// path so child shutdown and runtime-state cleanup always run afterward.
pub async fn run(args: BootstrapArgs) -> Result<()> {
    let state_root = absolute_path(&args.state_dir)?;
    let layout = Layout::new(state_root);
    layout.create_dirs()?;

    // Keep exclusive ownership until every child and in-process service stops.
    let _state_lock = acquire_lock(&layout.lock)?;

    let state_exists = layout.manifest.is_file();

    let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
    let tools = Toolchain::official(layout.clone(), binaries);
    let timeout = Duration::from_secs(args.startup_timeout);
    let settings = prepare_settings(&layout, &args)?;

    // The manifest is bootstrap's commit marker. Existing state is reused only
    // after a complete bootstrap; otherwise genesis recreates partial artifacts.
    let manifest = if state_exists {
        Manifest::load(&layout.manifest)?
    } else {
        let imported_accounts = parse_imported_accounts(&args.add_account)?;
        genesis::initialize(&layout, &tools, &settings, &imported_accounts, timeout).await?
    };

    let global_config = GlobalConfigFile::open(layout.global_config.clone())?;
    let dht_database = DhtDatabase::open(layout.dht_db.clone())?;
    let validator_database = ValidatorDatabase::open(layout.validator_db.clone())?;

    let masterchain_readiness = if state_exists {
        readiness::MasterchainReadiness::HeadAvailable
    } else {
        readiness::MasterchainReadiness::ProductionObserved
    };

    // Publish ownership before starting child processes. `ready` remains false
    // until the chain and every requested listener have passed readiness checks.
    let mut runtime = RuntimeState::load(&layout.runtime)?;
    runtime.mark_instance_started();
    runtime.save_atomic(&layout.runtime)?;

    let processes = ProcessRegistry::default();

    // Every process-start error, signal, or required-child exit converges on the
    // cleanup below instead of escaping through `?` with live processes.
    let result = async {
        nodes::start_core(
            &layout,
            &tools,
            &settings,
            dht_database,
            validator_database,
            &processes,
        )
        .await?;

        record_core_processes(&mut runtime, &processes, &manifest).await;
        runtime.save_atomic(&layout.runtime)?;

        let masterchain_seqno = readiness::wait_for_masterchain(
            &layout,
            tools.lite_client_tool.as_ref(),
            &LiteTarget::new(global_config.path()).with_label("genesis"),
            &processes,
            timeout,
            masterchain_readiness,
        )
        .await?;

        http::v2::start(
            &layout,
            &tools.binaries,
            &settings,
            timeout,
            &processes,
            &mut runtime,
        )
        .await?;

        let control = NodeController::new(layout.clone(), tools.clone(), processes.clone());
        let services = http::start(control, &settings, args.ton_http_api_bind).await?;

        if let Err(error) = mark_network_ready(&layout, &mut runtime, &services, masterchain_seqno)
        {
            services.shutdown().await;
            return Err(error);
        }

        if let Err(error) = print_connection_details(&manifest, &settings, &global_config) {
            services.shutdown().await;
            return Err(error);
        }

        let background = runtime::background::start(layout.clone(), &settings);
        info!("local TON services are ready; press Ctrl-C to stop");

        let supervision = tokio::select! {
            result = readiness::supervise(&processes) => result,
            signal_result = readiness::shutdown_signal() => signal_result,
        };

        background.shutdown().await;
        services.shutdown().await;
        supervision
    }
    .await;

    let stop_result = processes.stop_all().await;
    let state_result = mark_instance_stopped(&layout);

    result.and(stop_result).and(state_result)
}

/// Builds the effective settings for this invocation.
///
/// Network parameters and enabled services are persisted. Bind and executable
/// overrides are applied after that save because they are operational choices
/// for this run, not properties of the blockchain itself.
fn prepare_settings(layout: &Layout, args: &BootstrapArgs) -> Result<Settings> {
    let mut settings = Settings::load_or_create(&layout.settings)?;
    let genesis = settings
        .nodes
        .first()
        .context("bootstrap settings contain no genesis node")?;
    ensure!(
        genesis.name == "genesis" && genesis.enabled && genesis.validator,
        "bootstrap settings must start with an enabled genesis validator"
    );

    if let Some(block_time_ms) = args.block_time {
        if layout.manifest.is_file() {
            ensure!(
                settings.network.simplex_target_rate_ms == block_time_ms,
                "network block time is {}ms; --block-time cannot change after network creation",
                settings.network.simplex_target_rate_ms
            );
        } else {
            settings.network.simplex_target_rate_ms = block_time_ms;
        }
    }

    if let Some(election_time_seconds) = args.election_time {
        if layout.manifest.is_file() {
            ensure!(
                settings.network.elected_for_seconds == election_time_seconds,
                "network election time is {}s; --election-time cannot change after network creation",
                settings.network.elected_for_seconds
            );
        } else {
            settings
                .network
                .set_election_time_seconds(election_time_seconds)?;
        }
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
    settings.validate()?;
    settings.save_atomic(&layout.settings)?;

    // These CLI values affect only this bootstrap invocation and are not persisted.
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

/// Publishes the externally useful runtime state after startup succeeds.
///
/// `ready` means more than "processes exist": a trusted masterchain head and
/// every requested listener are available. Endpoints and that verified seqno are
/// saved together so readers do not observe a partially ready network.
fn mark_network_ready(
    layout: &Layout,
    runtime: &mut RuntimeState,
    services: &http::ServiceSet,
    masterchain_seqno: u32,
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
    runtime.observe_masterchain_head(masterchain_seqno, crate::storage::unix_time());
    runtime.save_atomic(&layout.runtime)
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

/// Atomically clears instance and child readiness after every exit path.
fn mark_instance_stopped(layout: &Layout) -> Result<()> {
    RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_instance_stopped();
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
