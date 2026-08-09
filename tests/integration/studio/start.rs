use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_studio::load_studio_daemon_descriptor;

use super::{StudioCliProcess, reserve_studio_port};

#[test]
fn studio_start_port_conflict_is_reported_with_hint() {
    let project = ProjectBuilder::new("studio-port-conflict").build();
    let (_listener, port) = reserve_studio_port();
    let port = port.to_string();

    project
        .acton()
        .current_dir(project.path())
        .arg("studio")
        .arg("start")
        .arg("--port")
        .arg(&port)
        .arg("--no-open")
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_contains("Failed to start Acton Studio on 127.0.0.1:")
        .assert_stderr_contains("Set another port with --port")
        .assert_stderr_contains("Or stop the process currently listening on that port")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_port_conflict.stderr.txt",
        );
}

#[test]
fn studio_start_rejects_non_loopback_host() {
    let project = ProjectBuilder::new("studio-public-host").build();

    project
        .acton()
        .current_dir(project.path())
        .args(["studio", "start", "--host", "0.0.0.0", "--no-open"])
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_rejects_non_loopback_host.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn studio_start_serves_workspace_and_registers_for_reporting() {
    let project = ProjectBuilder::new("studio-cli-start").build();
    let mut studio = StudioCliProcess::start(&project);
    let studio_pid = studio.id();

    let info = studio.wait_for_info();
    let workspace = info
        .workspace
        .expect("Studio started in a project must publish workspace info");
    let descriptor = load_studio_daemon_descriptor(project.path())
        .expect("Studio daemon descriptor must be readable")
        .expect("Studio CLI must register itself for test reporting");

    assert_eq!(info.protocol_version, 1);
    assert!(!info.server_version.is_empty());
    assert_eq!(workspace.name, "studio-cli-start");
    assert!(workspace.wallet_names.is_empty());
    assert_eq!(descriptor.url, studio.url());
    assert_eq!(descriptor.pid, studio_pid);

    let output = studio.stop();
    assert!(output.status.success());
    assert!(
        load_studio_daemon_descriptor(project.path())
            .expect("Studio daemon descriptor must remain readable after shutdown")
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn studio_start_rejects_a_second_instance_for_the_same_project() {
    let project = ProjectBuilder::new("studio-duplicate-instance").build();
    let mut first = StudioCliProcess::start(&project);

    let (second_listener, second_port) = reserve_studio_port();
    drop(second_listener);
    let second_port_arg = second_port.to_string();
    project
        .acton()
        .current_dir(project.path())
        .args(["studio", "start", "--port", &second_port_arg, "--no-open"])
        .run()
        .failure()
        .assert_not_contains("Starting Acton Studio")
        .assert_stderr_contains("Another Acton Studio instance is already running")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/studio/studio_start_rejects_duplicate_instance.stderr.txt",
        );

    let info = first.wait_for_info();
    assert_eq!(
        info.workspace
            .expect("first Studio instance must remain available")
            .name,
        "studio-duplicate-instance"
    );
    assert!(first.stop().status.success());
}

#[cfg(unix)]
#[test]
fn studio_start_works_without_an_acton_manifest() {
    let project = ProjectBuilder::new("studio-without-manifest")
        .without_acton_toml()
        .build();
    let mut studio = StudioCliProcess::start(&project);

    let info = studio.wait_for_info();
    assert!(info.workspace.is_none());
    let descriptor = load_studio_daemon_descriptor(project.path())
        .expect("standalone Studio descriptor must be readable")
        .expect("standalone Studio must publish its descriptor");
    assert_eq!(descriptor.url, studio.url());
    assert_eq!(descriptor.pid, studio.id());

    let output = studio.stop();
    assert!(output.status.success());
    assert!(
        load_studio_daemon_descriptor(project.path())
            .expect("standalone Studio descriptor must remain readable after shutdown")
            .is_none()
    );
}
