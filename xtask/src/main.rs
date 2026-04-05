mod modules;
mod tasks;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(bin_name = "cargo xtask")]
#[command(subcommand_required = true, arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    Dist(tasks::dist::DistArgs),
    GithubCleanup(tasks::github_cleanup::GithubCleanupArgs),
    Hello,
    Release(tasks::release::ReleaseArgs),
    Retag(tasks::retag::RetagArgs),
    Schema(tasks::schema::SchemaArgs),
    SyncArtifacts(tasks::sync_artifacts::SyncArtifactsArgs),
    UbicloudCleanup(tasks::ubicloud_cleanup::UbicloudCleanupArgs),
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        CliCommand::Dist(args) => tasks::dist::run(args),
        CliCommand::GithubCleanup(args) => tasks::github_cleanup::run(args),
        CliCommand::Hello => tasks::hello::run(),
        CliCommand::Release(args) => tasks::release::run(args),
        CliCommand::Retag(args) => tasks::retag::run(args),
        CliCommand::Schema(args) => tasks::schema::run(args),
        CliCommand::SyncArtifacts(args) => tasks::sync_artifacts::run(args),
        CliCommand::UbicloudCleanup(args) => tasks::ubicloud_cleanup::run(args),
    }
}
