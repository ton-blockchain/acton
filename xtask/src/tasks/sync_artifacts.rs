use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::str;

use crate::modules::github::Github;
use anyhow::{Context, Result, bail};
use flate2::bufread::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use tar::Archive;
use tempfile::Builder as TempfileBuilder;

const RELEASE_OBJS_RELEASE_TAG: &str = "release-objs";
const TON_OBJS_DIR: &str = "objs";
const TON_ARTIFACTS_LOCK_NAME: &str = "ton-artifacts.lock";

const ARTIFACTS_MANIFEST_ASSET_NAME: &str = "artifacts_manifest.toml";
const TON_OBJS_ARTIFACTS_MANIFEST_PATH: &str = "crates/ton-objs/artifacts_manifest.toml";

const TON_STDLIB_ASSET_NAME: &str = "ton-stdlib.tar.gz";

const TOLK_STDLIB_ARCHIVE_DIR: &str = "tolk-stdlib";
const TOLK_STDLIB_DIR: &str = "crates/tolk-compiler/assets/tolk-stdlib";

const FIFT_STDLIB_ARCHIVE_DIR: &str = "fift-stdlib";
const FIFT_STDLIB_DIR: &str = "crates/tolk-compiler/assets/fift-stdlib";
const FIFT_STDLIB_FILES: &[&str] = &["Asm.fif", "Fift.fif"];

pub(crate) fn run() -> Result<()> {
    println!(
        "Syncing `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}` from GitHub release `{RELEASE_OBJS_RELEASE_TAG}`"
    );

    let objs_dir = Path::new(TON_OBJS_DIR);
    let github = Github::new();
    let downloaded_asset =
        github.download_release_asset(RELEASE_OBJS_RELEASE_TAG, ARTIFACTS_MANIFEST_ASSET_NAME)?;

    let manifest_contents = str::from_utf8(&downloaded_asset)
        .context("downloaded artifacts manifest is not valid UTF-8")?;

    write_manifest(
        Path::new(TON_OBJS_ARTIFACTS_MANIFEST_PATH),
        manifest_contents,
    )?;
    println!("Synchronized `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}`");
    let rust_target = current_release_target()?;
    refresh_local_artifacts_from_release(github, objs_dir, &rust_target)?;
    warn_if_lock_does_not_match_manifest(objs_dir, &rust_target);

    Ok(())
}

fn write_manifest(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("failed to write `{}`", path.display()))?;
    Ok(())
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

fn refresh_local_artifacts_from_release(
    github: Github,
    objs_dir: &Path,
    rust_target: &str,
) -> Result<()> {
    let archive_name = release_archive_name(rust_target);

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

#[derive(Deserialize)]
struct TonArtifactsChecksums {
    rust_target: String,
    sha256_libemulator: String,
    sha256_libtolk: String,
}

fn warn_if_lock_does_not_match_manifest(objs_dir: &Path, rust_target: &str) {
    if let Err(error) = ensure_lock_matches_manifest(objs_dir, rust_target) {
        eprintln!(
            "Warning: `{}` does not match `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}`: {error}",
            objs_dir.join(TON_ARTIFACTS_LOCK_NAME).display()
        );
    }
}

fn ensure_lock_matches_manifest(objs_dir: &Path, rust_target: &str) -> Result<()> {
    let lock_path = objs_dir.join(TON_ARTIFACTS_LOCK_NAME);
    let lock_contents = fs::read_to_string(&lock_path)
        .with_context(|| format!("failed to read `{}`", lock_path.display()))?;
    let manifest_contents = fs::read_to_string(TON_OBJS_ARTIFACTS_MANIFEST_PATH)
        .with_context(|| format!("failed to read `{TON_OBJS_ARTIFACTS_MANIFEST_PATH}`"))?;

    ensure_lock_matches_manifest_contents(
        &lock_contents,
        &lock_path,
        &manifest_contents,
        rust_target,
    )
}

fn ensure_lock_matches_manifest_contents(
    lock_contents: &str,
    lock_path: &Path,
    manifest_contents: &str,
    rust_target: &str,
) -> Result<()> {
    let lock_checksums: TonArtifactsChecksums = serde_json::from_str(lock_contents)
        .with_context(|| format!("failed to parse `{}`", lock_path.display()))?;
    let manifest_checksums = parse_manifest_checksums(manifest_contents, rust_target)?;

    let mut mismatches = Vec::new();
    push_mismatch(
        &mut mismatches,
        "rust_target",
        &manifest_checksums.rust_target,
        &lock_checksums.rust_target,
    );
    push_mismatch(
        &mut mismatches,
        "sha256_libemulator",
        &manifest_checksums.sha256_libemulator,
        &lock_checksums.sha256_libemulator,
    );
    push_mismatch(
        &mut mismatches,
        "sha256_libtolk",
        &manifest_checksums.sha256_libtolk,
        &lock_checksums.sha256_libtolk,
    );

    if !mismatches.is_empty() {
        bail!("{}", mismatches.join("; "));
    }

    Ok(())
}

fn push_mismatch(mismatches: &mut Vec<String>, field_name: &str, expected: &str, actual: &str) {
    if expected != actual {
        mismatches.push(format!(
            "`{field_name}` expected `{expected}`, got `{actual}`"
        ));
    }
}

fn parse_manifest_checksums(
    manifest_contents: &str,
    rust_target: &str,
) -> Result<TonArtifactsChecksums> {
    let document = manifest_contents
        .parse::<toml_edit::DocumentMut>()
        .context("failed to parse downloaded artifacts manifest")?;

    let mut item = document.as_item();
    for key in ["target", rust_target, "sha256"] {
        item = item.get(key).with_context(|| {
            format!("missing table `target.{rust_target}.sha256` in downloaded artifacts manifest")
        })?;
    }

    Ok(TonArtifactsChecksums {
        rust_target: rust_target.to_owned(),
        sha256_libemulator: read_manifest_sha256(item, "libemulator", rust_target)?,
        sha256_libtolk: read_manifest_sha256(item, "libtolk", rust_target)?,
    })
}

fn read_manifest_sha256(
    sha256_item: &toml_edit::Item,
    field_name: &str,
    rust_target: &str,
) -> Result<String> {
    sha256_item
        .get(field_name)
        .and_then(toml_edit::Item::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .with_context(|| {
            format!(
                "missing string `target.{rust_target}.sha256.{field_name}` in downloaded artifacts manifest"
            )
        })
}

fn refresh_local_objs_from_release(
    github: Github,
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
    github: Github,
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
