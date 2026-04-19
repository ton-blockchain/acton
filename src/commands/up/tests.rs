use super::client::{Asset, Release, ReleaseClient};
use crate::commands::up::workflow;
use anyhow::{Result, bail};
use flate2::{Compression, GzBuilder};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[test]
fn test_update_stable_to_stable_upgrade() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    Ok(())
}

#[test]
fn test_update_stable_already_latest() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.2.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "old_binary");

    Ok(())
}

#[test]
fn test_update_stable_from_trunk() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        true,
        None,
        false,
        true,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_update_trunk_to_trunk() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_trunk(MockReleaseClient::create_release_assets("trunk"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        true,
        None,
        true,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-trunk");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_update_current_trunk_without_flags_keeps_trunk_channel() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.8.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.9.0", MockReleaseClient::create_release_assets("0.9.0"));
    client.set_trunk(MockReleaseClient::create_release_assets_with_content(
        "trunk",
        "trunk-default-channel",
    ));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        true,
        None,
        false,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-trunk: trunk-default-channel");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_downgrade() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.3.0";

    let mut client = MockReleaseClient::new();
    client.add_release("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        Some("0.2.0".to_owned()),
        false,
        false,
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

    let result = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    );

    let err = result.expect_err("network error must fail");
    assert_error_snapshot("test_network_error", err.to_string());
    Ok(())
}

#[test]
fn test_custom_version() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.add_release("0.0.5", MockReleaseClient::create_release_assets("0.0.5"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        Some("0.0.5".to_string()),
        false,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.0.5");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_validate_version_argument_rejects_unicode_dash_flag_typo() {
    let err = super::validate_version_argument(Some("\u{2014}trunk"))
        .expect_err("unicode dash flag typo should be rejected");

    assert_error_snapshot(
        "test_validate_version_argument_rejects_unicode_dash_flag_typo",
        err.to_string(),
    );
}

#[test]
fn test_validate_version_argument_allows_regular_version() -> Result<()> {
    super::validate_version_argument(Some("0.1.0"))?;
    Ok(())
}

#[test]
fn test_validate_version_argument_rejects_unicode_dash_force_flag_typo() {
    let err = super::validate_version_argument(Some("\u{2014}force"))
        .expect_err("unicode dash force flag typo should be rejected");

    assert_error_snapshot(
        "test_validate_version_argument_rejects_unicode_dash_force_flag_typo",
        err.to_string(),
    );
}

#[test]
fn test_install_trunk_version_and_then_stable() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.add_release("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));
    client.set_latest("0.3.0", MockReleaseClient::create_release_assets("0.3.0"));
    client.set_trunk(MockReleaseClient::create_release_assets("trunk"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        true,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-trunk");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    let current_version = "0.1.0";
    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        true,
        None,
        false,
        true,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.3.0");

    assert_backup_created(&bin_path, current_version, "binary-data-trunk")?;

    Ok(())
}

#[test]
fn test_install_versions() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    // emulate new version
    client.set_latest("0.3.0", MockReleaseClient::create_release_assets("0.3.0"));

    let current_version = "0.2.0";
    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.3.0");

    assert_backup_created(&bin_path, current_version, "binary-data-0.2.0")?;

    Ok(())
}

#[test]
fn test_install_trunk_versions() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_trunk(MockReleaseClient::create_release_assets_with_content(
        "trunk", "trunk-1",
    ));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        true,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-trunk: trunk-1");

    // emulate new trunk version
    client.set_trunk(MockReleaseClient::create_release_assets_with_content(
        "trunk", "trunk-2",
    ));

    let current_version = "0.1.0";
    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        true,
        None,
        true,
        false,
        false,
        false,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-trunk: trunk-2");

    assert_backup_created(&bin_path, current_version, "binary-data-trunk: trunk-1")?;

    Ok(())
}

#[test]
fn test_backup_is_created_correctly() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )?;
    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_force_reinstalls_latest_release_when_already_up_to_date() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.2.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        false,
        false,
        true,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_force_reinstalls_stable_release_when_explicit_stable_matches_current() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.2.0";

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        true,
    )?;

    let content = fs::read_to_string(&bin_path)?;
    assert_eq!(content, "binary-data-0.2.0");

    assert_backup_created(&bin_path, current_version, "old_binary")?;

    Ok(())
}

#[test]
fn test_find_asset_for_target_triple_distinguishes_gnu_and_musl() -> Result<()> {
    let release = Release {
        tag_name: "v0.2.0".to_owned(),
        assets: vec![
            MockReleaseClient::create_named_asset("0.2.0", "acton-x86_64-unknown-linux-gnu.tar.gz"),
            MockReleaseClient::create_named_asset(
                "0.2.0",
                "acton-x86_64-unknown-linux-musl.tar.gz",
            ),
        ],
    };

    let gnu_asset = workflow::find_asset_for_target_triple(&release, "x86_64-unknown-linux-gnu")?;
    assert_eq!(gnu_asset.name, "acton-x86_64-unknown-linux-gnu.tar.gz");

    let musl_asset = workflow::find_asset_for_target_triple(&release, "x86_64-unknown-linux-musl")?;
    assert_eq!(musl_asset.name, "acton-x86_64-unknown-linux-musl.tar.gz");

    Ok(())
}

#[test]
fn test_update_fails_without_checksum_asset() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    client.set_latest(
        "0.2.0",
        vec![MockReleaseClient::create_named_asset(
            "0.2.0",
            &MockReleaseClient::current_archive_name()?,
        )],
    );

    let err = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )
    .expect_err("missing checksum asset must fail");

    assert_error_snapshot("test_update_fails_without_checksum_asset", err.to_string());

    Ok(())
}

#[test]
fn test_update_fails_on_checksum_mismatch() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let mut assets = MockReleaseClient::create_release_assets("0.2.0");
    assets[1].content = Some("tampered-checksum".to_owned());
    client.set_latest("0.2.0", assets);

    let err = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )
    .expect_err("checksum mismatch must fail");

    assert_error_snapshot("test_update_fails_on_checksum_mismatch", err.to_string());
    assert_eq!(fs::read_to_string(&bin_path)?, "old_binary");

    Ok(())
}

#[test]
fn test_update_fails_when_checksum_file_is_empty() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let archive_name = MockReleaseClient::current_archive_name()?;
    client.set_latest(
        "0.2.0",
        vec![
            MockReleaseClient::create_named_asset("0.2.0", &archive_name),
            MockReleaseClient::create_named_asset_with_raw_bytes(
                "0.2.0",
                &format!("{archive_name}.sha256"),
                Vec::new(),
            ),
        ],
    );

    let err = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )
    .expect_err("empty checksum file must fail");

    assert_error_snapshot(
        "test_update_fails_when_checksum_file_is_empty",
        err.to_string(),
    );
    assert_eq!(fs::read_to_string(&bin_path)?, "old_binary");

    Ok(())
}

#[test]
fn test_update_fails_when_checksum_file_has_invalid_digest() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let archive_name = MockReleaseClient::current_archive_name()?;
    client.set_latest(
        "0.2.0",
        vec![
            MockReleaseClient::create_named_asset("0.2.0", &archive_name),
            MockReleaseClient::create_named_asset_with_raw_bytes(
                "0.2.0",
                &format!("{archive_name}.sha256"),
                b"not-a-sha256-digest  acton.tar.gz\n".to_vec(),
            ),
        ],
    );

    let err = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )
    .expect_err("invalid checksum digest must fail");

    assert_error_snapshot(
        "test_update_fails_when_checksum_file_has_invalid_digest",
        err.to_string(),
    );
    assert_eq!(fs::read_to_string(&bin_path)?, "old_binary");

    Ok(())
}

#[test]
fn test_update_fails_when_release_archive_is_invalid() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let archive_name = MockReleaseClient::current_archive_name()?;
    let archive_bytes = b"not-a-tar-gz".to_vec();
    let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
    client.set_latest(
        "0.2.0",
        vec![
            MockReleaseClient::create_named_asset_with_raw_bytes(
                "0.2.0",
                &archive_name,
                archive_bytes,
            ),
            MockReleaseClient::create_named_asset_with_raw_bytes(
                "0.2.0",
                &format!("{archive_name}.sha256"),
                format!("{checksum}  {archive_name}\n").into_bytes(),
            ),
        ],
    );

    let err = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )
    .expect_err("invalid release archive must fail");

    assert_error_snapshot(
        "test_update_fails_when_release_archive_is_invalid",
        err.to_string(),
    );
    assert_eq!(fs::read_to_string(&bin_path)?, "old_binary");

    Ok(())
}

#[test]
fn test_update_fails_when_archive_has_no_acton_binary() -> Result<()> {
    let (_dir, bin_path) = setup_env()?;
    let current_version = "0.1.0";

    let mut client = MockReleaseClient::new();
    let archive_name = MockReleaseClient::current_archive_name()?;
    let archive_bytes = MockReleaseClient::build_archive_with_single_file("README.txt", "hello")?;
    let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
    client.set_latest(
        "0.2.0",
        vec![
            MockReleaseClient::create_named_asset_with_raw_bytes(
                "0.2.0",
                &archive_name,
                archive_bytes,
            ),
            MockReleaseClient::create_named_asset_with_raw_bytes(
                "0.2.0",
                &format!("{archive_name}.sha256"),
                format!("{checksum}  {archive_name}\n").into_bytes(),
            ),
        ],
    );

    let err = workflow::run_update(
        &client,
        &bin_path,
        current_version,
        false,
        None,
        false,
        true,
        false,
        false,
    )
    .expect_err("archive without acton binary must fail");

    assert_error_snapshot(
        "test_update_fails_when_archive_has_no_acton_binary",
        err.to_string(),
    );
    assert_eq!(fs::read_to_string(&bin_path)?, "old_binary");

    Ok(())
}

#[test]
fn test_homebrew_update_without_yes_fails_non_interactively() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let cellar_dir = dir.path().join("Cellar").join("acton").join("bin");
    fs::create_dir_all(&cellar_dir)?;
    let bin_path = cellar_dir.join("acton");
    fs::write(&bin_path, "old_binary")?;

    let mut client = MockReleaseClient::new();
    client.set_latest("0.2.0", MockReleaseClient::create_release_assets("0.2.0"));

    let err = workflow::run_update(
        &client, &bin_path, "0.1.0", false, None, false, false, false, false,
    )
    .expect_err("homebrew update without --yes must fail in non-interactive tests");

    assert_error_snapshot(
        "test_homebrew_update_without_yes_fails_non_interactively",
        err.to_string(),
    );

    Ok(())
}

fn assert_error_snapshot(snapshot_name: &str, actual: impl AsRef<str>) {
    let snapshot_path = unit_snapshot_path(snapshot_name);
    let normalized = normalize_error_text(actual.as_ref());

    if env::var("SNAPSHOTS").ok().as_deref() == Some("overwrite") {
        fs::create_dir_all(
            snapshot_path
                .parent()
                .expect("unit snapshot must have a parent directory"),
        )
        .expect("failed to create unit snapshot directory");
        fs::write(&snapshot_path, format!("{normalized}\n"))
            .expect("failed to write unit snapshot");
        return;
    }

    let expected = fs::read_to_string(&snapshot_path)
        .unwrap_or_else(|err| panic!("failed to read snapshot {}: {err}", snapshot_path.display()));
    assert_eq!(expected, format!("{normalized}\n"));
}

fn normalize_error_text(text: &str) -> String {
    let stripped = strip_ansi_escapes::strip(text.as_bytes());
    normalize_up_snapshot_text(
        String::from_utf8(stripped)
            .expect("error text should stay utf-8")
            .replace("\r\n", "\n"),
    )
}

fn normalize_up_snapshot_text(text: String) -> String {
    let target_triple = env!("TARGET_TRIPLE");
    let archive_name = format!("acton-{target_triple}.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");

    text.replace(&checksum_name, "[ACTON_ARCHIVE_SHA256]")
        .replace(&archive_name, "[ACTON_ARCHIVE]")
        .replace(target_triple, "[TARGET_TRIPLE]")
}

fn unit_snapshot_path(snapshot_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join("up_unit")
        .join(format!("{snapshot_name}.txt"))
}

fn assert_backup_created(bin_path: &Path, version: &str, expected_content: &str) -> Result<()> {
    let bin_dir = bin_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent dir"))?;
    let backup_name = format!("acton-{version}");
    let backup_path = bin_dir.join(&backup_name);

    if !backup_path.exists() {
        bail!("Backup file {} does not exist", backup_path.display());
    }

    let content = fs::read_to_string(&backup_path)?;
    if content != expected_content {
        bail!("Backup content mismatch. Expected '{expected_content}', got '{content}'");
    }

    Ok(())
}

struct MockReleaseClient {
    releases: HashMap<String, Release>,
    trunk_release: Option<Release>,
    latest_release: Option<Release>,
    should_fail: bool,
}

impl MockReleaseClient {
    fn new() -> Self {
        Self {
            releases: HashMap::new(),
            trunk_release: None,
            latest_release: None,
            should_fail: false,
        }
    }

    fn add_release(&mut self, version: &str, assets: Vec<Asset>) {
        let tag = format!("v{version}");
        let release = Release {
            tag_name: tag.clone(),
            assets,
        };
        self.releases.insert(version.to_string(), release.clone());
        self.releases.insert(tag, release);
    }

    fn set_latest(&mut self, version: &str, assets: Vec<Asset>) {
        let tag = format!("v{version}");
        let release = Release {
            tag_name: tag,
            assets,
        };
        self.latest_release = Some(release);
    }

    fn set_trunk(&mut self, assets: Vec<Asset>) {
        let release = Release {
            tag_name: "trunk".to_string(),
            assets,
        };
        self.trunk_release = Some(release);
    }

    fn current_archive_name() -> Result<String> {
        workflow::release_asset_name_for_target_triple(env!("TARGET_TRIPLE"))
    }

    fn create_named_asset(version: &str, name: &str) -> Asset {
        Asset {
            name: name.to_owned(),
            url: format!("http://api.mock.url/v{version}/{name}"),
            version: version.to_owned(),
            content: None,
            raw_bytes: None,
            browser_download_url: format!("http://mock.url/v{version}/{name}"),
            size: 1024,
        }
    }

    fn create_named_asset_with_content(version: &str, name: &str, content: &str) -> Asset {
        Asset {
            name: name.to_owned(),
            url: format!("http://api.mock.url/v{version}/{name}"),
            version: version.to_owned(),
            content: Some(content.to_owned()),
            raw_bytes: None,
            browser_download_url: format!("http://mock.url/v{version}/{name}"),
            size: 1024,
        }
    }

    fn create_named_asset_with_raw_bytes(version: &str, name: &str, raw_bytes: Vec<u8>) -> Asset {
        Asset {
            name: name.to_owned(),
            url: format!("http://api.mock.url/v{version}/{name}"),
            version: version.to_owned(),
            content: None,
            raw_bytes: Some(raw_bytes),
            browser_download_url: format!("http://mock.url/v{version}/{name}"),
            size: 1024,
        }
    }

    fn create_release_assets(version: &str) -> Vec<Asset> {
        let archive_name =
            Self::current_archive_name().expect("current platform must be supported in tests");
        vec![
            Self::create_named_asset(version, &archive_name),
            Self::create_named_asset(version, &format!("{archive_name}.sha256")),
        ]
    }

    fn create_release_assets_with_content(version: &str, content: &str) -> Vec<Asset> {
        let archive_name =
            Self::current_archive_name().expect("current platform must be supported in tests");
        vec![
            Self::create_named_asset_with_content(version, &archive_name, content),
            Self::create_named_asset_with_content(
                version,
                &format!("{archive_name}.sha256"),
                content,
            ),
        ]
    }

    fn build_archive_bytes(asset: &Asset) -> Result<Vec<u8>> {
        if let Some(raw_bytes) = &asset.raw_bytes {
            return Ok(raw_bytes.clone());
        }

        let encoder = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(encoder);

        let data = format!(
            "binary-data-{}{}",
            asset.version,
            asset
                .content
                .as_ref()
                .map(|c| format!(": {c}"))
                .unwrap_or_default()
        );

        let mut header = tar::Header::new_gnu();
        header.set_path("acton")?;
        header.set_size(data.len() as u64);
        header.set_mode(0o755);
        header.set_mtime(0);
        header.set_cksum();

        tar.append(&header, data.as_bytes())?;

        let encoder = tar.into_inner()?;
        Ok(encoder.finish()?)
    }

    fn build_archive_with_single_file(path: &str, contents: &str) -> Result<Vec<u8>> {
        let encoder = GzBuilder::new()
            .mtime(0)
            .write(Vec::new(), Compression::default());
        let mut tar = tar::Builder::new(encoder);

        let mut header = tar::Header::new_gnu();
        header.set_path(path)?;
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_cksum();
        tar.append(&header, contents.as_bytes())?;

        let encoder = tar.into_inner()?;
        Ok(encoder.finish()?)
    }
}

impl ReleaseClient for MockReleaseClient {
    fn get_release(&self, version: Option<&str>, trunk: bool) -> Result<Release> {
        if self.should_fail {
            bail!("Mock network failure");
        }

        if trunk {
            return self
                .trunk_release
                .clone()
                .ok_or_else(|| anyhow::anyhow!("No trunk release found"));
        }

        if let Some(v) = version {
            if let Some(release) = self.releases.get(v) {
                return Ok(release.clone());
            }
            let alt = if v.starts_with('v') {
                v.trim_start_matches('v').to_string()
            } else {
                format!("v{v}")
            };
            if let Some(release) = self.releases.get(&alt) {
                return Ok(release.clone());
            }
            bail!("Release not found: {v}");
        }

        self.latest_release
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No latest release found"))
    }

    fn list_releases(&self) -> Result<Vec<String>> {
        if self.should_fail {
            bail!("Mock network failure");
        }

        let mut tags: Vec<String> = self.releases.keys().cloned().collect();
        if let Some(latest) = &self.latest_release {
            tags.push(latest.tag_name.clone());
        }
        if let Some(trunk) = &self.trunk_release {
            tags.push(trunk.tag_name.clone());
        }
        tags.sort();
        tags.dedup();
        Ok(tags)
    }

    fn download_asset(&self, asset: &Asset) -> Result<PathBuf> {
        if self.should_fail {
            bail!("Mock download failure");
        }

        let temp_file = tempfile::NamedTempFile::new()?;
        let path = temp_file.path().to_owned();
        temp_file.keep()?;

        if asset.name.ends_with(".sha256") {
            let archive_name = asset.name.trim_end_matches(".sha256");
            if let Some(raw_bytes) = &asset.raw_bytes {
                fs::write(&path, raw_bytes)?;
                return Ok(path);
            }

            let archive_asset = Asset {
                name: archive_name.to_owned(),
                url: asset.url.clone(),
                browser_download_url: asset.browser_download_url.clone(),
                size: asset.size,
                version: asset.version.clone(),
                content: asset.content.clone(),
                raw_bytes: None,
            };
            let archive_bytes = Self::build_archive_bytes(&archive_asset)?;
            let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
            fs::write(&path, format!("{checksum}  {archive_name}\n"))?;
        } else {
            fs::write(&path, Self::build_archive_bytes(asset)?)?;
        }

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
