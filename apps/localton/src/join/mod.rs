//! Host-local joiner and supervisor for one node entering an existing network.
//!
//! On its first run, the join workflow downloads a standard TON global config. It
//! creates an independent database and keys, starts the node, and keeps private
//! material on this host. With `--validator`, it can fund a host-local
//! wallet from a development faucet and enters elections directly through the
//! TON Elector contract.

mod ports;
mod state;
mod sync;
mod validator;

#[cfg(test)]
mod tests;

use std::time::Duration;

use anyhow::{Context, Result};
use tokio::select;
use tracing::info;

use crate::{
    binaries::TonBinaries,
    bootstrap::{acquire_lock, shutdown_signal, supervise},
    cli::JoinArgs,
    node,
    runtime::ProcessRegistry,
    storage::{Layout, RuntimeState},
    ton::{global_config::GlobalConfig, toolchain::Toolchain},
};

use self::{
    state::prepare_join_state,
    sync::{LocalLiteserver, wait_for_network_sync},
    validator::{apply_network_validator_config, validation_loop},
};

/// Joins an existing TON network and owns one node for this state directory.
///
/// The workflow prepares durable node state before process startup, waits until its
/// local liteserver follows the network head, and only then enables validator automation.
/// Every exit path converges on process shutdown and runtime-state cleanup.
pub async fn run(args: JoinArgs) -> Result<()> {
    std::fs::create_dir_all(&args.state.state_dir).with_context(|| {
        format!(
            "failed to create join state directory {}",
            args.state.state_dir.display()
        )
    })?;

    let state_root = dunce::canonicalize(&args.state.state_dir).with_context(|| {
        format!(
            "failed to resolve join state directory {}",
            args.state.state_dir.display()
        )
    })?;

    let layout = Layout::new(state_root);
    layout.create_dirs()?;

    let _state_lock = acquire_lock(&layout.lock)?;
    let processes = ProcessRegistry::default();

    // Install signal handling before any TON process starts. Otherwise Ctrl+C
    // during initial synchronization terminates only the Localton instance and skips
    // the managed-process cleanup below.
    let run_result = select! {
        result = async {
            // Commit network identity and node settings before starting processes.
            let settings = prepare_join_state(&layout, &args).await?;
            let node_settings = &settings.node;
            let node_name = node_settings.name.clone();
            let binaries = TonBinaries::resolve(&layout, args.ton_bin_dir.clone()).await?;
            let toolchain = Toolchain::official(layout.clone(), binaries);
            let startup_timeout = Duration::from_secs(args.startup_timeout);

            // Publish instance ownership before any persistent child starts. Every
            // later error converges on the cleanup boundary outside this block.
            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.mark_instance_started();
                Ok(())
            })?;

            // Start the host-local node and retain the liteserver identity needed
            // to query it without trusting the remote global-config liteserver list.
            let node_layout = layout.node.clone();
            node::initialize_joined_node(
                &layout,
                &node_layout,
                &toolchain,
                node_settings,
                startup_timeout,
            )
            .await
            .with_context(|| format!("failed to initialize joined node `{node_name}`"))?;

            let mut runtime = node::start(
                &layout,
                &node_layout,
                &toolchain,
                node_settings,
                startup_timeout,
                &processes,
            )
                .await
                .with_context(|| format!("failed to start joined node `{node_name}`"))?;

            let local_liteserver = LocalLiteserver {
                port: node_settings.liteserver_port,
                public_key: runtime
                    .liteserver_public_key
                    .context("joined node has no local liteserver identity")?,
            };

            runtime.begin_synchronization();

            RuntimeState::update_atomic(&layout.runtime, |state| {
                state.node = runtime;
                Ok(())
            })?;

            // A node is usable only after several consecutive near-head samples.
            let masterchain_seqno = select! {
                result = wait_for_network_sync(
                    &layout,
                    &toolchain,
                    node_settings,
                    &local_liteserver,
                ) => result?,
                result = supervise(&processes) => return result,
            };

            RuntimeState::update_atomic(&layout.runtime, |runtime| {
                runtime.node.status = "running".to_owned();
                runtime.mark_network_ready(masterchain_seqno);
                Ok(())
            })?;

            // Once synchronized, all wallet and election work must use a node on
            // this host instead of depending on the bootstrap host's liteserver.
            GlobalConfig::load(&layout.global_config)?
                .with_local_liteserver(local_liteserver.port, local_liteserver.public_key)
                .save_atomic(&node_layout.global_config)?;

            let toolchain = toolchain.with_node_config(&node_layout);
            apply_network_validator_config(&toolchain).await?;

            info!(
                endpoint = %format!("127.0.0.1:{}", local_liteserver.port),
                "join operations now use the synchronized local liteserver"
            );

            info!(
                node = node_name,
                validator = node_settings.validator,
                global_config = %args.global_config_url,
                "joined Localton node is running"
            );

            // Election automation is long-lived and supervised alongside the TON
            // processes so either failure tears down the whole owned instance.
            let validation_interval = toolchain.settings()?.validation.poll_interval_seconds;
            let validation = validation_loop(
                toolchain,
                args.faucet.clone(),
                validation_interval,
            );
            tokio::pin!(validation);
            let result = select! {
                result = supervise(&processes) => result,
                result = &mut validation => result,
            };

            result
        } => result,
        signal_result = shutdown_signal() => signal_result,
    };

    // Cleanup runs even when preparation, synchronization, supervision, or signal
    // handling fails. Preserve all errors by combining the independent results.
    let stop_result = processes.stop_all().await;
    let state_result = RuntimeState::update_atomic(&layout.runtime, |runtime| {
        runtime.mark_instance_stopped();
        Ok(())
    });

    run_result.and(stop_result).and(state_result.map(|_| ()))
}
