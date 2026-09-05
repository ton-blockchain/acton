mod build;
mod http_api_v2;
mod http_api_v3;
mod recursive_load;

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
    BuildTonHttpApiV2(build::BuildArgs),

    /// Build and install the pinned TON Center API V3 components.
    BuildTonHttpApiV3(http_api_v3::BuildV3Args),

    /// Build a deterministic external message for a recursive load root.
    PrepareRecursiveLoad(PrepareRecursiveLoadArgs),

    /// Fund and deploy a recursive load root through the Localton liteserver.
    RunRecursiveLoad(RunRecursiveLoadArgs),
}

#[derive(Debug, Args)]
struct PrepareRecursiveLoadArgs {
    /// Positive identifier that gives this workload tree a distinct address space.
    #[arg(default_value_t = 1)]
    tree_id: u64,
}

#[derive(Debug, Args)]
struct RunRecursiveLoadArgs {
    /// Root balance in GRAM, with at most nine decimal places.
    amount: String,

    /// Positive identifier that gives this workload tree a distinct address space.
    tree_id: u64,

    /// Localton state directory used by the wallet and liteserver commands.
    #[arg(long, default_value = ".localton")]
    state_dir: PathBuf,

    /// Maximum time to wait for funding and deployment confirmations.
    #[arg(long, default_value_t = 600)]
    timeout_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with_target(false)
        .without_time()
        .compact()
        .init();

    match Cli::parse().command {
        Command::BuildTonHttpApiV2(args) => http_api_v2::build(args).await,
        Command::BuildTonHttpApiV3(args) => http_api_v3::build(args).await,
        Command::PrepareRecursiveLoad(args) => recursive_load::prepare(args.tree_id).await,
        Command::RunRecursiveLoad(args) => {
            recursive_load::run(
                &args.amount,
                args.tree_id,
                &args.state_dir,
                args.timeout_seconds,
            )
            .await
        }
    }
}

#[cfg(test)]
#[test]
fn cli_arguments_are_consistent() {
    use clap::CommandFactory;

    Cli::command().debug_assert();
}
