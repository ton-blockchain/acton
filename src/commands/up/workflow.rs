use anyhow::{Context, Result, bail};
use flate2::read::GzDecoder;
use owo_colors::OwoColorize;
use semver::Version;
use std::env;
use std::fs::{self, File};
use std::path::Path;
use tar::Archive;

use super::client::{Asset, Release, ReleaseClient};

pub fn run_update<C: ReleaseClient>(
    client: &C,
    current_exe: &Path,
    current_version_str: &str,
    version: Option<String>,
    canary: bool,
    stable: bool,
) -> Result<()> {
    check_homebrew(current_exe)?;

    let current_version = Version::parse(current_version_str);

    let release = client.get_release(version.as_deref(), canary)?;

    let should_install = if version.is_some() || canary {
        // in case of explicit version we always use it despite be canary or not
        if canary {
            println!("Installing canary release...");
        } else {
            println!("Installing version {}...", release.tag_name);
        }
        true
    } else if stable {
        let clean_tag = release.tag_name.trim_start_matches('v');
        match Version::parse(clean_tag) {
            Ok(target_version) => {
                if current_version_str == "canary" {
                    // when we use canary and user provide `--stable`, update to latest stable version
                    println!("Installing stable version {target_version} (current: canary)...");
                    true
                } else if let Ok(current_version) = &current_version
                    && &target_version != current_version
                {
                    // if we on stable release, install new stable version
                    println!(
                        "Installing stable version {} (current: {})...",
                        target_version, current_version
                    );
                    true
                } else {
                    println!(
                        "Acton is already at the latest stable version ({}).",
                        current_version?
                    );
                    false
                }
            }
            Err(_) => {
                println!(
                    "Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                    release.tag_name
                );
                return Ok(());
            }
        }
    } else if current_version_str == "canary" {
        println!("Installing latest canary release...");
        true
    } else {
        let current_version = current_version.context("Cannot parse current version")?;
        let clean_tag = release.tag_name.trim_start_matches('v');
        match Version::parse(clean_tag) {
            Ok(target_version) => {
                if target_version > current_version {
                    println!(
                        "New version available: {} (current: {})",
                        target_version.green(),
                        current_version
                    );
                    true
                } else if target_version == current_version {
                    // If versions match, we're up to date
                    println!("Acton is up to date (version {}).", current_version);
                    return Ok(());
                } else {
                    println!("Acton is up to date (version {}).", current_version);
                    return Ok(());
                }
            }
            Err(_) => {
                println!(
                    "Latest release tag '{}' is not a valid semver. Skipping auto-update.",
                    release.tag_name
                );
                return Ok(());
            }
        }
    };

    if !should_install {
        return Ok(());
    }

    let asset = find_asset(&release)?;
    println!("Found asset: {}", asset.name);

    let tarball_path = client.download_asset(asset)?;

    install_binary(&tarball_path, current_exe, current_version_str)?;

    println!(
        "{} updated successfully to {}",
        "Acton".green(),
        release.tag_name
    );

    Ok(())
}

fn check_homebrew(exe: &Path) -> Result<()> {
    let path_str = exe.to_string_lossy();
    if path_str.contains("Cellar") || path_str.contains("homebrew") {
        eprintln!(
            "{}",
            "Warning: Acton seems to be installed via Homebrew.".yellow()
        );
        eprintln!("It is recommended to update using `brew upgrade acton`.");

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
        _ => bail!("Unsupported OS: {}", os),
    };

    let target_arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "arm64",
        _ => bail!("Unsupported architecture: {}", arch),
    };

    release
        .assets
        .iter()
        .find(|a| {
            let name = a.name.to_lowercase();
            name.contains(target_os) && name.contains(target_arch) && name.ends_with(".tar.gz")
        })
        .ok_or_else(|| anyhow::anyhow!("No matching asset found for {}/{}", target_os, target_arch))
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

    let backup_name = format!("acton-{}", current_version);
    let backup_path = bin_dir.join(&backup_name);

    if let Err(e) = fs::rename(current_exe, &backup_path) {
        eprintln!(
            "Warning: Failed to create backup at {}: {}",
            backup_path.display(),
            e
        );
        fs::copy(current_exe, &backup_path).context("Failed to copy current binary to backup")?;
    }

    fs::copy(&new_bin_path, current_exe).context("Failed to install new binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(current_exe, perms)?;
    }

    let _ = fs::remove_file(tarball_path);

    Ok(())
}
