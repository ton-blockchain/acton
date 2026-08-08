use std::fs;

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_studio::{
    STUDIO_API_VERSION, StudioDaemonDescriptor, load_studio_daemon_descriptor,
    persist_studio_daemon_descriptor,
};
use serde_json::Value;

use super::{StudioCliProcess, reserve_studio_port};

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test auto-discovered Studio reporting`() {
    expect(1).toEqual(1);
}
"#;

#[cfg(unix)]
#[test]
fn running_studio_is_discovered_by_acton_test() {
    let project = ProjectBuilder::new("studio-auto-reporting")
        .test_file("reporting", PASSING_TEST)
        .build();
    let studio = StudioCliProcess::start(&project);

    project
        .acton()
        .env_remove("ACTON_STUDIO_URL")
        .env_remove("ACTON_STUDIO_RUN_SOURCE")
        .env("ACTON_STUDIO_RUN_ID", "auto-discovered-run")
        .test()
        .run()
        .success()
        .assert_passed(1);

    let run_path = project
        .path()
        .join(".studio/tests/runs/auto-discovered-run.json");
    let run: Value = serde_json::from_slice(
        &fs::read(&run_path).expect("auto-discovered Studio run must be persisted"),
    )
    .expect("persisted Studio run must contain valid JSON");
    let reports = run["reports"]
        .as_array()
        .expect("persisted Studio run must contain reports");

    assert_eq!(run["source"], "manual");
    assert_eq!(run["status"], "passed");
    assert_eq!(run["exitCode"], 0);
    assert_eq!(run["stats"]["total"], 1);
    assert_eq!(run["stats"]["passed"], 1);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["name"], "test auto-discovered Studio reporting");
    assert!(
        run["traceDir"]
            .as_str()
            .is_some_and(|path| !path.is_empty())
    );

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn studio_reporting_rejects_a_descriptor_for_another_workspace() {
    let studio_project = ProjectBuilder::new("studio-reporting-owner").build();
    let test_project = ProjectBuilder::new("studio-reporting-other")
        .test_file("reporting", PASSING_TEST)
        .build();
    let studio = StudioCliProcess::start(&studio_project);

    let descriptor = load_studio_daemon_descriptor(studio_project.path())
        .expect("running Studio descriptor must be readable")
        .expect("running Studio must publish a descriptor");
    persist_studio_daemon_descriptor(test_project.path(), &descriptor)
        .expect("mismatched descriptor must be written for the test");

    test_project
        .acton()
        .env_remove("ACTON_STUDIO_URL")
        .env_remove("ACTON_STUDIO_RUN_SOURCE")
        .env("ACTON_STUDIO_RUN_ID", "mismatched-workspace")
        .test()
        .run()
        .success()
        .assert_passed(1);

    assert!(
        !test_project
            .path()
            .join(".studio/tests/runs/mismatched-workspace.json")
            .exists()
    );
    assert!(
        !test_project
            .path()
            .join(".studio/tests/traces/mismatched-workspace")
            .exists()
    );
    assert!(
        !studio_project
            .path()
            .join(".studio/tests/runs/mismatched-workspace.json")
            .exists()
    );

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[test]
fn studio_reporting_ignores_a_stale_descriptor() {
    let project = ProjectBuilder::new("studio-stale-descriptor")
        .test_file("reporting", PASSING_TEST)
        .build();
    let (listener, port) = reserve_studio_port();
    drop(listener);
    persist_studio_daemon_descriptor(
        project.path(),
        &StudioDaemonDescriptor {
            protocol_version: STUDIO_API_VERSION,
            url: format!("http://127.0.0.1:{port}"),
            pid: u32::MAX,
        },
    )
    .expect("stale Studio descriptor must be written for the test");

    project
        .acton()
        .env_remove("ACTON_STUDIO_URL")
        .env_remove("ACTON_STUDIO_RUN_SOURCE")
        .env("ACTON_STUDIO_RUN_ID", "stale-descriptor")
        .test()
        .run()
        .success()
        .assert_passed(1);

    assert!(
        !project
            .path()
            .join(".studio/tests/runs/stale-descriptor.json")
            .exists()
    );
    assert!(
        !project
            .path()
            .join(".studio/tests/traces/stale-descriptor")
            .exists()
    );
}
