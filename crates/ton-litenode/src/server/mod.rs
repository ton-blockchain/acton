pub mod handlers;
pub mod models;
pub mod router;

use crate::litenode::LiteNode;
use owo_colors::OwoColorize;
use std::sync::Arc;

pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
}

pub async fn run_server(node: Arc<LiteNode>, args: ServerArgs) -> anyhow::Result<()> {
    let app = router::create_router(node);

    let address = format!("localhost:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!(
        "    {} LiteNode server on http://{address}",
        "Starting".green().bold(),
    );
    axum::serve(listener, app).await?;
    Ok(())
}
