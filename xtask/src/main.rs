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
    Hello,
    UbicloudCleanup(tasks::ubicloud_cleanup::UbicloudCleanupArgs),
    Release(tasks::release::ReleaseArgs),
    Schema(tasks::schema::SchemaArgs),
}

fn main() -> Result<()> {
    let args = Cli::parse();

    match args.command {
        CliCommand::Hello => tasks::hello::run(),
        CliCommand::UbicloudCleanup(args) => tasks::ubicloud_cleanup::run(args),
        CliCommand::Release(args) => tasks::release::run(args),
        CliCommand::Schema(args) => tasks::schema::run(args),
    }
}
