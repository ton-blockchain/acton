use acton_config::color::OwoColorize;
use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use semver::Version;
use std::env;
use std::fs::{self, File};
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
) -> Result<UpdateInfo> {
    let release = client.get_release(None, false)?;
    let latest_version = release.tag_name;
    let is_canary = current_version_str == "canary";

    let update_available = if is_canary {
        // don't report anything for canary release user
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

pub(super) fn run_update<C: ReleaseClient>(
    client: &C,
    current_exe: &Path,
    current_version_str: &str,
    version: Option<String>,
    canary: bool,
    stable: bool,
    yes: bool,
) -> Result<()> {
    check_homebrew(current_exe, yes)?;

    let current_version = Version::parse(current_version_str);

    let release = client.get_release(version.as_deref(), canary)?;

    let should_install = if version.is_some() || canary {
        // in case of explicit version we always use it despite be canary or not
        if canary {
            println!("  {} canary release", "Installing".green().bold());
        } else {
            println!("  {} {}", "Installing".green().bold(), release.tag_name);
        }
        true
    } else if stable {
        let clean_tag = release.tag_name.trim_start_matches('v');
        if let Ok(target_version) = Version::parse(clean_tag) {
            if current_version_str == "canary" {
                // when we use canary and user provide `--stable`, update to latest stable version
                println!(
                    "   {} stable version {} (current: canary)",
                    "Installing".green().bold(),
                    target_version
                );
                true
            } else if let Ok(current_version) = &current_version
                && &target_version != current_version
            {
                // if we on stable release, install new stable version
                println!(
                    "   {} stable version {} (current: {})",
                    "Installing".green().bold(),
                    target_version,
                    current_version
                );
                true
            } else {
                println!(
                    "   {} Acton is already at the latest stable version ({})",
                    "Up to date".green().bold(),
                    current_version?
                );
                false
            }
        } else {
            println!(
                "   {} Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                "Skipping".yellow().bold(),
                release.tag_name
            );
            return Ok(());
        }
    } else if current_version_str == "canary" {
        println!("   {} latest canary release", "Installing".green().bold());
        true
    } else {
        let current_version = current_version.context("Cannot parse current version")?;
        let clean_tag = release.tag_name.trim_start_matches('v');
        if let Ok(target_version) = Version::parse(clean_tag) {
            if target_version > current_version {
                println!(
                    "   {} version {} (current: {})",
                    "Updating".green().bold(),
                    target_version,
                    current_version
                );
                true
            } else if target_version == current_version {
                // If versions match, we're up to date
                println!(
                    "   {} Acton is up to date (version {})",
                    "Up to date".green().bold(),
                    current_version
                );
                return Ok(());
            } else {
                println!(
                    "   {} Acton is up to date (version {})",
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
    let tarball_path = client.download_asset(asset)?;

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
    let os = env::consts::OS;
    let arch = env::consts::ARCH;

    let target_os = match os {
        "macos" => "darwin",
        "linux" => "linux",
        _ => bail!("Unsupported OS: {os}"),
    };

    let target_arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => bail!("Unsupported architecture: {arch}"),
    };

    release
        .assets
        .iter()
        .find(|a| {
            let name = a.name.to_lowercase();
            name.contains(target_os) && name.contains(target_arch) && name.ends_with(".tar.gz")
        })
        .ok_or_else(|| anyhow::anyhow!("No matching asset found for {target_os}/{target_arch}"))
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
    if let Err(e) = fs::copy(current_exe, &backup_path) {
        eprintln!(
            "Warning: Failed to create backup at {}: {}",
            backup_path.display(),
            e
        );
    }

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
