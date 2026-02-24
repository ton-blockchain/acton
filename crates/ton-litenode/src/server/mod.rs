pub mod handlers;
pub mod models;
pub mod router;

use crate::litenode::LiteNode;
use acton_config::color::OwoColorize;
use std::sync::Arc;

pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
}

pub async fn run_server(node: Arc<LiteNode>, args: ServerArgs) -> anyhow::Result<()> {
    let app = router::create_router(node);

    let address = format!("localhost:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!(
        "    {} LiteNode server on http://{address}",
        "Starting".green().bold(),
    );
    if let Some(fork_network) = args.fork_network {
        let fork_source = args
            .fork_block_number
            .map(|seqno| format!("{fork_network} at seqno {seqno}"))
            .unwrap_or(fork_network);
        println!("     {} from {}", "Forking".green().bold(), fork_source);
    }
    axum::serve(listener, app).await?;
    Ok(())
}
