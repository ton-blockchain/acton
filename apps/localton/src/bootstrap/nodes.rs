//! Startup of the local DHT and genesis validator-engine.

use std::time::Duration;

use anyhow::{Context, Result};
use tracing::info;

use crate::{
    runtime::ProcessRegistry,
    storage::Layout,
    storage::Settings,
    ton::{
        toolchain::Toolchain,
        tools::{
            dht_server::DhtStartRequest,
            types::{AdnlEndpoint, DhtDatabase, OperationContext},
            validator_engine::ValidatorDatabase,
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
