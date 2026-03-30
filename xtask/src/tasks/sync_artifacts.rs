use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::str;

use crate::modules::github::Github;
use anyhow::{Context, Result};
use clap::Args;
use flate2::bufread::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use tar::Archive;

const TRUNK_OBJS_RELEASE_TAG: &str = "trunk-objs";
const ARTIFACTS_MANIFEST_ASSET_NAME: &str = "artifacts_manifest.toml";
const TON_OBJS_ARTIFACTS_MANIFEST_PATH: &str = "crates/ton-objs/artifacts_manifest.toml";
const TON_OBJS_DIR: &str = "objs";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SyncStatus {
    Updated,
    AlreadyUpToDate,
}

#[derive(Args)]
pub(crate) struct SyncArtifactsArgs {
    #[arg(long = "force", alias = "force")]
    pub(crate) force: bool,
}

pub(crate) fn run(args: SyncArtifactsArgs) -> Result<()> {
    println!(
        "Syncing `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` from GitHub release `{TRUNK_OBJS_RELEASE_TAG}`"
    );

    let objs_dir = Path::new(TON_OBJS_DIR);
    let should_refresh_objs = args.force || !objs_dir.is_dir();
    let github = Github::new();
    let downloaded_asset =
        github.download_release_asset(TRUNK_OBJS_RELEASE_TAG, ARTIFACTS_MANIFEST_ASSET_NAME)?;

    let manifest_contents = str::from_utf8(&downloaded_asset)
        .context("downloaded artifacts manifest is not valid UTF-8")?;

    let sync_status = write_manifest_if_changed(
        Path::new(TON_OBJS_ARTIFACTS_MANIFEST_PATH),
        manifest_contents,
        args.force,
    )?;

    if sync_status == SyncStatus::Updated {
        println!("Synchronized `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}`");
    } else {
        println!("`{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` is already up to date");
    }

    if should_refresh_objs || sync_status == SyncStatus::Updated {
        maybe_offer_local_objs_refresh(&github, objs_dir, args.force)?;
    }

    Ok(())
}

fn write_manifest_if_changed(path: &Path, contents: &str, force: bool) -> Result<SyncStatus> {
    if force {
        fs::write(path, contents)
            .with_context(|| format!("failed to write `{}`", path.display()))?;
        return Ok(SyncStatus::Updated);
    }

    match fs::read_to_string(path) {
        Ok(existing) if existing == contents => {
            return Ok(SyncStatus::AlreadyUpToDate);
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read existing `{}`", path.display()));
        }
    }

    fs::write(path, contents).with_context(|| format!("failed to write `{}`", path.display()))?;

    Ok(SyncStatus::Updated)
}

fn current_release_target() -> Result<String> {
    option_env!("X_TASK_TARGET")
        .map(str::trim)
        .filter(|target| !target.is_empty())
        .map(str::to_owned)
        .context("xtask was built without embedded target triple")
}

fn release_archive_name(rust_target: &str) -> String {
    format!("ton-objs-{rust_target}.tar.gz")
}

fn maybe_offer_local_objs_refresh(github: &Github, objs_dir: &Path, force: bool) -> Result<()> {
    let rust_target = current_release_target()?;
    let archive_name = release_archive_name(&rust_target);

    if !force && objs_dir.is_dir() {
        print!(
            "`{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` changed. Update local `objs/` from release `{TRUNK_OBJS_RELEASE_TAG}` asset `{archive_name}`? Type `yes` to continue: "
        );
        io::stdout()
            .flush()
            .context("failed to flush confirmation prompt")?;

        let mut confirmation = Vec::new();
        io::stdin()
            .lock()
            .read_until(b'\n', &mut confirmation)
            .context("failed to read confirmation")?;

        if String::from_utf8_lossy(&confirmation).trim() != "yes" {
            println!("Skipped updating local `objs/`");
            return Ok(());
        }
    }

    refresh_local_objs_from_release(github, &archive_name, objs_dir)?;
    println!(
        "Updated local `objs/` from release `{TRUNK_OBJS_RELEASE_TAG}` asset `{archive_name}`"
    );

    Ok(())
}

fn refresh_local_objs_from_release(
    github: &Github,
    archive_name: &str,
    objs_dir: &Path,
) -> Result<()> {
    let progress = ProgressBar::new_spinner();
    progress.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .context("failed to build download progress style")?,
    );
    progress.set_message(format!(
        "Downloading `{archive_name}` from GitHub release `{TRUNK_OBJS_RELEASE_TAG}`"
    ));
    progress.enable_steady_tick(std::time::Duration::from_millis(120));

    let downloaded_archive = github.download_release_asset(TRUNK_OBJS_RELEASE_TAG, archive_name);
    progress.finish_and_clear();

    let downloaded_archive = downloaded_archive?;
    fs::create_dir_all(objs_dir)
        .with_context(|| format!("failed to create `{}`", objs_dir.display()))?;

    let decoder = GzDecoder::new(downloaded_archive.as_slice());
    let mut archive = Archive::new(decoder);
    archive.unpack(objs_dir).with_context(|| {
        format!(
            "failed to unpack `{archive_name}` into `{}`",
            objs_dir.display()
        )
    })
}
