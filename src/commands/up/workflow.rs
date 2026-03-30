use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use semver::Version;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use tar::Archive;

use super::client::{Asset, Release, ReleaseClient};

#[derive(serde::Serialize)]
pub(super) struct UpdateInfo {
    pub success: bool,
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

pub(super) fn check_update<C: ReleaseClient>(
    client: &C,
    current_version_str: &str,
    current_is_trunk: bool,
) -> Result<UpdateInfo> {
    let release = client.get_release(None, false)?;
    let latest_version = release.tag_name;

    let update_available = if current_is_trunk {
        // don't report anything for trunk release user
        false
    } else {
        let current_v =
            Version::parse(current_version_str).context("Cannot parse current version")?;
        let latest_v_str = latest_version.trim_start_matches('v');
        if let Ok(latest_v) = Version::parse(latest_v_str) {
            latest_v > current_v
        } else {
            false
        }
    };

    Ok(UpdateInfo {
        success: true,
        current_version: current_version_str.to_string(),
        latest_version,
        update_available,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_update<C: ReleaseClient>(
    client: &C,
    current_exe: &Path,
    current_version_str: &str,
    current_is_trunk: bool,
    version: Option<String>,
    trunk: bool,
    stable: bool,
    yes: bool,
) -> Result<()> {
    check_homebrew(current_exe, yes)?;

    let current_version = Version::parse(current_version_str);
    let use_trunk_release = version.is_none() && !stable && (trunk || current_is_trunk);

    let release = client.get_release(version.as_deref(), use_trunk_release)?;

    let should_install = if version.is_some() || use_trunk_release {
        // An explicit version always wins; otherwise stay on the active trunk channel.
        if version.is_none() && use_trunk_release {
            println!("  {} trunk release", "Installing".green().bold());
        } else {
            println!("  {} {}", "Installing".green().bold(), release.tag_name);
        }
        true
    } else if stable {
        let clean_tag = release.tag_name.trim_start_matches('v');
        if let Ok(target_version) = Version::parse(clean_tag) {
            if current_is_trunk {
                // when we are on a trunk build and user provide `--stable`, update to latest stable version
                println!(
                    "  {} stable version {} (current: trunk)",
                    "Installing".green().bold(),
                    target_version
                );
                true
            } else if let Ok(current_version) = &current_version
                && &target_version != current_version
            {
                // if we on stable release, install new stable version
                println!(
                    "  {} stable version {} (current: {})",
                    "Installing".green().bold(),
                    target_version,
                    current_version
                );
                true
            } else {
                println!(
                    "  {} Acton is already at the latest stable version ({})",
                    "Up to date".green().bold(),
                    current_version?
                );
                false
            }
        } else {
            println!(
                "    {} Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                "Skipping".yellow().bold(),
                release.tag_name
            );
            return Ok(());
        }
    } else {
        let current_version = current_version.context("Cannot parse current version")?;
        let clean_tag = release.tag_name.trim_start_matches('v');
        if let Ok(target_version) = Version::parse(clean_tag) {
            if target_version > current_version {
                println!(
                    "    {} version {} (current: {})",
                    "Updating".green().bold(),
                    target_version,
                    current_version
                );
                true
            } else if target_version == current_version {
                // If versions match, we're up to date
                println!(
                    "  {} Acton is up to date (version {})",
                    "Up to date".green().bold(),
                    current_version
                );
                return Ok(());
            } else {
                println!(
                    "  {} Acton is up to date (version {})",
                    "Up to date".green().bold(),
                    current_version
                );
                return Ok(());
            }
        } else {
            println!(
                "   {} Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                "Skipping".yellow().bold(),
                release.tag_name
            );
            return Ok(());
        }
    };

    if !should_install {
        return Ok(());
    }

    let asset = find_asset(&release)?;
    let checksum_asset = find_checksum_asset(&release, &asset.name)?;
    let tarball_path = client.download_asset(asset)?;
    let checksum_path = client.download_asset(checksum_asset)?;

    let verify_result = verify_sha256(&tarball_path, &checksum_path, &asset.name);
    let _ = fs::remove_file(&checksum_path);
    verify_result?;

    install_binary(&tarball_path, current_exe, current_version_str)?;

    println!("     {} to {}", "Updated".green().bold(), release.tag_name);

    Ok(())
}

fn check_homebrew(exe: &Path, yes: bool) -> Result<()> {
    let path_str = exe.to_string_lossy();
    if path_str.contains("Cellar") || path_str.contains("homebrew") {
        eprintln!(
            "{}",
            "Warning: Acton seems to be installed via Homebrew.".yellow()
        );
        eprintln!("It is recommended to update using `brew upgrade acton`.");

        if yes {
            return Ok(());
        }

        let ans = inquire::Confirm::new("Do you want to proceed with built-in update anyway?")
            .with_default(false)
            .prompt();

        match ans {
            Ok(true) => Ok(()),
            Ok(false) => bail!("Update cancelled."),
            Err(_) => bail!("Failed to read input."),
        }
    } else {
        Ok(())
    }
}

fn find_asset(release: &Release) -> Result<&Asset> {
    find_asset_for_target_triple(release, env!("TARGET_TRIPLE"))
}

pub(super) fn find_asset_for_target_triple<'a>(
    release: &'a Release,
    target_triple: &str,
) -> Result<&'a Asset> {
    let expected_name = release_asset_name_for_target_triple(target_triple)?;

    release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(&expected_name))
        .ok_or_else(|| anyhow::anyhow!("No matching asset found: expected {expected_name}"))
}

pub(super) fn release_asset_name_for_target_triple(target_triple: &str) -> Result<String> {
    if target_triple.trim().is_empty() {
        bail!("Target triple is empty");
    }

    Ok(format!("acton-{target_triple}.tar.gz"))
}

fn find_checksum_asset<'a>(release: &'a Release, archive_name: &str) -> Result<&'a Asset> {
    let checksum_name = format!("{archive_name}.sha256");

    release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case(&checksum_name))
        .ok_or_else(|| {
            anyhow::anyhow!("No matching checksum asset found: expected {checksum_name}")
        })
}

fn verify_sha256(tarball_path: &Path, checksum_path: &Path, archive_name: &str) -> Result<()> {
    let expected = read_expected_sha256(checksum_path, archive_name)?;
    let actual = compute_sha256(tarball_path)?;

    if actual != expected {
        bail!("SHA256 mismatch for {archive_name}: expected {expected}, got {actual}");
    }

    println!("    {} {archive_name}.sha256", "Verified".green().bold());

    Ok(())
}

fn read_expected_sha256(checksum_path: &Path, archive_name: &str) -> Result<String> {
    let contents = fs::read_to_string(checksum_path)
        .with_context(|| format!("Failed to read checksum file {}", checksum_path.display()))?;

    let line = contents
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Checksum file is empty: {}", checksum_path.display()))?;

    let mut parts = line.split_whitespace();
    let checksum = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Checksum file is invalid: {}", checksum_path.display()))?;

    if checksum.len() != 64 || !checksum.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!(
            "Checksum file has invalid SHA256 digest: {}",
            checksum_path.display()
        );
    }

    if let Some(reported_name) = parts.next() {
        let reported_name = reported_name.trim_start_matches('*');
        if reported_name != archive_name {
            bail!(
                "Checksum file references unexpected asset '{reported_name}' (expected '{archive_name}')"
            );
        }
    }

    Ok(checksum.to_ascii_lowercase())
}

fn compute_sha256(path: &Path) -> Result<String> {
    let mut file =
        File::open(path).with_context(|| format!("Failed to open file {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];

    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn install_binary(tarball_path: &Path, current_exe: &Path, current_version: &str) -> Result<()> {
    let tar_gz = File::open(tarball_path)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);

    let mut temp_bin_path = None;

    let temp_dir = tempfile::tempdir()?;
    archive.unpack(&temp_dir)?;

    for entry in walkdir::WalkDir::new(&temp_dir) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.file_name() == "acton" {
            temp_bin_path = Some(entry.path().to_owned());
            break;
        }
    }

    let new_bin_path = temp_bin_path
        .ok_or_else(|| anyhow::anyhow!("Could not find 'acton' binary in the archive"))?;

    let bin_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Could not determine binary directory"))?;

    let backup_name = format!("acton-{current_version}");
    let backup_path = bin_dir.join(&backup_name);

    // 1. Create backup by copying current binary
    fs::copy(current_exe, &backup_path)
        .with_context(|| format!("Failed to create backup at {}", backup_path.display()))?;

    // 2. Prepare new binary in a temporary file in the same directory to ensure atomic rename
    let temp_file = tempfile::NamedTempFile::new_in(bin_dir)
        .context("Failed to create temporary file for new binary")?;

    fs::copy(&new_bin_path, temp_file.path())
        .context("Failed to copy new binary to temporary file")?;

    // 3. Set permissions on the temporary file
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(temp_file.path())?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(temp_file.path(), perms)?;
    }

    // 4. Atomically replace the current binary
    temp_file
        .persist(current_exe)
        .context("Failed to atomically replace current binary")?;

    let _ = fs::remove_file(tarball_path);

    Ok(())
}
