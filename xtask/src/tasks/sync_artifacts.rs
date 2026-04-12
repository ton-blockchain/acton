use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::str;

use crate::modules::github::Github;
use anyhow::{Context, Result, bail};
use clap::Args;
use flate2::bufread::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use tar::Archive;
use tempfile::Builder as TempfileBuilder;

const RELEASE_OBJS_RELEASE_TAG: &str = "release-objs";
const TON_OBJS_DIR: &str = "objs";

const ARTIFACTS_MANIFEST_ASSET_NAME: &str = "artifacts_manifest.toml";
const TON_OBJS_ARTIFACTS_MANIFEST_PATH: &str = "crates/ton-objs/artifacts_manifest.toml";

const TON_STDLIB_ASSET_NAME: &str = "ton-stdlib.tar.gz";

const TOLK_STDLIB_ARCHIVE_DIR: &str = "tolk-stdlib";
const TOLK_STDLIB_DIR: &str = "crates/tolkc/assets/tolk-stdlib";

const FIFT_STDLIB_ARCHIVE_DIR: &str = "fift-stdlib";
const FIFT_STDLIB_DIR: &str = "crates/tolkc/assets/fift-stdlib";
const FIFT_STDLIB_FILES: &[&str] = &["Asm.fif", "Fift.fif"];

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
        "Syncing `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` from GitHub release `{RELEASE_OBJS_RELEASE_TAG}`"
    );

    let objs_dir = Path::new(TON_OBJS_DIR);
    let should_refresh_objs = args.force || !objs_dir.is_dir();
    let github = Github::new();
    let downloaded_asset =
        github.download_release_asset(RELEASE_OBJS_RELEASE_TAG, ARTIFACTS_MANIFEST_ASSET_NAME)?;

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
            "`{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` changed. Update local `objs/` and `ton-stdlib` from release `{RELEASE_OBJS_RELEASE_TAG}`? Type `yes` to continue: "
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
            println!("Skipped updating local `objs/` and `ton-stdlib`");
            return Ok(());
        }
    }

    refresh_local_objs_from_release(github, &archive_name, objs_dir)?;
    download_local_stdlib_archive_from_release(
        github,
        Path::new(TOLK_STDLIB_DIR),
        Path::new(FIFT_STDLIB_DIR),
    )?;
    println!(
        "Updated local `objs/` from release `{RELEASE_OBJS_RELEASE_TAG}` asset `{archive_name}`"
    );
    println!(
        "Replaced `{TOLK_STDLIB_DIR}` and `{FIFT_STDLIB_DIR}` from `{TON_STDLIB_ASSET_NAME}` download"
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
        "Downloading `{archive_name}` from GitHub release `{RELEASE_OBJS_RELEASE_TAG}`"
    ));
    progress.enable_steady_tick(std::time::Duration::from_millis(120));

    let downloaded_archive = github.download_release_asset(RELEASE_OBJS_RELEASE_TAG, archive_name);
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

fn download_local_stdlib_archive_from_release(
    github: &Github,
    tolk_stdlib_dir: &Path,
    fift_stdlib_dir: &Path,
) -> Result<()> {
    let progress = ProgressBar::new_spinner();
    progress.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .context("failed to build download progress style")?,
    );
    progress.set_message(format!(
        "Downloading `{TON_STDLIB_ASSET_NAME}` from GitHub release `{RELEASE_OBJS_RELEASE_TAG}`"
    ));
    progress.enable_steady_tick(std::time::Duration::from_millis(120));

    let downloaded_archive =
        github.download_release_asset(RELEASE_OBJS_RELEASE_TAG, TON_STDLIB_ASSET_NAME);
    progress.finish_and_clear();

    let downloaded_archive = downloaded_archive?;
    let mut temp_archive = TempfileBuilder::new()
        .prefix(".ton-stdlib-")
        .suffix(".tar.gz")
        .tempfile()
        .context("failed to create temporary TON stdlib archive")?;
    temp_archive
        .write_all(&downloaded_archive)
        .context("failed to write temporary TON stdlib archive")?;
    temp_archive
        .flush()
        .context("failed to flush temporary TON stdlib archive")?;

    let staging_dir = TempfileBuilder::new()
        .prefix(".sync-stdlib-assets-")
        .tempdir()
        .context("failed to create temporary stdlib staging directory")?;
    let archive_file = fs::File::open(temp_archive.path())
        .with_context(|| format!("failed to open `{}`", temp_archive.path().display()))?;
    let decoder = GzDecoder::new(io::BufReader::new(archive_file));
    let mut archive = Archive::new(decoder);
    archive
        .unpack(staging_dir.path())
        .with_context(|| format!("failed to unpack `{}`", temp_archive.path().display()))?;

    replace_tolk_stdlib_from_unpacked_dir(staging_dir.path(), tolk_stdlib_dir)?;
    replace_fift_stdlib_from_unpacked_dir(staging_dir.path(), fift_stdlib_dir)
}

fn replace_tolk_stdlib_from_unpacked_dir(
    unpacked_dir: &Path,
    tolk_stdlib_dir: &Path,
) -> Result<()> {
    let staged_tolk_stdlib_dir = unpacked_dir.join(TOLK_STDLIB_ARCHIVE_DIR);
    if !staged_tolk_stdlib_dir.is_dir() {
        bail!("`{TON_STDLIB_ASSET_NAME}` did not contain `{TOLK_STDLIB_ARCHIVE_DIR}/`");
    }

    if tolk_stdlib_dir.exists() {
        fs::remove_dir_all(tolk_stdlib_dir)
            .with_context(|| format!("failed to remove `{}`", tolk_stdlib_dir.display()))?;
    }

    copy_directory(&staged_tolk_stdlib_dir, tolk_stdlib_dir)
}

fn replace_fift_stdlib_from_unpacked_dir(
    unpacked_dir: &Path,
    fift_stdlib_dir: &Path,
) -> Result<()> {
    let staged_fift_stdlib_dir = unpacked_dir.join(FIFT_STDLIB_ARCHIVE_DIR);
    if !staged_fift_stdlib_dir.is_dir() {
        bail!("`{TON_STDLIB_ASSET_NAME}` did not contain `{FIFT_STDLIB_ARCHIVE_DIR}/`");
    }

    if fift_stdlib_dir.exists() {
        fs::remove_dir_all(fift_stdlib_dir)
            .with_context(|| format!("failed to remove `{}`", fift_stdlib_dir.display()))?;
    }

    fs::create_dir_all(fift_stdlib_dir)
        .with_context(|| format!("failed to create `{}`", fift_stdlib_dir.display()))?;

    for file_name in FIFT_STDLIB_FILES {
        let source_path = staged_fift_stdlib_dir.join(file_name);
        if !source_path.is_file() {
            bail!(
                "`{}` did not contain expected file `{}`",
                staged_fift_stdlib_dir.display(),
                source_path.display()
            );
        }

        let destination_path = fift_stdlib_dir.join(file_name);
        fs::copy(&source_path, &destination_path).with_context(|| {
            format!(
                "failed to copy `{}` to `{}`",
                source_path.display(),
                destination_path.display()
            )
        })?;
    }

    Ok(())
}

fn copy_directory(source_dir: &Path, destination_dir: &Path) -> Result<()> {
    fs::create_dir_all(destination_dir)
        .with_context(|| format!("failed to create `{}`", destination_dir.display()))?;

    for entry in fs::read_dir(source_dir)
        .with_context(|| format!("failed to read `{}`", source_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", source_dir.display()))?;
        let source_path = entry.path();
        let destination_path = destination_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read type of `{}`", source_path.display()))?;

        if file_type.is_dir() {
            copy_directory(&source_path, &destination_path)?;
            continue;
        }

        if !file_type.is_file() {
            bail!("unsupported entry `{}`", source_path.display());
        }

        fs::copy(&source_path, &destination_path).with_context(|| {
            format!(
                "failed to copy `{}` to `{}`",
                source_path.display(),
                destination_path.display()
            )
        })?;
    }

    Ok(())
}
