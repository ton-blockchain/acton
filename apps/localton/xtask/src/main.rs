mod http_api;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "xtask", about = "Build-time tasks for localton")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Build and install the pinned TON HTTP API V2 backend.
    BuildTonHttpApiV2(BuildTonHttpApiV2Args),
}

#[derive(Debug, Args)]
struct BuildTonHttpApiV2Args {
    /// State directory whose tools directory receives the installed artifacts.
    #[arg(long, default_value = ".localton")]
    state_dir: PathBuf,

    /// Number of parallel native compilation jobs.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(1..=64))]
    jobs: u8,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_target(false)
        .compact()
        .init();

    match Cli::parse().command {
        Command::BuildTonHttpApiV2(args) => {
            http_api::build(&args.state_dir, usize::from(args.jobs)).await
        }
    }
}
