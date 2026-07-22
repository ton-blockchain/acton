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
    DeployExplorer(tasks::deploy_explorer::DeployExplorerArgs),
    Dist(tasks::dist::DistArgs),
    GithubCleanup(tasks::github_cleanup::GithubCleanupArgs),
    Hello,
    Release(tasks::release::ReleaseArgs),
    Retag(tasks::retag::RetagArgs),
    Schema(tasks::schema::SchemaArgs),
    SyncArtifacts,
    UbicloudCleanup(tasks::ubicloud_cleanup::UbicloudCleanupArgs),
    UpdateDefaultConfig(tasks::update_default_config::UpdateDefaultConfigArgs),
    UpdateTemplateWrappers,
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        CliCommand::DeployExplorer(args) => tasks::deploy_explorer::run(args),
        CliCommand::Dist(args) => tasks::dist::run(args),
        CliCommand::GithubCleanup(args) => tasks::github_cleanup::run(args),
        CliCommand::Hello => tasks::hello::run(),
        CliCommand::Release(args) => tasks::release::run(args),
        CliCommand::Retag(args) => tasks::retag::run(args),
        CliCommand::Schema(args) => tasks::schema::run(args),
        CliCommand::SyncArtifacts => tasks::sync_artifacts::run(),
        CliCommand::UbicloudCleanup(args) => tasks::ubicloud_cleanup::run(args),
        CliCommand::UpdateDefaultConfig(args) => tasks::update_default_config::run(args),
        CliCommand::UpdateTemplateWrappers => tasks::update_template_wrappers::run(),
    }
}
