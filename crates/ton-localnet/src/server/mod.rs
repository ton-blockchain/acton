pub mod handlers;
pub mod models;
pub mod router;

use crate::localnet::Localnet;
use acton_config::color::OwoColorize;
use axum::extract::FromRef;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize)]
pub struct StartupWallet {
    pub name: String,
    pub mnemonic: Vec<String>,
    pub version: String,
    pub network: String,
    pub address: String,
    pub public_key: String,
    pub wallet_id: i32,
}

#[derive(Clone)]
pub struct ServerState {
    pub node: Arc<Localnet>,
    pub startup_wallets: Arc<Vec<StartupWallet>>,
}

impl FromRef<ServerState> for Arc<Localnet> {
    fn from_ref(state: &ServerState) -> Self {
        state.node.clone()
    }
}

impl FromRef<ServerState> for Arc<Vec<StartupWallet>> {
    fn from_ref(state: &ServerState) -> Self {
        state.startup_wallets.clone()
    }
}

pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
    pub rate_limit_rps: Option<u32>,
    pub startup_wallets: Vec<StartupWallet>,
}

pub async fn run_server(node: Arc<Localnet>, args: ServerArgs) -> anyhow::Result<()> {
    let app = router::create_router(
        ServerState {
            node,
            startup_wallets: Arc::new(args.startup_wallets),
        },
        args.rate_limit_rps,
    );

    let address = format!("127.0.0.1:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!(
        "    {} Localnet server and UI on http://{address}",
        "Starting".green().bold(),
    );
    if let Some(fork_network) = args.fork_network {
        let fork_source = args
            .fork_block_number
            .map(|seqno| format!("{fork_network} at seqno {seqno}"))
            .unwrap_or(fork_network);
        println!("    {} from {}", "Forking".green().bold(), fork_source);
    }
    if let Some(limit) = args.rate_limit_rps {
        println!(
            "    {} API requests to {} req/s",
            "Limiting".yellow().bold(),
            limit
        );
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            if tokio::signal::ctrl_c().await.is_ok() {
                println!("  {} Localnet server", "Stopping".yellow().bold());
            }
        })
        .await?;
    Ok(())
}
