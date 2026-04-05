use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Args)]
pub(crate) struct DistArgs {
    #[command(subcommand)]
    command: DistCommand,
}

pub(crate) fn run(args: DistArgs) -> Result<()> {
    match args.command {
        DistCommand::Announcement => run_announcement(),
        DistCommand::Build(args) => run_build(args),
        DistCommand::Check => run_check(),
        DistCommand::Installers => run_installers(),
    }
}

#[derive(Subcommand)]
enum DistCommand {
    Announcement,
    Build(BuildArgs),
    Check,
    Installers,
}

#[derive(Args)]
struct BuildArgs {
    #[arg(long, value_name = "TARGET")]
    target: String,
}

fn run_announcement() -> Result<()> {
    println!(
        "mock dist announcement: TODO: replace print with real release announcement generation",
    );
    Ok(())
}

fn run_build(args: BuildArgs) -> Result<()> {
    println!(
        "mock dist build: target=`{}`; TODO: replace print with real build",
        args.target
    );
    Ok(())
}

fn run_check() -> Result<()> {
    println!(
        "mock dist announcement: TODO: replace print with real release announcement generation",
    );
    Ok(())
}

fn run_installers() -> Result<()> {
    println!("mock dist installers: TODO: replace print with real installer creation",);
    Ok(())
}
