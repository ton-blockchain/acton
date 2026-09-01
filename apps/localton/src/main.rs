mod binaries;
mod bootstrap;
mod cache;
mod cli;
mod http;
mod join;
mod node;
mod observability;
mod operations;
mod runtime;
mod storage;
mod ton;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("localton=info")),
        )
        .with_target(false)
        .without_time()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Bootstrap(args) => bootstrap::run(args).await,
        Command::Join(args) => join::run(args).await,
        Command::Status(args) => operations::status::execute(args),
        Command::Config { command } => cli::commands::config(command).await,
        Command::Lite { command } => cli::commands::lite(command).await,
        Command::Wallet { command } => operations::wallets::execute(command).await,
        Command::Indexer { command } => operations::indexer::execute(command).await,
        Command::Node { command } => operations::nodes::execute(command).await,
        Command::Snapshot { command } => operations::snapshots::execute(command),
        Command::Validator { command } => operations::validators::execute(command).await,
        Command::Hardfork(args) => operations::hardfork::execute(args).await,
    }
}
