mod binaries;
mod bootstrap;
mod cli;
mod http;
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
        Some(Command::Run(args)) => bootstrap::run(args).await,
        Some(Command::Status(args)) => bootstrap::status(args).await,
        Some(Command::Config { command }) => cli::commands::config(command).await,
        Some(Command::Lite { command }) => cli::commands::lite(command).await,
        Some(Command::Wallet { command }) => operations::wallets::execute(command).await,
        Some(Command::Indexer { command }) => operations::indexer::execute(command).await,
        Some(Command::Node { command }) => operations::nodes::execute(command).await,
        Some(Command::Snapshot { command }) => operations::snapshots::execute(command),
        Some(Command::Validator { command }) => operations::validators::execute(command).await,
        Some(Command::Hardfork(args)) => operations::hardfork::execute(args).await,
        None => bootstrap::run(cli.run).await,
    }
}
