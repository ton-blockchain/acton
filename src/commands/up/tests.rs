use super::client::{Asset, Release, ReleaseClient};
use crate::commands::up::workflow;
use anyhow::{Result, bail};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[test]
fn test_update_stable_to_stable_upgrade() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("0.2.0");
    client.set_latest("0.2.0", vec![asset]);

    workflow::run_update(&client, &bin_path, current_version, None, false, true)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    Ok(())
}

#[test]
fn test_update_stable_already_latest() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.2.0";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("0.2.0");
    client.set_latest("0.2.0", vec![asset]);

    workflow::run_update(&client, &bin_path, current_version, None, false, true)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "old_binary");

    Ok(())
}

#[test]
fn test_update_stable_from_canary() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "canary";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("0.2.0");
    client.set_latest("0.2.0", vec![asset]);

    workflow::run_update(&client, &bin_path, current_version, None, false, true)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_update_canary_to_canary() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "canary";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("canary");
    client.set_canary(vec![asset]);

    workflow::run_update(&client, &bin_path, current_version, None, true, false)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-canary");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_downgrade() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.3.0";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("0.2.0");
    client.add_release("0.2.0", vec![asset]);

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        Some("0.2.0".to_owned()),
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_network_error() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.should_fail = true;

    let result = workflow::run_update(&client, &bin_path, current_version, None, false, true);

    assert!(result.is_err());
    assert_eq!(
        result.expect_err("checked").to_string(),
        "Mock network failure"
    );
    Ok(())
}

#[test]
fn test_custom_version() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("0.0.5");
    client.add_release("0.0.5", vec![asset]);

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        Some("0.0.5".to_string()),
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.0.5");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_install_canary_version_and_then_stable() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.add_release("0.2.0", vec![MockReleaseClient::create_asset("0.2.0")]);
    client.set_latest("0.3.0", vec![MockReleaseClient::create_asset("0.3.0")]);
    client.set_canary(vec![MockReleaseClient::create_asset("canary")]);

    workflow::run_update(&client, &bin_path, current_version, None, true, false)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-canary");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    let current_version = "canary";
    workflow::run_update(&client, &bin_path, current_version, None, false, true)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.3.0");

    assert_backup_created(&bin_path, current_version, "binary-data-canary")?;

    Ok(())
}

#[test]
fn test_install_versions() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", vec![MockReleaseClient::create_asset("0.2.0")]);

    workflow::run_update(&client, &bin_path, current_version, None, false, false)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    // emulate new version
    client.set_latest("0.3.0", vec![MockReleaseClient::create_asset("0.3.0")]);

    let current_version = "0.2.0";
    workflow::run_update(&client, &bin_path, current_version, None, false, false)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.3.0");

    assert_backup_created(&bin_path, current_version, "binary-data-0.2.0")?;

    Ok(())
}

#[test]
fn test_install_canary_versions() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_canary(vec![MockReleaseClient::create_asset_with_content(
        "canary", "canary-1",
    )]);

    workflow::run_update(&client, &bin_path, current_version, None, true, false)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-canary: canary-1");

    // emulate new canary version
    client.set_canary(vec![MockReleaseClient::create_asset_with_content(
        "canary", "canary-2",
    )]);

    let current_version = "canary";
    workflow::run_update(&client, &bin_path, current_version, None, true, false)?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-canary: canary-2");

    assert_backup_created(&bin_path, current_version, "binary-data-canary: canary-1")?;

    Ok(())
}

#[test]
fn test_backup_is_created_correctly() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let asset = MockReleaseClient::create_asset("0.2.0");
    client.set_latest("0.2.0", vec![asset]);

    workflow::run_update(&client, &bin_path, current_version, None, false, true)?;

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

fn assert_backup_created(bin_path: &Path, version: &str, expected_content: &str) -> Result<()> {
    let bin_dir = bin_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent dir"))?;
    let backup_name = format!("acton-{}", version);
    let backup_path = bin_dir.join(&backup_name);

    if !backup_path.exists() {
        bail!("Backup file {} does not exist", backup_path.display());
    }

    let content = fs::read_to_string(&backup_path)?;
    if content != expected_content {
        bail!(
            "Backup content mismatch. Expected '{}', got '{}'",
            expected_content,
            content
        );
    }

    Ok(())
}

struct MockReleaseClient {
    releases: HashMap<String, Release>,
    canary_release: Option<Release>,
    latest_release: Option<Release>,
    should_fail: bool,
}

impl MockReleaseClient {
    fn new() -> Self {
        Self {
            releases: HashMap::new(),
            canary_release: None,
            latest_release: None,
            should_fail: false,
        }
    }

    fn add_release(&mut self, version: &str, assets: Vec<Asset>) {
        let tag = format!("v{}", version);
        let release = Release {
            tag_name: tag.clone(),
            assets,
        };
        self.releases.insert(version.to_string(), release.clone());
        self.releases.insert(tag, release);
    }

    fn set_latest(&mut self, version: &str, assets: Vec<Asset>) {
        let tag = format!("v{}", version);
        let release = Release {
            tag_name: tag.clone(),
            assets,
        };
        self.latest_release = Some(release);
    }

    fn set_canary(&mut self, assets: Vec<Asset>) {
        let release = Release {
            tag_name: "canary".to_string(),
            assets,
        };
        self.canary_release = Some(release);
    }

    fn create_asset(version: &str) -> Asset {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            _ => "unknown",
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "arm64",
            _ => "unknown",
        };

        Asset {
            name: format!("acton-{}-{}.tar.gz", os, arch),
            version: version.to_owned(),
            content: None,
            browser_download_url: format!("http://mock.url/v{}/acton.tar.gz", version),
            size: 1024,
        }
    }

    fn create_asset_with_content(version: &str, content: &str) -> Asset {
        let os = match std::env::consts::OS {
            "macos" => "darwin",
            "linux" => "linux",
            _ => "unknown",
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "arm64",
            _ => "unknown",
        };

        Asset {
            name: format!("acton-{}-{}.tar.gz", os, arch),
            version: version.to_owned(),
            content: Some(content.to_owned()),
            browser_download_url: format!("http://mock.url/v{}/acton.tar.gz", version),
            size: 1024,
        }
    }
}

impl ReleaseClient for MockReleaseClient {
    fn get_release(&self, version: Option<&str>, canary: bool) -> Result<Release> {
        if self.should_fail {
            bail!("Mock network failure");
        }

        if canary {
            return self
                .canary_release
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No canary release found"));
        }

        if let Some(v) = version {
            if let Some(release) = self.releases.get(v) {
                return Ok(release.clone());
            }
            let alt = if v.starts_with('v') {
                v.trim_start_matches('v').to_string()
            } else {
                format!("v{}", v)
            };
            if let Some(release) = self.releases.get(&alt) {
                return Ok(release.clone());
            }
            bail!("Release not found");
        }

        self.latest_release
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No latest release found"))
    }

    fn download_asset(&self, asset: &Asset) -> Result<PathBuf> {
        if self.should_fail {
            bail!("Mock download failure");
        }

        let temp_file = tempfile::NamedTempFile::new()?;
        let path = temp_file.path().to_owned();
        temp_file.keep()?;

        let file = File::create(&path)?;
        let enc = GzEncoder::new(file, Compression::default());
        let mut tar = tar::Builder::new(enc);

        let data = format!(
            "binary-data-{}{}",
            asset.version,
            asset
                .content
                .as_ref()
                .map(|c| format!(": {}", c))
                .unwrap_or("".to_owned())
        );

        let mut header = tar::Header::new_gnu();
        header.set_path("acton")?;
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();

        tar.append(&header, data.as_bytes())?;
        tar.finish()?;

        Ok(path)
    }
}

fn setup_env() -> Result<(tempfile::TempDir, PathBuf)> {
    let dir = tempfile::tempdir()?;
    let bin_path = dir.path().join("acton");

    let mut file = File::create(&bin_path)?;
    file.write_all(b"old_binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&bin_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin_path, perms)?;
    }

    Ok((dir, bin_path))
}
