//! Downloads, verifies, and extracts one pinned TON release archive.
//!
//! Installation writes into a shared per-user cache. The archive is streamed to
//! a temporary `.part` file, checked against the platform SHA-256 digest, safely
//! extracted without accepting paths outside a staging directory, and published
//! atomically only after every step succeeds.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use fs2::FileExt;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tracing::info;
use zip::ZipArchive;

use crate::{cache, storage::TON_RELEASE};

use super::{
    REQUIRED_BINARIES,
    release::{current_asset, platform_id},
};

/// Resolves the pinned release from the shared cache or installs it once.
///
/// A release is reusable only when its marker matches the expected checksum and
/// every required binary and resource directory is present. Installation is
/// serialized per release and platform, then published from a staging directory.
pub(super) async fn install_pinned_release() -> Result<PathBuf> {
    let asset = current_asset()?;
    let platform = platform_id();

    let release_dir = cache::root()?.join("ton").join(TON_RELEASE);
    let install_dir = release_dir.join(&platform);

    // The common case must not create or lock anything after the first install.
    if installation_is_complete(&install_dir, asset.sha256) {
        info!(cache = %install_dir.display(), "using cached official TON {TON_RELEASE} binaries");
        return Ok(install_dir);
    }

    fs::create_dir_all(&release_dir)
        .with_context(|| format!("failed to create {}", release_dir.display()))?;

    let lock_path = release_dir.join(format!("{platform}.lock"));
    let _install_lock = acquire_install_lock(&lock_path).await?;

    // Another process can finish the same installation while this one waits.
    if installation_is_complete(&install_dir, asset.sha256) {
        info!(cache = %install_dir.display(), "using cached official TON {TON_RELEASE} binaries");
        return Ok(install_dir);
    }

    // Stable transient names let the next lock holder recover after interruption.
    let archive_path = release_dir.join(format!(".{platform}.zip.part"));
    let staging_dir = release_dir.join(format!(".{platform}.installing"));
    remove_path_if_exists(&archive_path)?;
    remove_path_if_exists(&staging_dir)?;

    let url = format!(
        "https://github.com/ton-blockchain/ton/releases/download/{TON_RELEASE}/{}",
        asset.file_name
    );
    info!("downloading official TON {TON_RELEASE} binaries for {platform}");

    let install_result: Result<()> = async {
        download(&url, &archive_path).await?;

        let actual_hash = sha256_file(&archive_path)?;
        if actual_hash != asset.sha256 {
            bail!(
                "TON archive checksum mismatch: expected {}, got {}",
                asset.sha256,
                actual_hash
            );
        }

        // ZIP extraction is blocking and must never occupy an async runtime worker.
        let archive = archive_path.clone();
        let destination = staging_dir.clone();
        tokio::task::spawn_blocking(move || extract_zip(&archive, &destination))
            .await
            .context("TON archive extraction task failed")??;

        validate_installation_contents(&staging_dir)?;

        // The marker is the commit record. It becomes visible together with the
        // validated directory when the staging rename succeeds.
        fs::write(staging_dir.join(".complete"), marker_contents(asset.sha256))?;

        remove_path_if_exists(&install_dir)?;
        fs::rename(&staging_dir, &install_dir).with_context(|| {
            format!(
                "failed to publish TON installation {}",
                install_dir.display()
            )
        })?;
        Ok(())
    }
    .await;

    // Best-effort cleanup preserves the original installation error for diagnosis.
    let _ = fs::remove_file(&archive_path);
    let _ = remove_path_if_exists(&staging_dir);

    install_result?;
    Ok(install_dir)
}

async fn acquire_install_lock(path: &Path) -> Result<File> {
    let path = path.to_owned();

    // File locking can wait indefinitely, so keep it outside async worker threads.
    tokio::task::spawn_blocking(move || {
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("failed to open TON cache lock {}", path.display()))?;
        FileExt::lock_exclusive(&file)
            .with_context(|| format!("failed to lock TON cache {}", path.display()))?;
        Ok(file)
    })
    .await
    .context("TON cache lock task failed")?
}

fn marker_contents(sha256: &str) -> String {
    format!("{TON_RELEASE}\n{sha256}\n")
}

fn installation_is_complete(path: &Path, sha256: &str) -> bool {
    let marker_matches = fs::read_to_string(path.join(".complete"))
        .is_ok_and(|marker| marker == marker_contents(sha256));

    marker_matches && validate_installation_contents(path).is_ok()
}

fn validate_installation_contents(path: &Path) -> Result<()> {
    for name in REQUIRED_BINARIES {
        let binary = path.join(name);
        if !binary.is_file() {
            bail!("required TON binary is missing: {}", binary.display());
        }
    }

    for directory in [path.join("lib"), path.join("smartcont")] {
        if !directory.is_dir() {
            bail!(
                "required TON resources are missing: {}",
                directory.display()
            );
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    // Inspect the link itself so cleanup cannot follow a stale symlink elsewhere.
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("failed to remove incomplete cache path {}", path.display()))
}

async fn download(url: &str, path: &Path) -> Result<()> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?;

    let progress = download_progress(response.content_length())?;

    let mut file = tokio::fs::File::create(path)
        .await
        .with_context(|| format!("failed to create {}", path.display()))?;
    let mut stream = response.bytes_stream();

    let result = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("TON binary download stream failed")?;
            file.write_all(&chunk).await?;
            progress.inc(chunk.len() as u64);
        }

        // Complete the file before synchronous checksum validation reads it.
        file.flush().await?;
        file.sync_all().await?;
        Ok(())
    }
    .await;

    match &result {
        Ok(()) => progress.finish_and_clear(),
        Err(_) => progress.abandon_with_message("TON binary download failed"),
    }

    result
}

fn download_progress(total: Option<u64>) -> Result<ProgressBar> {
    let progress = if let Some(total) = total {
        let progress = ProgressBar::new(total);
        progress.set_style(
            ProgressStyle::with_template(
                " {prefix:.green} [{bar:40.}] {percent:>3}% \
                 {bytes}/{total_bytes} {bytes_per_sec} ETA {eta}",
            )?
            .progress_chars("=>-"),
        );
        progress
    } else {
        let progress = ProgressBar::new_spinner();
        progress.set_style(ProgressStyle::with_template(
            " {prefix:.green} {spinner} {bytes} {bytes_per_sec} elapsed {elapsed_precise}",
        )?);
        progress
    };
    progress.set_prefix("Downloading");
    progress.enable_steady_tick(Duration::from_millis(100));
    Ok(progress)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn extract_zip(archive_path: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;

    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;

        // Reject absolute paths and `..` components before joining the target.
        let relative = entry
            .enclosed_name()
            .context("TON archive contains an unsafe path")?
            .to_owned();
        let target = destination.join(relative);

        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut output = File::create(&target)?;
        io::copy(&mut entry, &mut output)?;

        #[cfg(unix)]
        if REQUIRED_BINARIES
            .iter()
            .any(|name| target.file_name().is_some_and(|file| file == *name))
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_installation_requires_matching_marker_and_resources() {
        let root = tempfile::tempdir_in("/tmp").unwrap();
        for name in REQUIRED_BINARIES {
            File::create(root.path().join(name)).unwrap();
        }
        fs::create_dir(root.path().join("lib")).unwrap();
        fs::create_dir(root.path().join("smartcont")).unwrap();
        fs::write(root.path().join(".complete"), marker_contents("digest")).unwrap();

        assert!(installation_is_complete(root.path(), "digest"));
        assert!(!installation_is_complete(root.path(), "other-digest"));
    }

    #[test]
    fn download_progress_supports_known_and_unknown_sizes() {
        for total in [Some(1_024), None] {
            let progress = download_progress(total).unwrap();
            progress.inc(512);
            progress.finish_and_clear();
        }
    }
}
