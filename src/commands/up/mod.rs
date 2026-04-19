mod client;
mod workflow;

#[cfg(test)]
mod tests;

use crate::build_info;
use acton_config::color::OwoColorize;
use anyhow::Result;
use std::env;
use std::path::PathBuf;

use client::{GitHubClient, ReleaseClient};
use workflow::{check_update, run_update};

#[cfg(debug_assertions)]
const TEST_CURRENT_EXE_ENV: &str = "ACTON_TEST_UP_CURRENT_EXE"; // non-release test hook only

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
    let current_version_str = build_info::PACKAGE_VERSION;
    let current_is_trunk = build_info::is_trunk_build();
    let requested_release =
        requested_release_label(version.as_deref(), trunk, stable, current_is_trunk);

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
    let current_exe = current_executable_path()?;

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
            match client.list_releases() {
                Ok(releases) => {
                    eprintln!("Available versions:");
                    for release in releases {
                        eprintln!("  {}", release.yellow());
                    }
                    eprintln!();
                }
                Err(list_err) => {
                    eprintln!("Could not fetch the available versions list: {list_err}");
                    eprintln!();
                }
            }

            return Err(anyhow::anyhow!(
                "Requested {} was not found in GitHub releases. Run `acton up --list` to inspect available versions.",
                requested_release
            ));
        }
        return Err(e);
    }

    Ok(())
}

fn current_executable_path() -> Result<PathBuf> {
    if let Some(path) = test_current_executable_override()? {
        return Ok(path);
    }

    Ok(env::current_exe()?)
}

fn requested_release_label(
    version: Option<&str>,
    trunk: bool,
    stable: bool,
    current_is_trunk: bool,
) -> String {
    if let Some(version) = version {
        return format!("release `{}`", version.trim());
    }

    if trunk || (current_is_trunk && !stable) {
        return "the `trunk` release".to_owned();
    }

    "the latest stable release".to_owned()
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

#[cfg(debug_assertions)]
fn test_current_executable_override() -> Result<Option<PathBuf>> {
    match env::var(TEST_CURRENT_EXE_ENV) {
        Ok(path) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                anyhow::bail!(
                    "Invalid value for {}: path must not be empty",
                    TEST_CURRENT_EXE_ENV.yellow()
                );
            }
            Ok(Some(PathBuf::from(trimmed)))
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(anyhow::anyhow!(
            "Failed to read {}: {err}",
            TEST_CURRENT_EXE_ENV.yellow()
        )),
    }
}

#[cfg(not(debug_assertions))]
fn test_current_executable_override() -> Result<Option<PathBuf>> {
    Ok(None)
}
