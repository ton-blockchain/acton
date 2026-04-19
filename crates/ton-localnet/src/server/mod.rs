pub mod handlers;
pub mod models;
pub mod router;

use crate::localnet::Localnet;
use acton_config::color::OwoColorize;
use std::sync::Arc;

pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
    pub rate_limit_rps: Option<u32>,
}

pub async fn run_server(node: Arc<Localnet>, args: ServerArgs) -> anyhow::Result<()> {
    let app = router::create_router(node, args.rate_limit_rps);

    let address = format!("localhost:{}", args.port);
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
