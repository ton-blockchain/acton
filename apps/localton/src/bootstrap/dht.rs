//! DHT bootstrap workflow for the first node in a Localton network.
//!
//! This module owns the cross-tool lifecycle: initialize a stable DHT database,
//! then ask `generate-random-id` to sign one descriptor for every generated
//! identity. Typed adapters own command syntax, process lifecycle, JSON rendering,
//! parsing, and release-specific validation.

use std::time::Duration;

use anyhow::Result;
use tracing::{Instrument, info, info_span, warn};

use crate::{
    runtime::ProcessRegistry,
    storage::{Layout, NodeSettings, ServiceRuntime},
    ton::tools::{
        dht_server::{DhtInitializeRequest, DhtServer, DhtStartRequest},
        random_id::{DhtDescriptorRequest, RandomIdGenerator},
        types::{AdnlEndpoint, DhtDatabase, DhtNodeDescriptor, OperationContext},
    },
};

/// Starts the persistent DHT service and transfers ownership to the process registry.
///
/// DHT is bootstrap infrastructure rather than a blockchain node. Its lifecycle
/// therefore remains here, while validator-engine startup is shared through the
/// top-level node module.
pub(super) async fn start(
    layout: &Layout,
    dht_server: &dyn DhtServer,
    node: &NodeSettings,
    database: DhtDatabase,
    timeout: Duration,
    processes: &ProcessRegistry,
) -> Result<ServiceRuntime> {
    let context = OperationContext::for_node(timeout, &node.name);
    let process = dht_server
        .start(
            &context,
            DhtStartRequest {
                global_config: layout.global_config.clone(),
                database,
                log_path: layout.logs.join("dht-engine"),
                stdout_log: layout.logs.join("dht.stdout.log"),
                stderr_log: layout.logs.join("dht.stderr.log"),
                endpoint: AdnlEndpoint::new(node.public_ip, node.dht_port),
                threads: usize::from(node.threads),
                verbosity: node.verbosity,
            },
        )
        .await?;
    let runtime = ServiceRuntime {
        running: true,
        pid: process.pid(),
        endpoint: None,
        last_error: None,
    };

    processes.insert(process).await?;

    Ok(runtime)
}

/// Typed output of the cross-tool DHT initialization workflow.
///
/// Persistent startup reuses [`Self::database`] so the ADNL identity published by
/// [`Self::descriptors`] remains stable. JSON conversion is intentionally deferred
/// until the global-config builder, where release-owned descriptors are serialized.
#[derive(Debug)]
pub(super) struct InitializedDht {
    /// Validated persistent database reopened by normal bootstrap startup.
    pub(super) database: DhtDatabase,
    /// Signed bootstrap descriptors published in final global config.
    pub(super) descriptors: Vec<DhtNodeDescriptor>,
}

/// Creates the persistent DHT database and returns signed node descriptors.
///
/// `DhtServer` first generates and validates its database. Every returned keyring
/// identity is then combined with the advertised endpoint by `RandomIdGenerator`.
/// This ordering breaks the preliminary-global-config/DHT-descriptor cycle without
/// exposing either executable's argv or stdout to the workflow.
pub(super) async fn initialize_dht(
    layout: &Layout,
    dht_server: &dyn DhtServer,
    random_id: &dyn RandomIdGenerator,
    node: &NodeSettings,
    context: &OperationContext,
) -> Result<InitializedDht> {
    let endpoint = AdnlEndpoint::new(node.public_ip, node.dht_port);
    let workflow_span = info_span!(
        "dht_bootstrap_workflow",
        workflow = "dht_bootstrap",
        node = context.node_name.as_deref().unwrap_or(node.name.as_str()),
        endpoint = %endpoint,
    );
    let result = async {
        info!(
            milestone = "database_initialization_requested",
            database_path = %layout.dht_db.display(),
            global_config_path = %layout.global_config.display(),
            "starting DHT bootstrap"
        );
        let database = dht_server
            .initialize(
                context,
                DhtInitializeRequest {
                    global_config: layout.global_config.clone(),
                    database: layout.dht_db.clone(),
                    log_path: layout.logs.join("dht-init"),
                    endpoint,
                    out_port: node.out_port,
                    threads: usize::from(node.threads),
                    verbosity: node.verbosity,
                },
            )
            .await?;
        info!(
            milestone = "database_ready",
            database_path = %database.path.display(),
            config_path = %database.config.display(),
            key_count = database.keyring.len(),
            "DHT identities are ready for publication"
        );

        // The address list is a generator-owned scratch artifact reused for each
        // stable keyring identity. The adapter rewrites it atomically before signing.
        let address_list_path = layout.dht_db.join("adnl-address-list.json");
        let descriptor_count = database.keyring.len();
        let mut descriptors = Vec::with_capacity(descriptor_count);
        for (index, private_key) in database.keyring.iter().enumerate() {
            info!(
                milestone = "descriptor_generation_requested",
                descriptor = index + 1,
                descriptor_count,
                "signing DHT bootstrap descriptor"
            );
            descriptors.push(
                random_id
                    .create_dht_descriptor(
                        context,
                        DhtDescriptorRequest {
                            private_key: private_key.clone(),
                            address: endpoint,
                            address_list_path: address_list_path.clone(),
                        },
                    )
                    .await?,
            );
        }
        info!(
            milestone = "descriptors_ready",
            descriptor_count = descriptors.len(),
            address_list_path = %address_list_path.display(),
            "DHT bootstrap descriptors are ready for global config"
        );
        Ok(InitializedDht {
            database,
            descriptors,
        })
    }
    .instrument(workflow_span.clone())
    .await;
    workflow_span.in_scope(|| match &result {
        Ok(initialized) => info!(
            milestone = "complete",
            descriptor_count = initialized.descriptors.len(),
            database_path = %initialized.database.path.display(),
            "DHT bootstrap completed"
        ),
        Err(error) => warn!(milestone = "failed", %error, "DHT bootstrap failed"),
    });
    result
}
