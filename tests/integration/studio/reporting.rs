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
fn studio_keeps_coverage_and_gas_profiles_for_each_run_after_restart() {
    let project = ProjectBuilder::new("studio-viewer-artifacts")
        .test_file("reporting", PASSING_TEST)
        .build();
    let studio = StudioCliProcess::start(&project);
    let client = reqwest::blocking::Client::new();

    // Viewer data must not depend on the export format or reusable export paths.
    for (run_id, coverage_format, gas_format) in [
        ("lcov-profile", "lcov", "cpuprofile"),
        ("text-profile", "text", "collapsed"),
    ] {
        project
            .acton()
            .current_dir(project.path())
            .env("ACTON_STUDIO_RUN_ID", run_id)
            .args([
                "test",
                "--coverage",
                "--coverage-include-tests",
                "--coverage-format",
                coverage_format,
                "--coverage-file",
                "coverage-export",
                "--gas-profile",
                "gas-export",
                "--gas-profile-format",
                gas_format,
                "--gas-profile-include-tests",
            ])
            .run()
            .success()
            .assert_passed(1);

        let base = format!("{}/api/v1/test-runs/{run_id}/artifacts", studio.url());
        let config: Value = client
            .get(format!("{base}/config"))
            .send()
            .unwrap()
            .json()
            .unwrap();
        let coverage = client.get(format!("{base}/coverage.lcov")).send().unwrap();
        let coverage_status = coverage.status();
        let coverage = coverage.text().unwrap();
        let profile = client.get(format!("{base}/gas-profile")).send().unwrap();
        let profile_status = profile.status();
        let profile: Value = profile.json().unwrap();
        expect_test::expect![[r#"
            coverage available: true
            profile available: true
            coverage status: 200 OK
            profile status: 200 OK
            coverage includes test source: true
            total gas positive: true
            test: "test auto-discovered Studio reporting"
        "#]].assert_eq(&format!(
            "coverage available: {}\nprofile available: {}\ncoverage status: {coverage_status}\nprofile status: {profile_status}\ncoverage includes test source: {}\ntotal gas positive: {}\ntest: {}\n",
            config["coverage_available"], config["gas_profile_available"],
            coverage.contains("reporting.test.tolk"),
            profile["total_gas"].as_u64().is_some_and(|gas| gas > 0),
            profile["tests"][0]["name"],
        ));
    }

    fs::remove_file(project.path().join("coverage-export")).unwrap();
    fs::remove_file(project.path().join("gas-export")).unwrap();
    project
        .acton()
        .env("ACTON_STUDIO_RUN_ID", "no-artifacts")
        .test()
        .run()
        .success()
        .assert_passed(1);

    fs::write(
        project.path().join("tests/reporting.test.tolk"),
        PASSING_TEST.replace("toEqual(1)", "toEqual(2)"),
    )
    .unwrap();
    project
        .acton()
        .current_dir(project.path())
        .env("ACTON_STUDIO_RUN_ID", "failed-test")
        .args([
            "test",
            "--coverage",
            "--coverage-include-tests",
            "--gas-profile",
            "gas-export",
        ])
        .run()
        .failure()
        .assert_failed(1);
    studio.stop();

    let studio = StudioCliProcess::start(&project);
    let mut observed = Vec::new();
    for run_id in [
        "lcov-profile",
        "text-profile",
        "no-artifacts",
        "failed-test",
        "unknown-run",
    ] {
        let base = format!("{}/api/v1/test-runs/{run_id}/artifacts", studio.url());
        let coverage = client.get(format!("{base}/coverage.lcov")).send().unwrap();
        let profile = client.get(format!("{base}/gas-profile")).send().unwrap();
        observed.push(format!(
            "{run_id}: coverage {}, profile {}\n",
            coverage.status(),
            profile.status()
        ));
        if run_id == "no-artifacts" || run_id == "failed-test" {
            let config: Value = client
                .get(format!("{base}/config"))
                .send()
                .unwrap()
                .json()
                .unwrap();
            observed.push(format!(
                "{run_id} availability: {}, {}\n",
                config["coverage_available"], config["gas_profile_available"]
            ));
        }
    }
    expect_test::expect![[r"
        lcov-profile: coverage 200 OK, profile 200 OK
        text-profile: coverage 200 OK, profile 200 OK
        no-artifacts: coverage 204 No Content, profile 204 No Content
        no-artifacts availability: false, false
        failed-test: coverage 200 OK, profile 204 No Content
        failed-test availability: true, false
        unknown-run: coverage 404 Not Found, profile 404 Not Found
    "]]
    .assert_eq(&observed.concat());
    studio.stop();
}

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

    studio.stop();
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

    studio.stop();
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
