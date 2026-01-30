mod litenode;
mod server;

use crate::litenode::LiteNode;
use clap::{Parser, Subcommand};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "ton-litenode")]
#[command(about = "A lightweight TON node for testing")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start {
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { port } => {
            let node = Arc::new(LiteNode::new());
            server::run_server(node, port).await?;
        }
    }

    Ok(())
}
