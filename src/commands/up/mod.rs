mod client;
mod workflow;

#[cfg(test)]
mod tests;

use crate::build_info;
use acton_config::color::OwoColorize;
use anyhow::Result;
use std::env;

use client::{GitHubClient, ReleaseClient};
use workflow::{check_update, run_update};

pub fn up_cmd(
    version: Option<String>,
    trunk: bool,
    stable: bool,
    force: bool,
    yes: bool,
    list: bool,
    check: bool,
) -> Result<()> {
    let token = env::var("GITHUB_TOKEN").ok();
    let client = GitHubClient::new(token);
    let current_exe = env::current_exe()?;
    let current_version_str = build_info::PACKAGE_VERSION;
    let current_is_trunk = build_info::is_trunk_build();

    if check {
        let info = check_update(&client, current_version_str, current_is_trunk)?;
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

    validate_version_argument(version.as_deref())?;

    let result = run_update(
        &client,
        &current_exe,
        current_version_str,
        current_is_trunk,
        version,
        trunk,
        stable,
        yes,
        force,
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

fn validate_version_argument(version: Option<&str>) -> Result<()> {
    let Some(version) = version else {
        return Ok(());
    };

    if let Some(expected_flag) = unicode_dash_flag_suggestion(version) {
        anyhow::bail!(
            "{} looks like an option typed with a Unicode dash. Use {} instead.",
            version.yellow(),
            expected_flag.yellow()
        );
    }

    Ok(())
}

fn unicode_dash_flag_suggestion(arg: &str) -> Option<&'static str> {
    let stripped = arg.trim_start_matches(is_unicode_dash);
    if stripped.len() == arg.len() {
        return None;
    }

    match stripped.trim_start_matches('-') {
        "trunk" => Some("--trunk"),
        "stable" => Some("--stable"),
        "force" => Some("--force"),
        "list" => Some("--list"),
        "check" => Some("--check"),
        "yes" => Some("--yes"),
        "y" => Some("-y"),
        "help" => Some("--help"),
        "h" => Some("-h"),
        _ => None,
    }
}

const fn is_unicode_dash(ch: char) -> bool {
    matches!(
        ch,
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}' | '\u{2212}'
    )
}
