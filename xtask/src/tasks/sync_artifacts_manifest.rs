use std::fs;
use std::path::Path;

use crate::modules::github::Github;
use anyhow::{Context, Result};

const TRUNK_OBJS_RELEASE_TAG: &str = "trunk-objs";
const ARTIFACTS_MANIFEST_ASSET_NAME: &str = "artifacts_manifest.toml";
const TON_OBJS_ARTIFACTS_MANIFEST_PATH: &str = "crates/ton-objs/artifacts_manifest.toml";

pub(crate) fn run() -> Result<()> {
    println!(
        "Syncing `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` from GitHub release `{TRUNK_OBJS_RELEASE_TAG}`"
    );

    let github = Github::new();
    let downloaded_asset =
        github.download_release_asset(TRUNK_OBJS_RELEASE_TAG, ARTIFACTS_MANIFEST_ASSET_NAME)?;

    let manifest_contents = std::str::from_utf8(&downloaded_asset)
        .context("downloaded artifacts manifest is not valid UTF-8")?;

    manifest_contents
        .parse::<toml_edit::DocumentMut>()
        .context("downloaded artifacts manifest is not valid TOML")?;

    write_manifest_if_changed(
        Path::new(TON_OBJS_ARTIFACTS_MANIFEST_PATH),
        manifest_contents,
    )?;

    println!("Synchronized `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}`");

    Ok(())
}

fn write_manifest_if_changed(path: &Path, contents: &str) -> Result<()> {
    match fs::read_to_string(path) {
        Ok(existing) if existing == contents => {
            println!("`{}` is already up to date", path.display());
            return Ok(());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read existing `{}`", path.display()));
        }
    }

    fs::write(path, contents).with_context(|| format!("failed to write `{}`", path.display()))
}
