mod client;
mod workflow;

#[cfg(test)]
mod tests;

use anyhow::Result;
use std::env;

use client::GitHubClient;
use workflow::run_update;

pub fn up_cmd(version: Option<String>, canary: bool, stable: bool, yes: bool) -> Result<()> {
    let client = GitHubClient::new();
    let current_exe = env::current_exe()?;
    let current_version_str = env!("CARGO_PKG_VERSION");

    run_update(
        &client,
        &current_exe,
        current_version_str,
        version,
        canary,
        stable,
        yes,
    )
}
