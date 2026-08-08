use std::thread;
use std::time::{Duration, Instant};

use crate::support::project::ProjectBuilder;
use reqwest::blocking::Client;
use serde_json::{Value, json};

use super::StudioCliProcess;

const TEST_RUN_TIMEOUT: Duration = Duration::from_secs(20);

const FILTERED_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test selected by Studio`() {
    expect(1).toEqual(1);
}

get fun `test not selected by Studio`() {
    expect(1).toEqual(2);
}
"#;

const FAILING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test failure reported to Studio`() {
    expect(1).toEqual(2);
}
"#;

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test passes in Studio`() {
    expect(1).toEqual(1);
}
"#;

const REQUESTED_PATH_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test requested path`() {
    expect(1).toEqual(1);
}
"#;

const UNREQUESTED_PATH_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test unrequested path`() {
    expect(1).toEqual(2);
}
"#;

#[cfg(unix)]
#[test]
fn studio_api_runs_selected_tests_and_captures_output() {
    let project = ProjectBuilder::new("studio-api-filtered-run")
        .test_file("studio", FILTERED_TESTS)
        .build();
    let studio = StudioCliProcess::start(&project);
    let client = studio_client();

    let started = start_test_run(
        &client,
        studio.url(),
        json!({
            "filter": "^test selected by Studio$",
            "saveTraces": true,
        }),
    );
    let run_id = started["id"]
        .as_str()
        .expect("started Studio test run must have an ID");

    assert_eq!(started["source"], "studio");
    assert_eq!(started["status"], "running");
    assert_eq!(
        started["command"],
        json!([
            "acton",
            "test",
            "--filter",
            "^test selected by Studio$",
            "--save-test-trace",
            started["traceDir"]
        ])
    );

    let finished = wait_for_finished_run(&client, studio.url(), run_id);
    let reports = finished["reports"]
        .as_array()
        .expect("finished Studio test run must contain reports");

    assert_eq!(finished["status"], "passed");
    assert_eq!(finished["exitCode"], 0);
    assert_eq!(finished["stats"]["total"], 1);
    assert_eq!(finished["stats"]["passed"], 1);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["name"], "test selected by Studio");
    assert!(
        finished["traceDir"]
            .as_str()
            .is_some_and(|path| !path.is_empty())
    );

    let output = get_json(
        &client,
        &format!("{}/api/v1/test-runs/{run_id}/output", studio.url()),
    );
    assert!(
        output["stdout"]
            .as_str()
            .is_some_and(|stdout| stdout.contains("selected by Studio"))
    );
    assert_eq!(output["stderr"], "");

    let runs = get_json(&client, &format!("{}/api/v1/test-runs", studio.url()));
    assert!(
        runs.as_array()
            .expect("Studio test run list must be an array")
            .iter()
            .any(|run| run["id"] == run_id && run["status"] == "passed")
    );

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn studio_api_records_failed_test_runs() {
    let project = ProjectBuilder::new("studio-api-failed-run")
        .test_file("studio", FAILING_TEST)
        .build();
    let studio = StudioCliProcess::start(&project);
    let client = studio_client();

    let started = start_test_run(&client, studio.url(), json!({}));
    let run_id = started["id"]
        .as_str()
        .expect("started Studio test run must have an ID");
    let finished = wait_for_finished_run(&client, studio.url(), run_id);
    let reports = finished["reports"]
        .as_array()
        .expect("failed Studio test run must contain reports");

    assert_eq!(finished["status"], "failed");
    assert_eq!(finished["exitCode"], 1);
    assert_eq!(finished["stats"]["total"], 1);
    assert_eq!(finished["stats"]["failed"], 1);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["name"], "test failure reported to Studio");
    assert_eq!(reports[0]["status"], "Failed");
    assert!(
        reports[0]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );

    let output = get_json(
        &client,
        &format!("{}/api/v1/test-runs/{run_id}/output", studio.url()),
    );
    let stdout = output["stdout"]
        .as_str()
        .expect("captured Studio test stdout must be a string");
    let stderr = output["stderr"]
        .as_str()
        .expect("captured Studio test stderr must be a string");
    assert!(
        stdout.contains("failure reported to Studio"),
        "captured stdout:\n{stdout}\ncaptured stderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "captured stderr:\n{stderr}");

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn studio_api_cancels_a_running_test_process() {
    let project = ProjectBuilder::new("studio-api-cancelled-run")
        .test_file("studio", PASSING_TEST)
        .build();
    let studio = StudioCliProcess::start(&project);
    let client = studio_client();

    let started = start_test_run(&client, studio.url(), json!({}));
    let run_id = started["id"]
        .as_str()
        .expect("started Studio test run must have an ID");
    let cancel_response = client
        .post(format!("{}/api/v1/test-runs/{run_id}/cancel", studio.url()))
        .send()
        .expect("Studio test cancellation request must succeed");
    assert_eq!(cancel_response.status(), reqwest::StatusCode::OK);

    let finished = wait_for_finished_run(&client, studio.url(), run_id);
    assert_eq!(finished["status"], "cancelled");
    assert_eq!(finished["exitCode"], Value::Null);
    assert_eq!(finished["error"], "Test run was cancelled");

    let repeated_cancel = client
        .post(format!("{}/api/v1/test-runs/{run_id}/cancel", studio.url()))
        .send()
        .expect("repeated Studio test cancellation request must complete");
    assert_eq!(repeated_cancel.status(), reqwest::StatusCode::CONFLICT);
    let error: Value = repeated_cancel
        .json()
        .expect("Studio cancellation conflict must contain valid JSON");
    assert_eq!(error["error"]["code"], "test_run_not_cancellable");

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn studio_restores_test_history_after_restart() {
    let project = ProjectBuilder::new("studio-api-restored-history")
        .test_file("studio", PASSING_TEST)
        .build();
    let first_studio = StudioCliProcess::start(&project);
    let client = studio_client();

    let started = start_test_run(&client, first_studio.url(), json!({}));
    let run_id = started["id"]
        .as_str()
        .expect("started Studio test run must have an ID")
        .to_owned();
    let finished = wait_for_finished_run(&client, first_studio.url(), &run_id);
    assert_eq!(finished["status"], "passed");

    let output = first_studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let restarted_studio = StudioCliProcess::start(&project);
    let restored = get_json(
        &client,
        &format!("{}/api/v1/test-runs/{run_id}", restarted_studio.url()),
    );
    let runs = get_json(
        &client,
        &format!("{}/api/v1/test-runs", restarted_studio.url()),
    );

    assert_eq!(restored["id"], run_id);
    assert_eq!(restored["source"], "studio");
    assert_eq!(restored["status"], "passed");
    assert_eq!(restored["stats"]["total"], 1);
    assert_eq!(restored["reports"][0]["name"], "test passes in Studio");
    assert!(
        runs.as_array()
            .expect("restored Studio test run list must be an array")
            .iter()
            .any(|run| run["id"] == run_id && run["status"] == "passed")
    );

    let output = restarted_studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn studio_api_runs_only_the_requested_test_path() {
    let project = ProjectBuilder::new("studio-api-requested-path")
        .test_file("requested", REQUESTED_PATH_TEST)
        .test_file("unrequested", UNREQUESTED_PATH_TEST)
        .build();
    let studio = StudioCliProcess::start(&project);
    let client = studio_client();

    let started = start_test_run(
        &client,
        studio.url(),
        json!({"paths": ["tests/requested.test.tolk"]}),
    );
    let run_id = started["id"]
        .as_str()
        .expect("started Studio test run must have an ID");
    assert_eq!(
        started["command"],
        json!(["acton", "test", "tests/requested.test.tolk"])
    );

    let finished = wait_for_finished_run(&client, studio.url(), run_id);
    let reports = finished["reports"]
        .as_array()
        .expect("path-selected Studio test run must contain reports");
    assert_eq!(finished["status"], "passed");
    assert_eq!(finished["stats"]["total"], 1);
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0]["name"], "test requested path");

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn studio_api_rejects_empty_test_path_and_filter() {
    let project = ProjectBuilder::new("studio-api-invalid-run").build();
    let studio = StudioCliProcess::start(&project);
    let client = studio_client();

    for (request, expected_code, expected_message) in [
        (
            json!({"paths": [" "]}),
            "invalid_test_path",
            "Test paths cannot be empty",
        ),
        (
            json!({"filter": " "}),
            "invalid_test_filter",
            "Test filter cannot be empty",
        ),
    ] {
        let response = client
            .post(format!("{}/api/v1/test-runs", studio.url()))
            .json(&request)
            .send()
            .expect("invalid Studio test run request must complete");
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        let error: Value = response
            .json()
            .expect("Studio validation error must contain valid JSON");
        assert_eq!(error["error"]["code"], expected_code);
        assert_eq!(error["error"]["message"], expected_message);
    }

    let runs = get_json(&client, &format!("{}/api/v1/test-runs", studio.url()));
    assert_eq!(runs, json!([]));

    let output = studio.stop();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

fn studio_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("Studio test HTTP client must build")
}

fn start_test_run(client: &Client, studio_url: &str, request: Value) -> Value {
    let response = client
        .post(format!("{studio_url}/api/v1/test-runs"))
        .json(&request)
        .send()
        .expect("Studio test run request must succeed");
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    response
        .json()
        .expect("Studio test run response must contain valid JSON")
}

fn wait_for_finished_run(client: &Client, studio_url: &str, run_id: &str) -> Value {
    let deadline = Instant::now() + TEST_RUN_TIMEOUT;
    loop {
        let run = get_json(client, &format!("{studio_url}/api/v1/test-runs/{run_id}"));
        if matches!(
            run["status"].as_str(),
            Some("passed" | "failed" | "cancelled")
        ) {
            return run;
        }
        assert!(
            Instant::now() < deadline,
            "Studio test run {run_id} did not finish"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn get_json(client: &Client, url: &str) -> Value {
    let response = client.get(url).send().expect("Studio request must succeed");
    assert!(
        response.status().is_success(),
        "Studio request to {url} failed with {}",
        response.status()
    );
    response
        .json()
        .expect("Studio response must contain valid JSON")
}
