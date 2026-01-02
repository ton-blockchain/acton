mod client;
mod workflow;

#[cfg(test)]
mod tests;

use anyhow::Result;
use owo_colors::OwoColorize;
use std::env;

use client::{GitHubClient, ReleaseClient};
use workflow::{check_update, run_update};

pub fn up_cmd(
    version: Option<String>,
    canary: bool,
    stable: bool,
    yes: bool,
    list: bool,
    check: bool,
) -> Result<()> {
    let token = env::var("GITHUB_TOKEN").ok();
    let client = GitHubClient::new(token);
    let current_exe = env::current_exe()?;
    let current_version_str = env!("CARGO_PKG_VERSION");

    if check {
        let info = check_update(&client, current_version_str)?;
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    if list {
        let releases = client.list_releases()?;
        println!("Available versions:");
        for release in releases {
            println!("  {}", release.yellow());
        }
        return Ok(());
    }

    let result = run_update(
        &client,
        &current_exe,
        current_version_str,
        version,
        canary,
        stable,
        yes,
    );

    if let Err(e) = result {
        if e.to_string().contains("Release not found") {
            let releases = client.list_releases()?;
            eprintln!("{}: {}", "Error".red(), e);
            eprintln!("\nAvailable versions:");
            for release in releases {
                eprintln!("  {}", release.yellow());
            }
            eprintln!();
            eprintln!("Check the available versions above");
            eprintln!();
            return Err(anyhow::anyhow!("Update failed due to unknown version"));
        }
        return Err(e);
    }

    Ok(())
}
