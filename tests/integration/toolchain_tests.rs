use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use acton::build_info;
use anyhow::Result;
use base64::Engine;
use flate2::{Compression, GzBuilder};
use fs2::FileExt;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const TEST_TOOLCHAIN_GITHUB_API_BASE_ENV: &str = "ACTON_TEST_TOOLCHAIN_GITHUB_API_BASE";

#[test]
fn test_toolchain_help_snapshot() {
    let project = ProjectBuilder::new("toolchain-help").build();
    let home = isolated_home(&project);

    toolchain_command(&project, &home)
        .arg("--help")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/toolchain/test_toolchain_help.stdout.txt");
}

#[test]
fn test_toolchain_probe_json_snapshot() {
    let project = ProjectBuilder::new("toolchain-probe-json").build();

    project
        .acton()
        .current_dir(project.path())
        .arg("--toolchain-probe")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_probe_json.stdout.txt",
        );
}

#[test]
fn test_toolchain_list_empty_home_snapshot() {
    let project = ProjectBuilder::new("toolchain-list-empty").build();
    let home = isolated_home(&project);
    write_current_toolchain_index(&home).expect("failed to write toolchain index");

    toolchain_command(&project, &home)
        .arg("list")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_empty_home.stdout.txt",
        );
}

#[test]
fn test_toolchain_list_fetches_remote_index_and_caches_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-remote-index").build();
    let home = isolated_home(&project);
    let remote_index = remote_toolchain_index_json("0.5.0", "1.5.0");
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![ExpectedHttpRequest::json(
            "/repos/i582/acton-public/contents/toolchain-index.json",
            github_contents_response(&remote_index),
        )]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_fetches_remote_index_and_caches.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index.json",
            "integration/snapshots/toolchain/test_toolchain_list_fetches_remote_index_and_caches.index.json",
        )
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index-meta.json",
            "integration/snapshots/toolchain/test_toolchain_list_fetches_remote_index_and_caches.index-meta.json",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_fetches_index_from_fallback_repo_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-index-fallback").build();
    let home = isolated_home(&project);
    let remote_index = remote_toolchain_index_json("0.5.1", "1.5.1");
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::empty(
                404,
                "/repos/i582/acton-public/contents/toolchain-index.json",
            ),
            ExpectedHttpRequest::json(
                "/repos/ton-blockchain/acton/contents/toolchain-index.json",
                github_contents_response(&remote_index),
            ),
        ]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index-meta.json",
            "integration/snapshots/toolchain/test_toolchain_list_fetches_index_from_fallback_repo.index-meta.json",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_uses_fresh_cached_index_without_network_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-fresh-cache").build();
    let home = isolated_home(&project);
    write_toolchain_index_json(&home, &remote_toolchain_index_json("0.6.0", "1.6.0"))?;
    let mock = GitHubMockServer::spawn_with(|_| Vec::new());

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_uses_fresh_cached_index_without_network.stdout.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_refreshes_stale_cache_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-stale-cache-refresh").build();
    let home = isolated_home(&project);
    write_stale_toolchain_index_json(
        &home,
        &remote_toolchain_index_json("0.6.0", "1.6.0"),
        "i582/acton-public",
        None,
        None,
    )?;
    let remote_index = remote_toolchain_index_json("0.6.1", "1.6.1");
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![ExpectedHttpRequest::json(
            "/repos/i582/acton-public/contents/toolchain-index.json",
            github_contents_response(&remote_index),
        )]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_refreshes_stale_cache.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index.json",
            "integration/snapshots/toolchain/test_toolchain_list_refreshes_stale_cache.index.json",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_reuses_stale_cache_when_refresh_fails_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-stale-cache-fallback").build();
    let home = isolated_home(&project);
    write_stale_toolchain_index_json(
        &home,
        &remote_toolchain_index_json("0.6.0", "1.6.0"),
        "i582/acton-public",
        None,
        None,
    )?;
    let mock = failing_toolchain_index_server();

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_reuses_stale_cache_when_refresh_fails.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_reuses_stale_cache_when_refresh_fails.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_not_modified_refreshes_cache_metadata_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-not-modified").build();
    let home = isolated_home(&project);
    write_stale_toolchain_index_json(
        &home,
        &remote_toolchain_index_json("0.6.0", "1.6.0"),
        "i582/acton-public",
        Some("\"test-etag\""),
        Some("Fri, 24 Apr 2026 00:00:00 GMT"),
    )?;
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::empty(
                304,
                "/repos/i582/acton-public/contents/toolchain-index.json",
            )
            .with_header("if-none-match", "\"test-etag\"")
            .with_header("if-modified-since", "Fri, 24 Apr 2026 00:00:00 GMT"),
        ]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index-meta.json",
            "integration/snapshots/toolchain/test_toolchain_list_not_modified_refreshes_cache_metadata.index-meta.json",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_not_modified_without_cache_warns_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-not-modified-no-cache").build();
    let home = isolated_home(&project);
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![ExpectedHttpRequest::empty(
            304,
            "/repos/i582/acton-public/contents/toolchain-index.json",
        )]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_not_modified_without_cache_warns.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_not_modified_without_cache_warns.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_fresh_remote_index_saves_http_cache_headers_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-cache-headers").build();
    let home = isolated_home(&project);
    let remote_index = remote_toolchain_index_json("0.6.2", "1.6.2");
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::json(
                "/repos/i582/acton-public/contents/toolchain-index.json",
                github_contents_response(&remote_index),
            )
            .with_response_header("ETag", "\"toolchain-index-etag\"")
            .with_response_header("Last-Modified", "Fri, 24 Apr 2026 00:00:00 GMT"),
        ]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index-meta.json",
            "integration/snapshots/toolchain/test_toolchain_list_fresh_remote_index_saves_http_cache_headers.index-meta.json",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_warns_about_malformed_cached_metadata_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-malformed-cache-meta").build();
    let home = isolated_home(&project);
    write_toolchain_index_json(&home, &remote_toolchain_index_json("0.6.0", "1.6.0"))?;
    fs::write(home.join(".acton/toolchains/index-meta.json"), "{")?;
    let mock = failing_toolchain_index_server();

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_warns_about_malformed_cached_metadata.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_warns_about_malformed_cached_metadata.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_list_invalid_fresh_index_keeps_cached_index_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-list-invalid-refresh").build();
    let home = isolated_home(&project);
    write_stale_toolchain_index_json(
        &home,
        &remote_toolchain_index_json("0.6.0", "1.6.0"),
        "i582/acton-public",
        None,
        None,
    )?;
    let invalid_index = r#"{"schema":1,"generated_at":"2026-04-24T00:00:00Z","releases":[{"acton":"0.6.1","tolk":"1.6","tag":"v0.6.1","stable":true}]}"#;
    let mock = GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::json(
                "/repos/i582/acton-public/contents/toolchain-index.json",
                github_contents_response(invalid_index),
            ),
            ExpectedHttpRequest::empty(
                404,
                "/repos/ton-blockchain/acton/contents/toolchain-index.json",
            ),
        ]
    });

    toolchain_command(&project, &home)
        .arg("list")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_invalid_fresh_index_keeps_cached_index.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_invalid_fresh_index_keeps_cached_index.stderr.txt",
        )
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/index.json",
            "integration/snapshots/toolchain/test_toolchain_list_invalid_fresh_index_keeps_cached_index.index.json",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_current_version_noop_snapshot() {
    let project = ProjectBuilder::new("toolchain-install-current").build();
    let home = isolated_home(&project);
    write_current_toolchain_index(&home).expect("failed to write toolchain index");

    toolchain_command(&project, &home)
        .arg("install")
        .arg(build_info::PACKAGE_VERSION)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_current_version_noop.stdout.txt",
        );
}

#[test]
fn test_toolchain_remove_missing_version_snapshot() {
    let project = ProjectBuilder::new("toolchain-remove-missing").build();
    let home = isolated_home(&project);

    toolchain_command(&project, &home)
        .arg("remove")
        .arg("9.9.9")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_remove_missing_version.stdout.txt",
        );
}

#[test]
fn test_toolchain_remove_installed_noninteractive_requires_confirmation_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-remove-noninteractive").build();
    let home = isolated_home(&project);
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &version_probe_fake_acton("0.4.0", "1.4.0", "remove target"),
    )?;

    toolchain_command(&project, &home)
        .arg("remove")
        .arg("0.4.0")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_remove_installed_noninteractive_requires_confirmation.stderr.txt",
        );

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_toolchain_remove_interactive_decline_snapshot() -> Result<()> {
    use expectrl::Eof;

    let project = ProjectBuilder::new("toolchain-remove-interactive-decline").build();
    let home = isolated_home(&project);
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &version_probe_fake_acton("0.4.0", "1.4.0", "remove target"),
    )?;

    let mut session = toolchain_command(&project, &home)
        .arg("remove")
        .arg("0.4.0")
        .spawn_pty();

    session.expect("Remove Acton 0.4.0 from");
    session.send_line("n", "failed to decline toolchain removal");
    session.expect("Cancelled.");
    session.expect(Eof);
    write_toolchain_exists_snapshot(&project, &home, "0.4.0")?;

    assert_project_file_snapshot_matches(
        &project,
        "toolchain-exists.txt",
        "integration/snapshots/toolchain/test_toolchain_remove_interactive_decline.exists.txt",
    );

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_toolchain_remove_interactive_accepts_snapshot() -> Result<()> {
    use expectrl::Eof;

    let project = ProjectBuilder::new("toolchain-remove-interactive-accept").build();
    let home = isolated_home(&project);
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &version_probe_fake_acton("0.4.0", "1.4.0", "remove target"),
    )?;

    let mut session = toolchain_command(&project, &home)
        .arg("remove")
        .arg("0.4.0")
        .spawn_pty();

    session.expect("Remove Acton 0.4.0 from");
    session.send_line("y", "failed to confirm toolchain removal");
    session.expect("Removed Acton 0.4.0 from");
    session.expect(Eof);
    write_toolchain_exists_snapshot(&project, &home, "0.4.0")?;

    assert_project_file_snapshot_matches(
        &project,
        "toolchain-exists.txt",
        "integration/snapshots/toolchain/test_toolchain_remove_interactive_accepts.exists.txt",
    );

    Ok(())
}

#[test]
fn test_toolchain_install_without_project_manifest_snapshot() {
    let project = ProjectBuilder::new("toolchain-install-no-manifest")
        .without_acton_toml()
        .build();
    let home = isolated_home(&project);

    toolchain_command(&project, &home)
        .arg("install")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_without_project_manifest.stderr.txt",
        );
}

#[test]
fn test_toolchain_install_without_toolchain_section_snapshot() {
    let project = ProjectBuilder::new("toolchain-install-no-section").build();
    let home = isolated_home(&project);

    toolchain_command(&project, &home)
        .arg("install")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_without_toolchain_section.stderr.txt",
        );
}

#[test]
fn test_toolchain_which_missing_selected_version_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-which-missing", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("which")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_which_missing_selected_version.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_installed_yanked_metadata_fails_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-installed-yanked", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_current_toolchain_index(&home)?;
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        true,
        Some("broken release"),
        &version_probe_fake_acton("0.4.0", "1.4.0", "should not run"),
    )?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .test()
        .env("HOME", &home)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_installed_yanked_metadata_fails.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_conflicting_pins_suggests_fix_snapshot() -> Result<()> {
    let project = project_with_toolchain(
        "toolchain-resolve-conflict",
        "acton = \"0.3.1\"\ntolk = \"1.4.0\"",
    );
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_conflicting_pins_suggests_fix.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_unknown_acton_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-unknown-acton", "acton = \"9.9.9\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_unknown_acton.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_unknown_tolk_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-unknown-tolk", "tolk = \"9.9.9\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_unknown_tolk.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_invalid_acton_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-invalid-acton", "acton = \"0.4\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_invalid_acton.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_invalid_tolk_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-invalid-tolk", "tolk = \"1.4\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_invalid_tolk.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_rejects_trunk_acton_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-trunk-acton", "acton = \"trunk\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_rejects_trunk_acton.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_normalizes_v_prefixed_acton_snapshot() -> Result<()> {
    let project =
        project_with_toolchain("toolchain-resolve-v-prefixed-acton", "acton = \"v0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_normalizes_v_prefixed_acton.stdout.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_yanked_acton_from_index_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-yanked-index", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index_json(
        &home,
        &toolchain_index_json(vec![release_entry("0.4.0", "1.4.0", true, true)]),
    )?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_yanked_acton_from_index.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_resolve_tolk_only_all_compatible_releases_yanked_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-resolve-all-yanked", "tolk = \"1.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index_json(
        &home,
        &toolchain_index_json(vec![
            release_entry("0.4.0", "1.4.0", true, true),
            release_entry("0.4.1", "1.4.0", true, true),
        ]),
    )?;

    toolchain_command(&project, &home)
        .arg("resolve")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_resolve_tolk_only_all_compatible_releases_yanked.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_invalid_cli_selector_snapshot() {
    let project = ProjectBuilder::new("toolchain-invalid-cli-selector").build();

    project
        .acton()
        .current_dir(project.path())
        .arg("+0.4")
        .arg("test")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_invalid_cli_selector.stderr.txt",
        );
}

#[test]
fn test_project_command_cli_selector_disallows_toolchain_command_snapshot() {
    let project = ProjectBuilder::new("toolchain-cli-selector-toolchain-command").build();

    project
        .acton()
        .current_dir(project.path())
        .arg("+0.4.0")
        .arg("toolchain")
        .arg("list")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_cli_selector_disallows_toolchain_command.stderr.txt",
        );
}

#[test]
fn test_toolchain_install_missing_version_without_index_fails_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-index-unavailable").build();
    let home = isolated_home(&project);
    let mock = failing_toolchain_index_server();

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_missing_version_without_index_fails.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_installed_exact_without_index_reexecs_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-installed-no-index", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &recording_fake_acton("0.4.0", "1.4.0", "installed exact reexec complete"),
    )?;
    let mock = failing_toolchain_index_server();
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .test()
        .env("HOME", &home)
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_installed_exact_without_index_reexecs.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_project_command_installed_exact_without_index_reexecs.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_missing_toolchain_noninteractive_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-project-command-missing", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .test()
        .env("HOME", &home)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_missing_toolchain_noninteractive.stderr.txt",
        );

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_project_command_missing_toolchain_interactive_accepts_install_and_reexecs_snapshot()
-> Result<()> {
    use expectrl::Eof;

    let project =
        project_with_toolchain("toolchain-project-command-interactive", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = recording_fake_acton("0.4.0", "1.4.0", "fake acton reexec complete");
    let bundle = release_bundle(&fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    let home = home.to_string_lossy().into_owned();
    let mut session = project
        .acton()
        .test()
        .env("HOME", &home)
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .spawn_pty();

    session.expect("Project requires acton 0.4.0 (Tolk 1.4.0). Install it now?");
    session.send_line("y", "failed to confirm toolchain installation");
    session.expect("fake acton reexec complete");
    session.expect(Eof);
    session.assert_file_snapshot_matches(
        ".toolchain-reexec.txt",
        "integration/snapshots/toolchain/test_project_command_missing_toolchain_interactive_accepts_install_and_reexecs.reexec.txt",
    );

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_project_command_missing_toolchain_interactive_decline_snapshot() -> Result<()> {
    use expectrl::Eof;

    let project = project_with_toolchain("toolchain-project-command-decline", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let home = home.to_string_lossy().into_owned();

    let mut session = project.acton().test().env("HOME", &home).spawn_pty();

    let mut transcript = Vec::new();
    let prompt = expectrl::Session::expect(
        &mut *session,
        "Project requires acton 0.4.0 (Tolk 1.4.0). Install it now?",
    )
    .expect("expected toolchain install prompt");
    transcript.extend_from_slice(prompt.as_bytes());

    session.send_line("n", "failed to decline toolchain installation");

    let output = expectrl::Session::expect(&mut *session, Eof)
        .expect("expected process to exit after declining toolchain installation");
    transcript.extend_from_slice(output.as_bytes());
    assert_pty_transcript_snapshot_matches(
        &project,
        &transcript,
        "integration/snapshots/toolchain/test_project_command_missing_toolchain_interactive_decline.pty.txt",
    );

    Ok(())
}

#[test]
fn test_ci_preinstall_then_project_command_reexecs_noninteractive_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-ci-preinstall", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = recording_fake_acton("0.4.0", "1.4.0", "ci fake acton reexec complete");
    let bundle = release_bundle(&fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_ci_preinstall_then_project_command_reexecs_noninteractive.install.stdout.txt",
        );

    let home = home.to_string_lossy().into_owned();
    project
        .acton()
        .test()
        .env("HOME", &home)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_ci_preinstall_then_project_command_reexecs_noninteractive.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_ci_preinstall_then_project_command_reexecs_noninteractive.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_tolk_only_selects_newest_compatible_release_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-install-tolk-only", "tolk = \"1.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index_json(
        &home,
        &toolchain_index_json(vec![
            release_entry("0.4.0", "1.4.0", true, false),
            release_entry("0.4.1", "1.4.0", true, false),
            release_entry("0.4.2", "1.4.0", true, true),
        ]),
    )?;

    let fake_acton = version_probe_fake_acton("0.4.1", "1.4.0", "toolchain-acton-0.4.1");
    let bundle = release_bundle(&fake_acton)?;
    let mock = mock_release_server("0.4.1", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_tolk_only_selects_newest_compatible_release.stdout.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_cli_selector_overrides_conflicting_project_toolchain_snapshot() -> Result<()>
{
    let project = project_with_toolchain(
        "toolchain-cli-selector-overrides-conflict",
        "acton = \"0.3.1\"\ntolk = \"1.3.0\"",
    );
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &recording_fake_acton("0.4.0", "1.4.0", "cli override reexec complete"),
    )?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .current_dir(project.path())
        .arg("+0.4.0")
        .arg("test")
        .env("HOME", &home)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_cli_selector_overrides_conflicting_project_toolchain.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_project_command_cli_selector_overrides_conflicting_project_toolchain.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_project_toolchain_reexecs_library_publish_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-library-publish-project", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &recording_fake_acton("0.4.0", "1.4.0", "library publish reexec complete"),
    )?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .current_dir(project.path())
        .arg("library")
        .arg("publish")
        .arg("counter")
        .arg("--duration")
        .arg("1d")
        .arg("--wallet")
        .arg("test")
        .arg("--yes")
        .env("HOME", &home)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_project_toolchain_reexecs_library_publish.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_project_toolchain_reexecs_library_publish.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_cli_selector_reexecs_library_publish_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-library-publish-cli").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &recording_fake_acton("0.4.0", "1.4.0", "cli library publish reexec complete"),
    )?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .current_dir(project.path())
        .arg("+0.4.0")
        .arg("library")
        .arg("publish")
        .arg("counter")
        .arg("--duration")
        .arg("1d")
        .arg("--wallet")
        .arg("test")
        .arg("--yes")
        .env("HOME", &home)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_cli_selector_reexecs_library_publish.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_project_command_cli_selector_reexecs_library_publish.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_project_toolchain_reexecs_retrace_with_contract_snapshot() -> Result<()> {
    let project = project_with_toolchain("toolchain-retrace-contract-project", "acton = \"0.4.0\"");
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &recording_fake_acton("0.4.0", "1.4.0", "retrace contract reexec complete"),
    )?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .current_dir(project.path())
        .arg("retrace")
        .arg("abcdef")
        .arg("--contract")
        .arg("counter")
        .env("HOME", &home)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_project_toolchain_reexecs_retrace_with_contract.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_project_toolchain_reexecs_retrace_with_contract.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_cli_selector_reexecs_retrace_with_contract_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-retrace-contract-cli").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    write_installed_toolchain(
        &home,
        "0.4.0",
        "1.4.0",
        false,
        None,
        &recording_fake_acton("0.4.0", "1.4.0", "cli retrace contract reexec complete"),
    )?;
    let home = home.to_string_lossy().into_owned();

    project
        .acton()
        .current_dir(project.path())
        .arg("+0.4.0")
        .arg("retrace")
        .arg("abcdef")
        .arg("--contract")
        .arg("counter")
        .env("HOME", &home)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_cli_selector_reexecs_retrace_with_contract.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".toolchain-reexec.txt",
            "integration/snapshots/toolchain/test_project_command_cli_selector_reexecs_retrace_with_contract.reexec.txt",
        );

    Ok(())
}

#[test]
fn test_project_command_cli_selector_disallows_retrace_without_contract_snapshot() {
    let project = ProjectBuilder::new("toolchain-retrace-no-contract-cli").build();

    project
        .acton()
        .current_dir(project.path())
        .arg("+0.4.0")
        .arg("retrace")
        .arg("abcdef")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_project_command_cli_selector_disallows_retrace_without_contract.stderr.txt",
        );
}

#[test]
fn test_toolchain_install_fails_when_probe_reports_wrong_acton_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-probe-mismatch").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = version_probe_fake_acton("0.4.1", "1.4.0", "wrong acton");
    let bundle = release_bundle(&fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_probe_reports_wrong_acton.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_probe_reports_wrong_tolk_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-probe-wrong-tolk").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = version_probe_fake_acton("0.4.0", "1.4.1", "wrong tolk");
    let bundle = release_bundle(&fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_probe_reports_wrong_tolk.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_probe_output_is_unparseable_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-probe-unparseable").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = "#!/bin/sh\necho 'not a version line'\n";
    let bundle = release_bundle(fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_probe_output_is_unparseable.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_probe_exits_nonzero_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-probe-nonzero").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = "#!/bin/sh\nexit 42\n";
    let bundle = release_bundle(fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_probe_exits_nonzero.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_existing_directory_without_binary_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-dir-without-binary").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    fs::create_dir_all(home.join(".acton/toolchains/0.4.0"))?;

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_existing_directory_without_binary.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_release_missing_target_asset_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-missing-target-asset").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v0.4.0",
            &mock_assets(base_url, "0.4.0", "acton-wrong-target.tar.gz", 1, 1),
        );
        vec![ExpectedHttpRequest::json(
            "/repos/ton-blockchain/acton/releases/tags/v0.4.0",
            release,
        )]
    });

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_release_missing_target_asset.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_release_missing_checksum_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-missing-checksum").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let archive_name = supported_archive_name();
    let mock = GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            "v0.4.0",
            &[release_asset_json(base_url, "0.4.0", &archive_name, 1)],
        );
        vec![ExpectedHttpRequest::json(
            "/repos/ton-blockchain/acton/releases/tags/v0.4.0",
            release,
        )]
    });

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_release_missing_checksum.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_checksum_mismatches_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-checksum-mismatch").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let archive_name = supported_archive_name();
    let archive_bytes = archive_bytes(&version_probe_fake_acton(
        "0.4.0",
        "1.4.0",
        "checksum mismatch",
    ))?;
    let checksum_bytes = format!("{:064x}  {archive_name}\n", 0).into_bytes();
    let bundle = ReleaseBundle {
        archive_bytes,
        checksum_bytes,
    };
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_checksum_mismatches.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_archive_is_corrupted_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-corrupt-archive").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let archive_name = supported_archive_name();
    let archive_bytes = b"not-a-tar-gz".to_vec();
    let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
    let bundle = ReleaseBundle {
        archive_bytes,
        checksum_bytes: format!("{checksum}  {archive_name}\n").into_bytes(),
    };
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_archive_is_corrupted.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_fails_when_archive_has_no_acton_binary_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-archive-no-binary").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;
    let archive_name = supported_archive_name();
    let archive_bytes = archive_bytes_with_single_file("README.txt", "hello")?;
    let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
    let bundle = ReleaseBundle {
        archive_bytes,
        checksum_bytes: format!("{checksum}  {archive_name}\n").into_bytes(),
    };
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_fails_when_archive_has_no_acton_binary.stderr.txt",
        );

    Ok(())
}

#[test]
fn test_toolchain_install_lock_timeout_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-lock-timeout").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let store_dir = home.join(".acton/toolchains");
    fs::create_dir_all(&store_dir)?;
    let lock_path = store_dir.join(".0.4.0.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock.lock_exclusive()?;

    let output = toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env("ACTON_TEST_TOOLCHAIN_LOCK_TIMEOUT_MS", "1")
        .run()
        .failure();
    let _ = lock.unlock();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/toolchain/test_toolchain_install_lock_timeout.stderr.txt",
    );

    Ok(())
}

#[test]
fn test_toolchain_install_downloads_requested_release_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-download").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let fake_acton = version_probe_fake_acton("0.4.0", "1.4.0", "toolchain-acton-0.4.0");
    let bundle = release_bundle(&fake_acton)?;
    let mock = mock_release_server("0.4.0", &bundle);

    toolchain_command(&project, &home)
        .arg("install")
        .arg("0.4.0")
        .env(TEST_TOOLCHAIN_GITHUB_API_BASE_ENV, mock.base_url())
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_install_downloads_requested_release.stdout.txt",
        )
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/0.4.0/acton",
            "integration/snapshots/toolchain/test_toolchain_install_downloads_requested_release.binary.txt",
        )
        .assert_file_snapshot_matches(
            ".home/.acton/toolchains/0.4.0/release.json",
            "integration/snapshots/toolchain/test_toolchain_install_downloads_requested_release.release.json",
        );

    Ok(())
}

fn toolchain_command(project: &Project, home: &Path) -> crate::support::project::ActonCommand {
    let home = home.to_string_lossy().into_owned();
    project
        .acton()
        .current_dir(project.path())
        .env("HOME", &home)
        .arg("toolchain")
}

fn isolated_home(project: &Project) -> PathBuf {
    project.path().join(".home")
}

#[cfg(unix)]
fn assert_pty_transcript_snapshot_matches(
    project: &Project,
    transcript: &[u8],
    snapshot_path: &str,
) {
    let transcript = String::from_utf8_lossy(transcript);
    let normalized = crate::support::snapshots::normalize_output(&transcript, project.path());

    let mut snapshot_full_path = std::env::current_dir().expect("failed to get current dir");
    snapshot_full_path.push("tests");
    snapshot_full_path.push(snapshot_path);

    let expected = snapbox::Data::read_from(&snapshot_full_path, None);
    crate::common::assertion().eq(normalized, expected);
}

fn assert_project_file_snapshot_matches(project: &Project, file_path: &str, snapshot_path: &str) {
    let file_content = fs::read_to_string(project.path().join(file_path))
        .unwrap_or_else(|err| panic!("failed to read project file `{file_path}`: {err}"));
    let normalized = crate::support::snapshots::normalize_output(&file_content, project.path());

    let mut snapshot_full_path = std::env::current_dir().expect("failed to get current dir");
    snapshot_full_path.push("tests");
    snapshot_full_path.push(snapshot_path);

    let expected = snapbox::Data::read_from(&snapshot_full_path, None);
    crate::common::assertion().eq(normalized, expected);
}

fn write_toolchain_exists_snapshot(project: &Project, home: &Path, version: &str) -> Result<()> {
    let exists = home
        .join(".acton/toolchains")
        .join(version)
        .exists()
        .to_string();
    fs::write(
        project.path().join("toolchain-exists.txt"),
        format!("{exists}\n"),
    )?;
    Ok(())
}

fn project_with_toolchain(name: &str, toolchain: &str) -> Project {
    ProjectBuilder::new(name)
        .without_acton_toml()
        .raw_file(
            "Acton.toml",
            &format!(
                r#"[package]
name = "{name}"
description = "A test project"
version = "0.1.0"

[toolchain]
{toolchain}
"#
            ),
        )
        .build()
}

fn write_toolchain_index(home: &Path) -> Result<()> {
    write_toolchain_index_json(
        home,
        r#"{
  "schema": 1,
  "generated_at": "2026-04-24T00:00:00Z",
  "releases": [
    {
      "acton": "0.3.1",
      "tolk": "1.3.0",
      "tag": "v0.3.1",
      "stable": true,
      "yanked": false
    },
    {
      "acton": "0.4.0",
      "tolk": "1.4.0",
      "tag": "v0.4.0",
      "stable": true,
      "yanked": false
    }
  ]
}
"#,
    )
}

fn write_current_toolchain_index(home: &Path) -> Result<()> {
    write_toolchain_index_json(
        home,
        &json!({
            "schema": 1,
            "generated_at": "2026-04-24T00:00:00Z",
            "releases": [
                {
                    "acton": build_info::PACKAGE_VERSION,
                    "tolk": build_info::TOLK_VERSION,
                    "tag": format!("v{}", build_info::PACKAGE_VERSION),
                    "stable": true,
                    "yanked": false
                }
            ]
        })
        .to_string(),
    )
}

fn write_toolchain_index_json(home: &Path, index_json: &str) -> Result<()> {
    write_toolchain_index_json_with_meta(
        home,
        index_json,
        &chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "test-cache",
        "test",
        None,
        None,
    )
}

fn write_stale_toolchain_index_json(
    home: &Path,
    index_json: &str,
    source_repo: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<()> {
    write_toolchain_index_json_with_meta(
        home,
        index_json,
        "2020-01-01T00:00:00Z",
        source_repo,
        "test",
        etag,
        last_modified,
    )
}

fn write_toolchain_index_json_with_meta(
    home: &Path,
    index_json: &str,
    fetched_at: &str,
    source_repo: &str,
    source_ref: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<()> {
    let index_dir = home.join(".acton/toolchains");
    fs::create_dir_all(&index_dir)?;
    fs::write(index_dir.join("index.json"), index_json)?;
    let mut meta = json!({
        "schema": 1,
        "fetched_at": fetched_at,
        "source_repo": source_repo,
        "source_ref": source_ref
    });
    if let Some(etag) = etag {
        meta["etag"] = json!(etag);
    }
    if let Some(last_modified) = last_modified {
        meta["last_modified"] = json!(last_modified);
    }
    fs::write(index_dir.join("index-meta.json"), format!("{meta:#}\n"))?;
    Ok(())
}

fn write_installed_toolchain(
    home: &Path,
    acton: &str,
    tolk: &str,
    yanked: bool,
    yank_reason: Option<&str>,
    binary_contents: &str,
) -> Result<()> {
    let dir = home.join(format!(".acton/toolchains/{acton}"));
    fs::create_dir_all(&dir)?;
    let binary_path = dir.join(acton_binary_name());
    fs::write(&binary_path, binary_contents)?;
    set_executable_permissions(&binary_path)?;
    let mut metadata = json!({
        "schema": 1,
        "acton": acton,
        "tolk": tolk,
        "target_triple": build_info::TARGET_TRIPLE,
        "yanked": yanked
    });
    if let Some(reason) = yank_reason {
        metadata["yank_reason"] = json!(reason);
    }
    fs::write(dir.join("release.json"), format!("{metadata:#}\n"))?;
    Ok(())
}

fn remote_toolchain_index_json(acton: &str, tolk: &str) -> String {
    toolchain_index_json(vec![release_entry(acton, tolk, true, false)])
}

fn toolchain_index_json(releases: Vec<Value>) -> String {
    json!({
        "schema": 1,
        "generated_at": "2026-04-24T00:00:00Z",
        "releases": releases
    })
    .to_string()
}

fn release_entry(acton: &str, tolk: &str, stable: bool, yanked: bool) -> Value {
    let mut release = json!({
        "acton": acton,
        "tolk": tolk,
        "tag": format!("v{acton}"),
        "stable": stable,
        "yanked": yanked
    });
    if yanked {
        release["yank_reason"] = json!("broken release");
    }
    release
}

fn github_contents_response(content: &str) -> String {
    json!({
        "encoding": "base64",
        "content": base64::engine::general_purpose::STANDARD.encode(content)
    })
    .to_string()
}

fn supported_archive_name() -> String {
    format!("acton-{}.tar.gz", build_info::TARGET_TRIPLE)
}

fn acton_binary_name() -> &'static str {
    if cfg!(windows) { "acton.exe" } else { "acton" }
}

fn set_executable_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }

    Ok(())
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

fn version_probe_fake_acton(acton: &str, tolk: &str, stdout_line: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--toolchain-probe" ]; then
  cat <<'JSON'
{{"schema":1,"acton":"{acton}","tolk":"{tolk}","target_triple":"test-target"}}
JSON
  exit 0
fi
if [ "$1" = "-V" ] || [ "$1" = "--version" ]; then
  echo 'acton {acton} (test-git 2026-04-24) with Tolk {tolk}'
  exit 0
fi
echo '{stdout_line}'
"#
    )
}

fn recording_fake_acton(acton: &str, tolk: &str, stdout_line: &str) -> String {
    format!(
        r#"#!/bin/sh
if [ "$1" = "--toolchain-probe" ]; then
  cat <<'JSON'
{{"schema":1,"acton":"{acton}","tolk":"{tolk}","target_triple":"test-target"}}
JSON
  exit 0
fi
if [ "$1" = "-V" ] || [ "$1" = "--version" ]; then
  echo 'acton {acton} (test-git 2026-04-24) with Tolk {tolk}'
  exit 0
fi
{{
  echo "argv=$*"
  echo "requested_acton=$ACTON_TOOLCHAIN_REQUESTED_ACTON"
  echo "requested_tolk=$ACTON_TOOLCHAIN_REQUESTED_TOLK"
  echo "reexec_depth=$ACTON_TOOLCHAIN_REEXEC_DEPTH"
  echo "source=$ACTON_TOOLCHAIN_SOURCE"
  echo "cwd=$(pwd)"
}} > .toolchain-reexec.txt
echo '{stdout_line}'
"#
    )
}

fn mock_release_server(version: &str, bundle: &ReleaseBundle) -> GitHubMockServer {
    let archive_name = supported_archive_name();
    GitHubMockServer::spawn_with(|base_url| {
        let release = release_response(
            &format!("v{version}"),
            &mock_assets(
                base_url,
                version,
                &archive_name,
                bundle.archive_len(),
                bundle.checksum_len(),
            ),
        );
        vec![
            ExpectedHttpRequest::json(
                &format!("/repos/ton-blockchain/acton/releases/tags/v{version}"),
                release,
            ),
            ExpectedHttpRequest::binary(
                &format!("/download/{version}/{archive_name}"),
                bundle.archive_bytes.clone(),
            ),
            ExpectedHttpRequest::binary(
                &format!("/download/{version}/{archive_name}.sha256"),
                bundle.checksum_bytes.clone(),
            ),
        ]
    })
}

fn failing_toolchain_index_server() -> GitHubMockServer {
    GitHubMockServer::spawn_with(|_| {
        vec![
            ExpectedHttpRequest::empty(
                500,
                "/repos/i582/acton-public/contents/toolchain-index.json",
            ),
            ExpectedHttpRequest::empty(
                500,
                "/repos/ton-blockchain/acton/contents/toolchain-index.json",
            ),
        ]
    })
}

fn archive_bytes(binary_contents: &str) -> Result<Vec<u8>> {
    archive_bytes_with_single_file("acton", binary_contents)
}

fn archive_bytes_with_single_file(path: &str, contents: &str) -> Result<Vec<u8>> {
    let encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    let mut tar = tar::Builder::new(encoder);

    let mut header = tar::Header::new_gnu();
    header.set_path(path)?;
    header.set_size(contents.len() as u64);
    header.set_mode(0o755);
    header.set_mtime(0);
    header.set_cksum();
    tar.append(&header, contents.as_bytes())?;

    let encoder = tar.into_inner()?;
    Ok(encoder.finish()?)
}

struct ExpectedHttpRequest {
    status: u16,
    path: String,
    required_headers: Vec<(String, String)>,
    response_headers: Vec<(String, String)>,
    content_type: &'static str,
    body: Vec<u8>,
}

impl ExpectedHttpRequest {
    fn empty(status: u16, path: &str) -> Self {
        Self {
            status,
            path: path.to_owned(),
            required_headers: Vec::new(),
            response_headers: Vec::new(),
            content_type: "text/plain",
            body: Vec::new(),
        }
    }

    fn json(path: &str, body: impl Into<String>) -> Self {
        Self {
            status: 200,
            path: path.to_owned(),
            required_headers: Vec::new(),
            response_headers: Vec::new(),
            content_type: "application/json",
            body: body.into().into_bytes(),
        }
    }

    fn binary(path: &str, body: Vec<u8>) -> Self {
        Self {
            status: 200,
            path: path.to_owned(),
            required_headers: Vec::new(),
            response_headers: Vec::new(),
            content_type: "application/octet-stream",
            body,
        }
    }

    fn with_header(mut self, name: &str, value: &str) -> Self {
        self.required_headers
            .push((name.to_ascii_lowercase(), value.to_owned()));
        self
    }

    fn with_response_header(mut self, name: &str, value: &str) -> Self {
        self.response_headers
            .push((name.to_owned(), value.to_owned()));
        self
    }
}

struct GitHubMockServer {
    base_url: String,
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

        let handle = thread::spawn(move || {
            for expected in expected_requests {
                let wait_until = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(err) if err.kind() == ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() <= wait_until,
                                "timed out waiting for GitHub mock request: {}",
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
                    request.path, expected.path,
                    "unexpected request path for {}",
                    expected.path
                );
                for (name, value) in &expected.required_headers {
                    let actual = request
                        .headers
                        .iter()
                        .find(|(header_name, _)| header_name == name)
                        .map(|(_, header_value)| header_value.as_str());
                    assert_eq!(
                        actual,
                        Some(value.as_str()),
                        "unexpected `{name}` header for {}",
                        expected.path
                    );
                }

                write_http_response(
                    &mut stream,
                    expected.status,
                    expected.content_type,
                    &expected.response_headers,
                    &expected.body,
                );
            }
        });

        Self {
            base_url,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
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

struct RecordedHttpRequest {
    path: String,
    headers: Vec<(String, String)>,
}

fn read_http_request(stream: &std::net::TcpStream) -> RecordedHttpRequest {
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

    let mut headers = Vec::new();
    loop {
        let mut header_line = String::new();
        let read = reader
            .read_line(&mut header_line)
            .expect("failed to read GitHub mock header line");
        if read == 0 || header_line == "\r\n" {
            break;
        }
        if let Some((name, value)) = header_line.trim_end().split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    assert_eq!(method, "GET", "unexpected GitHub mock request method");
    RecordedHttpRequest {
        path: parts.next().unwrap_or_default().to_owned(),
        headers,
    }
}

fn write_http_response(
    stream: &mut std::net::TcpStream,
    status: u16,
    content_type: &str,
    response_headers: &[(String, String)],
    body: &[u8],
) {
    let status_text = match status {
        304 => "Not Modified",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let mut response_head = format!(
        "HTTP/1.1 {status} {status_text}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in response_headers {
        response_head.push_str(&format!("{name}: {value}\r\n"));
    }
    response_head.push_str("\r\n");

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
