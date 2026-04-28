use crate::commands::new::extract_standalone_app_scaffold;
use acton_config::color::OwoColorize;
use anyhow::Context;
use std::path::Path;

pub const DEFAULT_APP_DIR: &str = "app";
const STANDALONE_APP_NPM_NAME: &str = "ton-dapp-template";

pub fn create_app_cmd(path: Option<&Path>) -> anyhow::Result<()> {
    let target_dir = resolve_target_dir(path);
    validate_app_target_dir(target_dir)?;
    extract_standalone_app_scaffold(target_dir, STANDALONE_APP_NPM_NAME)
        .context("Failed to create app scaffold")?;
    print_app_created_message(target_dir);

    Ok(())
}

fn validate_app_target_dir(target_dir: &Path) -> anyhow::Result<()> {
    if target_dir.exists() {
        anyhow::bail!(
            "Directory {} already exists. Delete it before running `acton init --create-app`.",
            target_dir.display().to_string().yellow()
        );
    }

    Ok(())
}

fn print_app_created_message(target_dir: &Path) {
    println!("{}", "✓ Created TypeScript app scaffold".green().bold());
    println!(
        "  {} {}",
        "Directory:".bright_black(),
        target_dir.display().to_string().cyan()
    );
    println!();
    println!("Next steps:");
    println!();
    println!("  {}", "# Install app dependencies".dimmed());
    println!("  {} {}", "cd".bold(), target_dir.display());
    println!("  {} ci", "npm".bold());
    println!("  {}", "# Start the TypeScript app".dimmed());
    println!("  {} run dev", "npm".bold());
}

fn resolve_target_dir(path: Option<&Path>) -> &Path {
    path.unwrap_or_else(|| Path::new(DEFAULT_APP_DIR))
}
