use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use acton::build_info;
use anyhow::Result;
use flate2::{Compression, GzBuilder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
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
fn test_toolchain_list_empty_home_snapshot() {
    let project = ProjectBuilder::new("toolchain-list-empty").build();
    let home = isolated_home(&project);

    toolchain_command(&project, &home)
        .arg("list")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/toolchain/test_toolchain_list_empty_home.stdout.txt",
        );
}

#[test]
fn test_toolchain_install_current_version_noop_snapshot() {
    let project = ProjectBuilder::new("toolchain-install-current").build();
    let home = isolated_home(&project);

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

    let fake_acton = recording_fake_acton("fake acton reexec complete");
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

    let fake_acton = recording_fake_acton("ci fake acton reexec complete");
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
fn test_toolchain_install_downloads_requested_release_snapshot() -> Result<()> {
    let project = ProjectBuilder::new("toolchain-install-download").build();
    let home = isolated_home(&project);
    write_toolchain_index(&home)?;

    let bundle = release_bundle("toolchain-acton-0.4.0")?;
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
    let index_dir = home.join(".acton/toolchains");
    fs::create_dir_all(&index_dir)?;
    fs::write(
        index_dir.join("index.json"),
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
    )?;
    Ok(())
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

fn recording_fake_acton(stdout_line: &str) -> String {
    format!(
        r#"#!/bin/sh
{{
  printf 'argv=%s\n' "$*"
  printf 'requested_acton=%s\n' "$ACTON_TOOLCHAIN_REQUESTED_ACTON"
  printf 'requested_tolk=%s\n' "$ACTON_TOOLCHAIN_REQUESTED_TOLK"
  printf 'reexec_depth=%s\n' "$ACTON_TOOLCHAIN_REEXEC_DEPTH"
  printf 'source=%s\n' "$ACTON_TOOLCHAIN_SOURCE"
  printf 'cwd=%s\n' "$(pwd)"
}} > .toolchain-reexec.txt
printf '{stdout_line}\n'
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

struct ExpectedHttpRequest {
    path: String,
    content_type: &'static str,
    body: Vec<u8>,
}

impl ExpectedHttpRequest {
    fn json(path: &str, body: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            content_type: "application/json",
            body: body.into().into_bytes(),
        }
    }

    fn binary(path: &str, body: Vec<u8>) -> Self {
        Self {
            path: path.to_owned(),
            content_type: "application/octet-stream",
            body,
        }
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

                let request_path = read_http_request_path(&stream);
                assert_eq!(
                    request_path, expected.path,
                    "unexpected request path for {}",
                    expected.path
                );

                write_http_response(&mut stream, expected.content_type, &expected.body);
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

fn read_http_request_path(stream: &std::net::TcpStream) -> String {
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

    loop {
        let mut header_line = String::new();
        let read = reader
            .read_line(&mut header_line)
            .expect("failed to read GitHub mock header line");
        if read == 0 || header_line == "\r\n" {
            break;
        }
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    assert_eq!(method, "GET", "unexpected GitHub mock request method");
    parts.next().unwrap_or_default().to_owned()
}

fn write_http_response(stream: &mut std::net::TcpStream, content_type: &str, body: &[u8]) {
    let response_head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
