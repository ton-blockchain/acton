pub mod handlers;
pub mod models;
pub mod router;

use crate::liteapi;
use crate::localnet::Localnet;
use crate::node::StateSource;
use acton_config::color::OwoColorize;
use axum::extract::FromRef;
use serde::Serialize;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast;
use ton_api::OffchainJsonResolver;

#[derive(Clone, Debug, Serialize)]
pub struct StartupAccount {
    pub name: String,
    pub version: String,
    pub network: String,
    pub address: String,
}

#[derive(Clone)]
pub struct ServerState {
    pub node: Arc<Localnet>,
    pub offchain_metadata: OffchainJsonResolver,
    pub startup_accounts: Arc<Vec<StartupAccount>>,
    pub shutdown: ShutdownSignal,
    pub network_conditions: NetworkConditions,
    pub rate_limit_rps: Option<u32>,
    pub auth_token: Option<Arc<str>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StateSourceInfo {
    pub state_source: &'static str,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
}

impl From<&StateSource> for StateSourceInfo {
    fn from(state_source: &StateSource) -> Self {
        match state_source {
            StateSource::Local => Self {
                state_source: "local",
                fork_network: None,
                fork_block_number: None,
            },
            StateSource::Remote(provider) => Self {
                state_source: "remote",
                fork_network: Some(provider.network.to_string()),
                fork_block_number: provider.fork_block_number,
            },
        }
    }
}

#[derive(Clone)]
pub struct NetworkConditions {
    response_delay_ms: Arc<AtomicU64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkConditionsInfo {
    pub response_delay_ms: u64,
}

impl NetworkConditions {
    fn new(response_delay_ms: Option<u64>) -> Self {
        Self {
            response_delay_ms: Arc::new(AtomicU64::new(response_delay_ms.unwrap_or_default())),
        }
    }

    #[must_use]
    pub fn response_delay_ms(&self) -> u64 {
        self.response_delay_ms.load(Ordering::Relaxed)
    }

    pub fn set_response_delay_ms(&self, response_delay_ms: u64) {
        self.response_delay_ms
            .store(response_delay_ms, Ordering::Relaxed);
    }

    #[must_use]
    pub fn info(&self) -> NetworkConditionsInfo {
        NetworkConditionsInfo {
            response_delay_ms: self.response_delay_ms(),
        }
    }
}

#[derive(Clone)]
pub struct ShutdownSignal {
    tx: broadcast::Sender<()>,
}

impl ShutdownSignal {
    fn new() -> Self {
        // Streaming handlers are long-lived; graceful shutdown waits until they exit.
        let (tx, _) = broadcast::channel(1);
        Self { tx }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    fn notify(&self) {
        let _ = self.tx.send(());
    }
}

impl FromRef<ServerState> for Arc<Localnet> {
    fn from_ref(state: &ServerState) -> Self {
        state.node.clone()
    }
}

impl FromRef<ServerState> for OffchainJsonResolver {
    fn from_ref(state: &ServerState) -> Self {
        state.offchain_metadata.clone()
    }
}

impl FromRef<ServerState> for Arc<Vec<StartupAccount>> {
    fn from_ref(state: &ServerState) -> Self {
        state.startup_accounts.clone()
    }
}

impl FromRef<ServerState> for ShutdownSignal {
    fn from_ref(state: &ServerState) -> Self {
        state.shutdown.clone()
    }
}

impl FromRef<ServerState> for NetworkConditions {
    fn from_ref(state: &ServerState) -> Self {
        state.network_conditions.clone()
    }
}

pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
    pub rate_limit_rps: Option<u32>,
    pub response_delay_ms: Option<u64>,
    pub startup_accounts: Vec<StartupAccount>,
    pub auth_token: Option<String>,
    pub liteapi: bool,
    pub liteapi_port: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("failed to bind localnet server to {address}")]
    Bind { address: String, source: io::Error },
    #[error("localnet LiteAPI port overflows u16 for HTTP port {http_port}")]
    LiteApiPortOverflow { http_port: u16 },
    #[error("failed to bind localnet LiteAPI to {address}")]
    LiteApiBind { address: String, source: io::Error },
    #[error("localnet server stopped with an error")]
    Serve { source: io::Error },
    #[error("failed to initialize the off-chain metadata HTTP client")]
    OffchainMetadataClient { source: reqwest::Error },
}

pub async fn run_server(node: Arc<Localnet>, args: ServerArgs) -> Result<(), ServerError> {
    let ServerArgs {
        port,
        db_path: _,
        fork_network,
        fork_block_number,
        rate_limit_rps,
        response_delay_ms,
        startup_accounts,
        auth_token,
        liteapi,
        liteapi_port,
    } = args;
    let auth_token = auth_token.map(Arc::<str>::from);

    let network_conditions = NetworkConditions::new(response_delay_ms);
    let shutdown = ShutdownSignal::new();
    let offchain_metadata = OffchainJsonResolver::new()
        .map_err(|source| ServerError::OffchainMetadataClient { source })?;
    let app = router::create_router(ServerState {
        node: Arc::clone(&node),
        offchain_metadata,
        startup_accounts: Arc::new(startup_accounts),
        shutdown: shutdown.clone(),
        network_conditions: network_conditions.clone(),
        rate_limit_rps,
        auth_token: auth_token.clone(),
    });

    let address = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|source| ServerError::Bind {
            address: address.clone(),
            source,
        })?;
    let liteapi_endpoint = if liteapi {
        let liteapi_port = match liteapi_port {
            Some(liteapi_port) => liteapi_port,
            None => port
                .checked_add(1)
                .ok_or(ServerError::LiteApiPortOverflow { http_port: port })?,
        };
        Some(
            liteapi::spawn_liteapi_server(Arc::clone(&node), liteapi_port)
                .await
                .map_err(|source| ServerError::LiteApiBind {
                    address: format!("127.0.0.1:{liteapi_port}"),
                    source,
                })?,
        )
    } else {
        None
    };
    println!(
        "    {} Localnet server on http://{address}",
        "Starting".green().bold(),
    );
    if let Some(liteapi_endpoint) = liteapi_endpoint {
        println!(
            "    {} Localnet LiteAPI on tcp://{}",
            "Starting".green().bold(),
            liteapi_endpoint.address,
        );
        println!(
            "         {} LiteAPI public key: {}",
            "Key".yellow().bold(),
            liteapi_endpoint.public_key_base64
        );
    }
    if let Some(token) = auth_token.as_deref() {
        println!(
            "        {} Localnet API token: {}",
            "Auth".yellow().bold(),
            token
        );
    }
    if let Some(fork_network) = fork_network {
        let fork_source = fork_block_number
            .map(|seqno| format!("{fork_network} at seqno {seqno}"))
            .unwrap_or(fork_network);
        println!("     {} from {}", "Forking".green().bold(), fork_source);
    }
    if let Some(limit) = rate_limit_rps {
        println!(
            "    {} API requests to {} req/s",
            "Limiting".yellow().bold(),
            limit
        );
    }
    let delay_ms = network_conditions.response_delay_ms();
    if delay_ms > 0 {
        println!(
            "    {} API v2/v3/emulate responses by {}ms",
            "Delaying".yellow().bold(),
            delay_ms
        );
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                println!("  {} Localnet server", "Stopping".yellow().bold());
                shutdown.notify();
            }
        })
        .await
        .map_err(|source| ServerError::Serve { source })?;
    Ok(())
}
