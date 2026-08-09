//! Downloads, verifies, and extracts one pinned TON release archive.
//!
//! Installation writes into the network cache. The archive is streamed to a
//! temporary `.part` file, checked against the platform SHA-256 digest, safely
//! extracted without accepting paths outside the destination, and marked
//! complete only after every step succeeds.

use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tracing::info;
use zip::ZipArchive;

use crate::storage::{Layout, TON_RELEASE};

use super::{
    REQUIRED_BINARIES,
    release::{current_asset, platform_id},
};

pub(super) async fn install_pinned_release(layout: &Layout) -> Result<PathBuf> {
    let asset = current_asset()?;
    let platform = platform_id();
    let install_dir = layout.cache.join(TON_RELEASE).join(&platform);
    let marker = install_dir.join(".complete");
    if marker.is_file() {
        return Ok(install_dir);
    }

    fs::create_dir_all(&install_dir)?;
    let archive_path = layout
        .cache
        .join(format!("{TON_RELEASE}-{platform}.zip.part"));
    let url = format!(
        "https://github.com/ton-blockchain/ton/releases/download/{TON_RELEASE}/{}",
        asset.file_name
    );
    info!("downloading official TON {TON_RELEASE} binaries for {platform}");
    download(&url, &archive_path).await?;

    let actual_hash = sha256_file(&archive_path)?;
    if actual_hash != asset.sha256 {
        let _ = fs::remove_file(&archive_path);
        bail!(
            "TON archive checksum mismatch: expected {}, got {}",
            asset.sha256,
            actual_hash
        );
    }

    let archive = archive_path.clone();
    let destination = install_dir.clone();
    tokio::task::spawn_blocking(move || extract_zip(&archive, &destination))
        .await
        .context("TON archive extraction task failed")??;
    fs::remove_file(&archive_path)
        .with_context(|| format!("failed to remove {}", archive_path.display()))?;
    fs::write(&marker, format!("{TON_RELEASE}\n{}\n", asset.sha256))?;
    Ok(install_dir)
}

async fn download(url: &str, path: &Path) -> Result<()> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("download failed for {url}"))?;
    let mut file = tokio::fs::File::create(path)
        .await
        .with_context(|| format!("failed to create {}", path.display()))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    file.flush().await?;
    Ok(())
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
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
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
