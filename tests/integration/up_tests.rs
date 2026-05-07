use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use acton::build_info;
use anyhow::Result;
use flate2::{Compression, GzBuilder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const TEST_GITHUB_API_BASE_ENV: &str = "ACTON_TEST_UP_GITHUB_API_BASE";
const TEST_CURRENT_EXE_ENV: &str = "ACTON_TEST_UP_CURRENT_EXE";

#[test]
// Verifies that `acton up --list` lists versions from the Acton release repository.
fn test_up_list_lists_versions_from_release_repository() {
    let project = ProjectBuilder::new("up-list-versions").build();
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases?per_page=100&page=1",
            json!([
                { "tag_name": "v0.4.0", "assets": [] },
                { "tag_name": "v0.3.0", "assets": [] },
                { "tag_name": "trunk", "assets": [] }
            ])
            .to_string(),
        )]
    });

    let output = up_command(&project)
        .arg("--list")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/up/test_up_list_lists_versions_from_release_repository.stdout.txt",
    );
}

#[test]
// Verifies that `acton up` remains available when [toolchain].acton does not match.
fn test_up_list_ignores_project_toolchain_acton_mismatch() {
    let project = ProjectBuilder::new("up-list-toolchain-mismatch").build();
    let config_path = project.path().join("Acton.toml");
    let mut toml_content = fs::read_to_string(&config_path).expect("Read Acton.toml");
    toml_content.push_str(
        r#"
[toolchain]
acton = "0.0.0"
"#,
    );
    fs::write(config_path, toml_content).expect("Write Acton.toml");

    let mock = GitHubMockServer::spawn_with(|_| {
        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases?per_page=100&page=1",
            json!([{ "tag_name": "v0.4.0", "assets": [] }]).to_string(),
        )]
    });

    let output = up_command(&project)
        .arg("--list")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/up/test_up_list_ignores_project_toolchain_acton_mismatch.stdout.txt",
    );
}

#[test]
// Verifies that `acton up --check` returns machine-readable JSON when a newer stable release exists.
fn test_up_check_reports_update_available_as_json() {
    let project = ProjectBuilder::new("up-check-update").build();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v9.9.9",
            &mock_assets(base_url, "9.9.9", supported_archive_name(), 1, 1),
        );
        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases/latest",
            release,
        )]
    });

    let output = up_command(&project)
        .arg("--check")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/up/test_up_check_reports_update_available_as_json.stdout.txt",
    );
}

#[test]
// Verifies that `acton up --check` reports no available update when GitHub returns the current version.
fn test_up_check_reports_no_update_when_latest_matches_current() {
    let project = ProjectBuilder::new("up-check-no-update").build();
    let current_version = build_info::PACKAGE_VERSION;
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            &format!("v{current_version}"),
            &mock_assets(base_url, current_version, supported_archive_name(), 1, 1),
        );
        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases/latest",
            release,
        )]
    });

    let output = up_command(&project)
        .arg("--check")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success();

    output.assert_snapshot_matches(
        "integration/snapshots/up/test_up_check_reports_no_update_when_latest_matches_current.stdout.txt",
    );
}

#[test]
// Verifies that invalid release metadata from the release repository fails without fallback.
fn test_up_update_fails_when_release_json_is_malformed() {
    let project = ProjectBuilder::new("up-update-malformed-json").build();
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![ExpectedHttpRequest::json(
            200,
            "/repos/ton-blockchain/acton/releases/latest",
            "{",
        )]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_update_fails_when_release_json_is_malformed.stderr.txt",
    );
}

#[test]
// Verifies that an explicit version argument is normalized to a `v...` tag and installs that release.
fn test_up_explicit_version_uses_normalized_tag_and_installs_requested_release() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-explicit-version")?;
    let bundle = release_bundle("binary-data-0.1.5")?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v0.1.5",
            &mock_assets(
                base_url,
                "0.1.5",
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            expected_json_response("/repos/ton-blockchain/acton/releases/tags/v0.1.5", release),
            expected_binary_response(
                &format!("/download/0.1.5/{archive_name}"),
                bundle.archive_bytes.clone(),
            ),
            expected_binary_response(
                &format!("/download/0.1.5/{archive_name}.sha256"),
                bundle.checksum_bytes.clone(),
            ),
        ]
    });

    let output = up_command(&project)
        .arg("0.1.5")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_explicit_version_uses_normalized_tag_and_installs_requested_release.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_explicit_version_uses_normalized_tag_and_installs_requested_release.binary.txt",
        )
        .assert_file_snapshot_matches(
            &backup_file_name(),
            "integration/snapshots/up/test_up_explicit_version_uses_normalized_tag_and_installs_requested_release.backup.txt",
        );

    Ok(())
}

#[test]
// Verifies that `--trunk` fetches the trunk tag and installs the trunk artifact.
fn test_up_trunk_installs_trunk_release() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-trunk")?;
    let bundle = release_bundle("binary-data-trunk")?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "trunk",
            &mock_assets(
                base_url,
                "trunk",
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            expected_json_response("/repos/ton-blockchain/acton/releases/tags/trunk", release),
            expected_binary_response(
                &format!("/download/trunk/{archive_name}"),
                bundle.archive_bytes.clone(),
            ),
            expected_binary_response(
                &format!("/download/trunk/{archive_name}.sha256"),
                bundle.checksum_bytes.clone(),
            ),
        ]
    });

    let output = up_command(&project)
        .arg("--trunk")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_trunk_installs_trunk_release.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_trunk_installs_trunk_release.binary.txt",
        )
        .assert_file_snapshot_matches(
            &backup_file_name(),
            "integration/snapshots/up/test_up_trunk_installs_trunk_release.backup.txt",
        );

    Ok(())
}

#[test]
// Verifies that `--force` reinstalls the current version instead of exiting as already up to date.
fn test_up_force_reinstalls_current_version() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-force-reinstall")?;
    let current_version = build_info::PACKAGE_VERSION;
    let bundle = release_bundle(&format!("binary-data-{current_version}"))?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            &format!("v{current_version}"),
            &mock_assets(
                base_url,
                current_version,
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            expected_json_response("/repos/ton-blockchain/acton/releases/latest", release),
            expected_binary_response(
                &format!("/download/{current_version}/{archive_name}"),
                bundle.archive_bytes.clone(),
            ),
            expected_binary_response(
                &format!("/download/{current_version}/{archive_name}.sha256"),
                bundle.checksum_bytes.clone(),
            ),
        ]
    });

    let output = up_command(&project)
        .arg("--force")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_force_reinstalls_current_version.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_force_reinstalls_current_version.binary.txt",
        )
        .assert_file_snapshot_matches(
            &backup_file_name(),
            "integration/snapshots/up/test_up_force_reinstalls_current_version.backup.txt",
        );

    Ok(())
}

#[test]
// Verifies that an unknown explicit version prints the available versions list before failing.
fn test_up_unknown_version_lists_available_versions() {
    let project = ProjectBuilder::new("up-unknown-version").build();
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::empty(404, "/repos/ton-blockchain/acton/releases/tags/v0.0.1"),
            expected_json_response(
                "/repos/ton-blockchain/acton/releases?per_page=100&page=1",
                json!([{ "tag_name": "v0.4.0", "assets": [] }]).to_string(),
            ),
        ]
    });

    let output = up_command(&project)
        .arg("0.0.1")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_unknown_version_lists_available_versions.stderr.txt",
    );
}

#[test]
// Verifies that update fails early when the release is missing the `.sha256` checksum asset.
fn test_up_fails_when_checksum_asset_is_missing() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-missing-checksum")?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = json!({
            "tag_name": "v9.9.9",
            "assets": [release_asset_json(base_url, "9.9.9", &archive_name, 5)]
        })
        .to_string();
        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases/latest",
            release,
        )]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .failure();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_checksum_asset_is_missing.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_checksum_asset_is_missing.stderr.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_fails_when_checksum_asset_is_missing.binary.txt",
        );

    Ok(())
}

#[test]
// Verifies that checksum validation rejects a checksum file that references a different archive name.
fn test_up_fails_when_checksum_mismatch() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-checksum-mismatch")?;
    let bundle = release_bundle("binary-data-9.9.9")?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v9.9.9",
            &mock_assets(
                base_url,
                "9.9.9",
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            expected_json_response("/repos/ton-blockchain/acton/releases/latest", release),
            expected_binary_response(
                &format!("/download/9.9.9/{archive_name}"),
                bundle.archive_bytes.clone(),
            ),
            expected_binary_response(
                &format!("/download/9.9.9/{archive_name}.sha256"),
                b"0000000000000000000000000000000000000000000000000000000000000000  broken.tar.gz\n"
                    .to_vec(),
            ),
        ]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .failure();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_checksum_mismatch.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_checksum_mismatch.stderr.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_fails_when_checksum_mismatch.binary.txt",
        );

    Ok(())
}

#[test]
// Verifies that update fails when the release does not contain an artifact for the current target triple.
fn test_up_fails_when_release_has_no_matching_target_asset() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-missing-target-asset")?;
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = json!({
            "tag_name": "v9.9.9",
            "assets": [
                release_asset_json(base_url, "9.9.9", "acton-x86_64-pc-windows-msvc.tar.gz", 5),
                release_asset_json(base_url, "9.9.9", "acton-x86_64-pc-windows-msvc.tar.gz.sha256", 5)
            ]
        })
        .to_string();
        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases/latest",
            release,
        )]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .failure();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_release_has_no_matching_target_asset.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_release_has_no_matching_target_asset.stderr.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_fails_when_release_has_no_matching_target_asset.binary.txt",
        );

    Ok(())
}

#[test]
// Verifies that HTTP failures during asset download are surfaced and do not replace the current binary.
fn test_up_fails_when_asset_download_returns_http_error() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-download-http-error")?;
    let bundle = release_bundle("binary-data-9.9.9")?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v9.9.9",
            &mock_assets(
                base_url,
                "9.9.9",
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            expected_json_response("/repos/ton-blockchain/acton/releases/latest", release),
            ExpectedHttpRequest::binary(
                500,
                &format!("/download/9.9.9/{archive_name}"),
                Vec::new(),
            ),
        ]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .failure();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_asset_download_returns_http_error.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_asset_download_returns_http_error.stderr.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_fails_when_asset_download_returns_http_error.binary.txt",
        );

    Ok(())
}

#[test]
// Verifies that `--list` reports a GitHub fetch failure when the release repository is unreachable.
fn test_up_list_fails_when_offline() {
    let project = ProjectBuilder::new("up-list-offline").build();
    let offline_url = unused_local_url();

    let output = up_command(&project)
        .arg("--list")
        .env(TEST_GITHUB_API_BASE_ENV, &offline_url)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_list_fails_when_offline.stderr.txt",
    );
}

#[test]
// Verifies that `--list` surfaces GitHub API server errors from the release repository.
fn test_up_list_fails_when_github_returns_server_errors() {
    let project = ProjectBuilder::new("up-list-server-errors").build();
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![ExpectedHttpRequest::empty(
            500,
            "/repos/ton-blockchain/acton/releases?per_page=100&page=1",
        )]
    });

    let output = up_command(&project)
        .arg("--list")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_list_fails_when_github_returns_server_errors.stderr.txt",
    );
}

#[test]
// Verifies that a normal update surfaces a connectivity error instead of pretending the release is missing.
fn test_up_update_fails_when_offline() {
    let project = ProjectBuilder::new("up-update-offline").build();
    let offline_url = unused_local_url();

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, &offline_url)
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_update_fails_when_offline.stderr.txt",
    );
}

#[test]
// Verifies that a normal update surfaces GitHub API server errors from the release repository.
fn test_up_update_fails_when_github_returns_server_errors() {
    let project = ProjectBuilder::new("up-update-server-errors").build();
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![ExpectedHttpRequest::empty(
            500,
            "/repos/ton-blockchain/acton/releases/latest",
        )]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_update_fails_when_github_returns_server_errors.stderr.txt",
    );
}

#[test]
// Verifies that an unknown version still fails clearly when fetching the versions list also fails.
fn test_up_unknown_version_when_available_versions_list_fails() {
    let project = ProjectBuilder::new("up-unknown-version-list-failure").build();
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::empty(404, "/repos/ton-blockchain/acton/releases/tags/v0.0.1"),
            ExpectedHttpRequest::empty(
                500,
                "/repos/ton-blockchain/acton/releases?per_page=100&page=1",
            ),
        ]
    });

    let output = up_command(&project)
        .arg("0.0.1")
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/up/test_up_unknown_version_when_available_versions_list_fails.stderr.txt",
    );
}

#[test]
// Verifies that update reports a download connectivity error after release lookup succeeds but the asset host is offline.
fn test_up_fails_when_asset_download_is_offline() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-download-offline")?;
    let archive_name = supported_archive_name();
    let offline_url = unused_local_url();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = json!({
            "tag_name": "v9.9.9",
            "assets": [
                {
                    "name": archive_name,
                    "url": format!("{base_url}/download-api/9.9.9/{archive_name}"),
                    "browser_download_url": format!("{offline_url}/download/9.9.9/{archive_name}"),
                    "size": 42
                },
                {
                    "name": format!("{archive_name}.sha256"),
                    "url": format!("{base_url}/download-api/9.9.9/{archive_name}.sha256"),
                    "browser_download_url": format!("{base_url}/download/9.9.9/{archive_name}.sha256"),
                    "size": 42
                }
            ]
        })
        .to_string();

        vec![expected_json_response(
            "/repos/ton-blockchain/acton/releases/latest",
            release,
        )]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .run()
        .failure();

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_asset_download_is_offline.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/up/test_up_fails_when_asset_download_is_offline.stderr.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_fails_when_asset_download_is_offline.binary.txt",
        );

    Ok(())
}

#[test]
// Verifies that setting `GITHUB_TOKEN` switches asset downloads to the authenticated asset API URLs.
fn test_up_downloads_assets_via_api_url_when_github_token_is_set() -> Result<()> {
    let (project, fake_binary) = setup_up_project("up-github-token-download")?;
    let bundle = release_bundle("binary-data-9.9.9")?;
    let archive_name = supported_archive_name();
    let token = "test-token";
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v9.9.9",
            &mock_assets(
                base_url,
                "9.9.9",
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            ExpectedHttpRequest::json_with_headers(
                200,
                "/repos/ton-blockchain/acton/releases/latest",
                release,
                vec![("authorization".to_owned(), format!("token {token}"))],
            ),
            ExpectedHttpRequest::binary_with_headers(
                200,
                &format!("/download-api/9.9.9/{archive_name}"),
                bundle.archive_bytes.clone(),
                vec![
                    ("authorization".to_owned(), format!("token {token}")),
                    ("accept".to_owned(), "application/octet-stream".to_owned()),
                ],
            ),
            ExpectedHttpRequest::binary_with_headers(
                200,
                &format!("/download-api/9.9.9/{archive_name}.sha256"),
                bundle.checksum_bytes.clone(),
                vec![
                    ("authorization".to_owned(), format!("token {token}")),
                    ("accept".to_owned(), "application/octet-stream".to_owned()),
                ],
            ),
        ]
    });

    let output = up_command(&project)
        .env(TEST_GITHUB_API_BASE_ENV, mock.base_url())
        .env(TEST_CURRENT_EXE_ENV, &fake_binary.to_string_lossy())
        .env("GITHUB_TOKEN", token)
        .run()
        .success();

    let captured = mock.captured_requests();
    fs::write(
        project.path().join("captured-paths.txt"),
        format!(
            "{}\n",
            captured
                .iter()
                .map(|request| request.path.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )?;

    output
        .assert_snapshot_matches(
            "integration/snapshots/up/test_up_downloads_assets_via_api_url_when_github_token_is_set.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".fake-bin/acton",
            "integration/snapshots/up/test_up_downloads_assets_via_api_url_when_github_token_is_set.binary.txt",
        )
        .assert_file_snapshot_matches(
            &backup_file_name(),
            "integration/snapshots/up/test_up_downloads_assets_via_api_url_when_github_token_is_set.backup.txt",
        )
        .assert_file_snapshot_matches(
            "captured-paths.txt",
            "integration/snapshots/up/test_up_downloads_assets_via_api_url_when_github_token_is_set.captured.txt",
        );

    Ok(())
}

fn up_command(project: &Project) -> crate::support::project::ActonCommand {
    project.acton().current_dir(project.path()).arg("up")
}

fn setup_up_project(name: &str) -> Result<(Project, PathBuf)> {
    let project = ProjectBuilder::new(name).build();
    let fake_bin_dir = project.path().join(".fake-bin");
    fs::create_dir_all(&fake_bin_dir)?;

    let fake_binary = fake_bin_dir.join("acton");
    fs::write(&fake_binary, "old_binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&fake_binary)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&fake_binary, perms)?;
    }

    Ok((project, fake_binary))
}

fn backup_file_name() -> String {
    format!(".fake-bin/acton-{}", build_info::PACKAGE_VERSION)
}

fn supported_archive_name() -> String {
    format!("acton-{}.tar.gz", build_info::TARGET_TRIPLE)
}

fn mock_assets(
    base_url: &str,
    label: &str,
    archive_name: impl AsRef<str>,
    archive_size: u64,
    checksum_size: u64,
) -> Vec<Value> {
    let archive_name = archive_name.as_ref();
    vec![
        release_asset_json(base_url, label, archive_name, archive_size),
        release_asset_json(
            base_url,
            label,
            &format!("{archive_name}.sha256"),
            checksum_size,
        ),
    ]
}

fn release_response(tag_name: &str, assets: &[Value]) -> String {
    json!({
        "tag_name": tag_name,
        "assets": assets,
    })
    .to_string()
}

fn release_asset_json(base_url: &str, label: &str, name: &str, size: u64) -> Value {
    json!({
        "name": name,
        "url": format!("{base_url}/download-api/{label}/{name}"),
        "browser_download_url": format!("{base_url}/download/{label}/{name}"),
        "size": size,
    })
}

fn expected_json_response(path: &str, body: String) -> ExpectedHttpRequest {
    ExpectedHttpRequest::json(200, path, body)
}

fn expected_binary_response(path: &str, body: Vec<u8>) -> ExpectedHttpRequest {
    ExpectedHttpRequest::binary(200, path, body)
}

fn unused_local_url() -> String {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to reserve an unused port");
    let addr = listener
        .local_addr()
        .expect("failed to inspect reserved local port");
    drop(listener);
    format!("http://{addr}")
}

struct ReleaseBundle {
    archive_bytes: Vec<u8>,
    checksum_bytes: Vec<u8>,
}

impl ReleaseBundle {
    fn archive_len(&self) -> u64 {
        self.archive_bytes.len() as u64
    }

    fn checksum_len(&self) -> u64 {
        self.checksum_bytes.len() as u64
    }
}

fn release_bundle(binary_contents: &str) -> Result<ReleaseBundle> {
    let archive_name = supported_archive_name();
    let archive_bytes = archive_bytes(binary_contents)?;
    let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
    let checksum_bytes = format!("{checksum}  {archive_name}\n").into_bytes();
    Ok(ReleaseBundle {
        archive_bytes,
        checksum_bytes,
    })
}

fn archive_bytes(binary_contents: &str) -> Result<Vec<u8>> {
    let encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(encoder);

    let mut header = tar::Header::new_gnu();
    header.set_path("acton")?;
    header.set_size(binary_contents.len() as u64);
    header.set_mode(0o755);
    header.set_mtime(0);
    header.set_cksum();
    tar.append(&header, binary_contents.as_bytes())?;

    let encoder = tar.into_inner()?;
    Ok(encoder.finish()?)
}

#[derive(Debug, Clone)]
struct CapturedHttpRequest {
    path: String,
}

struct ExpectedHttpRequest {
    method: &'static str,
    path: String,
    required_headers: Vec<(String, String)>,
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl ExpectedHttpRequest {
    fn json(status: u16, path: &str, body: impl Into<String>) -> Self {
        Self {
            method: "GET",
            path: path.to_owned(),
            required_headers: Vec::new(),
            status,
            content_type: "application/json",
            body: body.into().into_bytes(),
        }
    }

    fn json_with_headers(
        status: u16,
        path: &str,
        body: impl Into<String>,
        required_headers: Vec<(String, String)>,
    ) -> Self {
        let mut request = Self::json(status, path, body);
        request.required_headers = required_headers;
        request
    }

    fn binary(status: u16, path: &str, body: Vec<u8>) -> Self {
        Self {
            method: "GET",
            path: path.to_owned(),
            required_headers: Vec::new(),
            status,
            content_type: "application/octet-stream",
            body,
        }
    }

    fn binary_with_headers(
        status: u16,
        path: &str,
        body: Vec<u8>,
        required_headers: Vec<(String, String)>,
    ) -> Self {
        let mut request = Self::binary(status, path, body);
        request.required_headers = required_headers;
        request
    }

    fn empty(status: u16, path: &str) -> Self {
        Self::json(status, path, "")
    }
}

struct GitHubMockServer {
    base_url: String,
    captured: Arc<Mutex<Vec<CapturedHttpRequest>>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl GitHubMockServer {
    fn spawn_with<F>(builder: F) -> Self
    where
        F: FnOnce(&str) -> Vec<ExpectedHttpRequest>,
    {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind GitHub mock");
        listener
            .set_nonblocking(true)
            .expect("failed to set GitHub mock non-blocking");
        let addr = listener
            .local_addr()
            .expect("failed to inspect GitHub mock address");
        let base_url = format!("http://{addr}");
        let expected_requests = builder(&base_url);

        let captured = Arc::new(Mutex::new(Vec::<CapturedHttpRequest>::new()));
        let captured_thread = Arc::clone(&captured);

        let handle = thread::spawn(move || {
            for expected in expected_requests {
                let wait_until = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(err) if err.kind() == ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() <= wait_until,
                                "timed out waiting for GitHub mock request: {} {}",
                                expected.method,
                                expected.path
                            );
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(err) => panic!("GitHub mock accept failed: {err}"),
                    }
                };

                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("failed to set GitHub mock read timeout");

                let request = read_http_request(&stream);
                assert_eq!(
                    request.method, expected.method,
                    "unexpected HTTP method for {}",
                    expected.path
                );
                assert_eq!(
                    request.path, expected.path,
                    "unexpected request path for {}",
                    expected.path
                );

                for (name, value) in &expected.required_headers {
                    assert_eq!(
                        header_value(&request.headers, name),
                        Some(value.as_str()),
                        "missing or invalid header `{name}` for {}",
                        expected.path
                    );
                }

                captured_thread
                    .lock()
                    .expect("captured GitHub requests mutex poisoned")
                    .push(CapturedHttpRequest {
                        path: request.path.clone(),
                    });

                write_http_response(
                    &mut stream,
                    expected.status,
                    expected.content_type,
                    &expected.body,
                );
            }
        });

        Self {
            base_url,
            captured,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn captured_requests(&self) -> Vec<CapturedHttpRequest> {
        self.captured
            .lock()
            .expect("captured GitHub requests mutex poisoned")
            .clone()
    }
}

impl Drop for GitHubMockServer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            if thread::panicking() {
                let _ = handle.join();
            } else {
                handle
                    .join()
                    .expect("GitHub mock server thread must finish");
            }
        }
    }
}

struct ParsedHttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn read_http_request(stream: &std::net::TcpStream) -> ParsedHttpRequest {
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .expect("failed to clone GitHub mock stream"),
    );
    let mut request_line = String::new();
    let read_deadline = Instant::now() + Duration::from_secs(2);

    loop {
        request_line.clear();
        match reader.read_line(&mut request_line) {
            Ok(0) => {
                assert!(
                    Instant::now() <= read_deadline,
                    "timed out waiting for GitHub mock request line"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Ok(_) => break,
            Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                assert!(
                    Instant::now() <= read_deadline,
                    "timed out waiting for GitHub mock request line"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("failed to read GitHub mock request line: {err}"),
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();

    let mut headers = Vec::new();
    let mut content_length = 0_usize;
    loop {
        let mut header_line = String::new();
        let read = reader
            .read_line(&mut header_line)
            .expect("failed to read GitHub mock header line");
        if read == 0 || header_line == "\r\n" {
            break;
        }

        if let Some((name, value)) = header_line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }

            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }

    if content_length > 0 {
        let mut body = vec![0_u8; content_length];
        reader
            .read_exact(&mut body)
            .expect("failed to read GitHub mock request body");
    }

    ParsedHttpRequest {
        method,
        path,
        headers,
    }
}

fn write_http_response(
    stream: &mut std::net::TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) {
    let response_head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        status_text(status),
        content_type,
        body.len()
    );

    stream
        .write_all(response_head.as_bytes())
        .expect("failed to write GitHub mock response head");
    stream
        .write_all(body)
        .expect("failed to write GitHub mock response body");
    stream
        .flush()
        .expect("failed to flush GitHub mock response");
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}
