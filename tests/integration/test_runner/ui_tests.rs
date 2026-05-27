use crate::common::{acton_exe, acton_path_env, assertion};
use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use reqwest::StatusCode;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const UI_DEPLOY_TEST_TEMPLATE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"

get fun `__TEST_NAME__`() {
    val init = ContractState {
        code: build("__CONTRACT_NAME__"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployer = testing.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    });

    val txs = net.send(deployer.address, msg);
    expect(txs).toHaveSuccessfulDeploy({ to: address });
}
"#;

fn ui_deploy_test_source(test_name: &str) -> String {
    ui_deploy_test_source_for_contract(test_name, "simple")
}

fn ui_deploy_test_source_for_contract(test_name: &str, contract_name: &str) -> String {
    UI_DEPLOY_TEST_TEMPLATE
        .replace("__TEST_NAME__", test_name)
        .replace("__CONTRACT_NAME__", contract_name)
}

fn reserve_ui_port() -> (Option<TcpListener>, String) {
    // Preferred mode: occupy an ephemeral loopback port so `acton test --ui`
    // always fails fast with deterministic "address already in use".
    if let Ok(listener) = TcpListener::bind("127.0.0.1:0") {
        let port = listener
            .local_addr()
            .expect("Reserved TCP port has no address")
            .port()
            .to_string();
        return (Some(listener), port);
    }

    // Sandbox fallback: local socket bind may be prohibited.
    // Use a privileged port to keep `--ui` failure deterministic.
    (None, "1".to_string())
}

fn append_acton_toml(project: &Project, content: &str) {
    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("should read generated Acton.toml");
    acton_toml.push_str(content);
    fs::write(&acton_toml_path, acton_toml).expect("should update generated Acton.toml");
}

fn write_acton_toml(project: &Project, content: &str) {
    fs::write(project.path().join("Acton.toml"), content).expect("should write Acton.toml");
}

fn unused_ui_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("should reserve unused UI port")
        .local_addr()
        .expect("reserved UI port has no local address")
        .port()
}

struct UiTestProcess {
    child: Option<Child>,
}

impl Drop for UiTestProcess {
    fn drop(&mut self) {
        let Some(child) = &mut self.child else {
            return;
        };
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn_test_ui(project: &Project, port: u16) -> UiTestProcess {
    let port = port.to_string();
    spawn_test_ui_with_args(project, ["test", "--ui", "--ui-port", &port])
}

fn spawn_test_ui_with_args<'a>(
    project: &Project,
    args: impl IntoIterator<Item = &'a str>,
) -> UiTestProcess {
    let mut command = Command::new(acton_exe());
    command
        .current_dir(project.path())
        .env("PATH", acton_path_env())
        .env("HOME", project.isolated_home())
        .env("USERPROFILE", project.isolated_home())
        .env("ACTON_LOG_DIR", project.path().join(".acton-test-logs"))
        .env("NO_COLOR", "1")
        .env("ACTON_INTERNAL_SKIP_BROWSER", "1")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    UiTestProcess {
        child: Some(command.spawn().expect("should spawn acton test --ui")),
    }
}

fn wait_for_test_ui(process: &mut UiTestProcess, base_url: &str) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(300))
        .build()
        .expect("should create HTTP client");
    let deadline = Instant::now() + Duration::from_secs(15);

    loop {
        if client
            .get(format!("{base_url}/api/health"))
            .send()
            .is_ok_and(|response| response.status().is_success())
        {
            return;
        }

        let child = process
            .child
            .as_mut()
            .expect("acton test --ui process should be active");
        if child
            .try_wait()
            .expect("should inspect acton test --ui status")
            .is_some()
        {
            let output = process
                .child
                .take()
                .expect("acton test --ui process should be available")
                .wait_with_output()
                .expect("should collect acton test --ui output");
            panic!(
                "acton test --ui exited before API became available: {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for Test UI API at {base_url}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

fn assert_ui_api_snapshot(summary: String, snapshot_path: &str) {
    let mut path = std::env::current_dir().expect("Failed to get current dir");
    path.push("tests");
    path.push(snapshot_path);
    assertion().eq(summary, snapbox::Data::read_from(&path, None));
}

fn encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[test]
fn ui_bind_failure_is_reported_before_tests_run_and_skips_default_traces() {
    let project = ProjectBuilder::new("f-ui-default-trace")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-default-trace"))
        .build();

    let (listener, port) = reserve_ui_port();

    let output = project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .run()
        .failure();

    output
        .assert_not_contains("Starting UI server at")
        .assert_stderr_contains("Failed to start UI server on 127.0.0.1:")
        .assert_stderr_contains("Choose another port with --ui-port")
        .assert_stderr_contains("Or stop the process currently listening on that port");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_bind_failure.stderr.txt",
        );
    }

    let trace_dir = project.path().join("build/traces");
    assert!(
        !trace_dir.exists(),
        "UI bind failure must happen before default trace creation: {}",
        trace_dir.display()
    );
}

#[test]
fn ui_api_serves_contract_metadata_with_path_like_display_name() {
    let project = ProjectBuilder::new("f-ui-contract-display-name-path")
        .raw_file("contracts/simple/contract.tolk", SIMPLE_CONTRACT)
        .test_file(
            "ui",
            &ui_deploy_test_source_for_contract("test-ui-contract-path", "simple/contract"),
        )
        .build();
    write_acton_toml(
        &project,
        r#"[package]
name = "f-ui-contract-display-name-path"
description = "A test project"
version = "0.1.0"

[contracts."simple/contract"]
display-name = "Pool/Wallet"
src = "contracts/simple/contract.tolk"
depends = []
"#,
    );

    let port = unused_ui_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut process = spawn_test_ui(&project, port);
    wait_for_test_ui(&mut process, &base_url);

    let client = reqwest::blocking::Client::new();
    let reports: Value = client
        .get(format!("{base_url}/api/reports"))
        .send()
        .expect("should fetch UI reports")
        .json()
        .expect("reports response should be JSON");
    let trace_path = reports[0]["trace_path"]
        .as_str()
        .expect("UI report should include trace path");
    let trace: Value = client
        .get(format!(
            "{base_url}/api/trace/{}",
            encode_component(trace_path)
        ))
        .send()
        .expect("should fetch UI trace")
        .json()
        .expect("trace response should be JSON");
    let contract_name = trace["contracts"]
        .as_array()
        .expect("trace should include contracts")
        .iter()
        .find_map(Value::as_str)
        .expect("trace should include contract name");
    let contract_response = client
        .get(format!(
            "{base_url}/api/contract/{}",
            encode_component(contract_name)
        ))
        .send()
        .expect("should fetch UI contract metadata");
    let contract_status = contract_response.status();
    let contract: Value = contract_response
        .json()
        .expect("contract response should be JSON");

    assert_eq!(contract_status, StatusCode::OK);
    assert_ui_api_snapshot(
        format!(
            "trace_path: {trace_path}\ncontract_name: {contract_name}\ncontract_status: {contract_status}\ncontract_json_name: {}\ncontract_json_display_name: {}\n",
            contract["name"].as_str().unwrap_or("<missing>"),
            contract["display_name"].as_str().unwrap_or("<missing>")
        ),
        "integration/snapshots/test-runner/test_runner_ui/ui_api_serves_contract_metadata_with_path_like_display_name.txt",
    );
}

#[test]
fn ui_api_returns_no_content_for_missing_or_empty_trace_file() {
    let project = ProjectBuilder::new("f-ui-missing-trace-file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-missing-trace-file"))
        .build();

    let port = unused_ui_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut process = spawn_test_ui(&project, port);
    wait_for_test_ui(&mut process, &base_url);

    let client = reqwest::blocking::Client::new();
    let reports: Value = client
        .get(format!("{base_url}/api/reports"))
        .send()
        .expect("should fetch UI reports")
        .json()
        .expect("reports response should be JSON");
    let trace_path = reports[0]["trace_path"]
        .as_str()
        .expect("UI report should include trace path");
    let trace_file = project.path().join("build/traces").join(trace_path);
    fs::remove_file(&trace_file).expect("should remove generated trace file");

    let missing_trace_response = client
        .get(format!(
            "{base_url}/api/trace/{}",
            encode_component(trace_path)
        ))
        .send()
        .expect("should fetch missing UI trace response");
    let missing_trace_status = missing_trace_response.status();
    let missing_trace_body = missing_trace_response
        .text()
        .expect("missing trace response body should be readable");
    fs::write(&trace_file, "").expect("should write empty trace file");

    let empty_trace_response = client
        .get(format!(
            "{base_url}/api/trace/{}",
            encode_component(trace_path)
        ))
        .send()
        .expect("should fetch empty UI trace response");
    let empty_trace_status = empty_trace_response.status();
    let empty_trace_body = empty_trace_response
        .text()
        .expect("empty trace response body should be readable");

    assert_eq!(missing_trace_status, StatusCode::NO_CONTENT);
    assert_eq!(empty_trace_status, StatusCode::NO_CONTENT);
    assert_ui_api_snapshot(
        format!(
            "missing_trace_status: {missing_trace_status}\nmissing_trace_body: {missing_trace_body:?}\nempty_trace_status: {empty_trace_status}\nempty_trace_body: {empty_trace_body:?}\n"
        ),
        "integration/snapshots/test-runner/test_runner_ui/ui_api_returns_no_content_for_missing_or_empty_trace_file.txt",
    );
}

#[test]
fn ui_trace_dir_starts_test_ui_from_saved_traces_without_running_tests() {
    let project = ProjectBuilder::new("f-ui-saved-trace-dir").build();
    let trace_dir = project.path().join("saved-traces");
    fs::create_dir_all(&trace_dir).expect("should create saved trace dir");
    let source_path = project.path().join("tests/manual.test.tolk");
    fs::create_dir_all(
        source_path
            .parent()
            .expect("source path should have parent"),
    )
    .expect("should create tests dir");
    fs::write(&source_path, "// saved trace source\n").expect("should write source file");
    fs::write(
        trace_dir.join("manual_trace_trace.json"),
        serde_json::json!({
            "name": "manual saved trace",
            "pos": {
                "uri": source_path.to_string_lossy(),
                "row": 7,
                "column": 3,
            },
            "traces": [
                {
                    "name": "Manual chain",
                    "transactions": [],
                    "failed_messages": [],
                },
            ],
            "contracts": [],
            "wallets": {},
        })
        .to_string(),
    )
    .expect("should write saved trace file");

    let port = unused_ui_port();
    let port_arg = port.to_string();
    let trace_dir_arg = trace_dir.to_string_lossy().to_string();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut process = spawn_test_ui_with_args(
        &project,
        [
            "test",
            "--ui-trace-dir",
            &trace_dir_arg,
            "--ui-port",
            &port_arg,
        ],
    );
    wait_for_test_ui(&mut process, &base_url);

    let client = reqwest::blocking::Client::new();
    let reports: Value = client
        .get(format!("{base_url}/api/reports"))
        .send()
        .expect("should fetch UI reports")
        .json()
        .expect("reports response should be JSON");
    let trace_path = reports[0]["trace_path"]
        .as_str()
        .expect("UI report should include trace path");
    let trace: Value = client
        .get(format!(
            "{base_url}/api/trace/{}",
            encode_component(trace_path)
        ))
        .send()
        .expect("should fetch saved UI trace")
        .json()
        .expect("saved trace response should be JSON");

    assert_ui_api_snapshot(
        format!(
            "reports: {}\nreport_name: {}\nreport_suite: {}\nreport_status: {}\ntrace_path: {}\ntrace_name: {}\ntrace_count: {}\n",
            reports.as_array().map_or(0, Vec::len),
            reports[0]["name"].as_str().unwrap_or("<missing>"),
            reports[0]["suite_name"].as_str().unwrap_or("<missing>"),
            reports[0]["status"].as_str().unwrap_or("<missing>"),
            trace_path,
            trace["name"].as_str().unwrap_or("<missing>"),
            trace["traces"].as_array().map_or(0, Vec::len),
        ),
        "integration/snapshots/test-runner/test_runner_ui/ui_trace_dir_starts_test_ui_from_saved_traces_without_running_tests.txt",
    );
}

#[test]
fn ui_port_config_is_used_when_cli_port_is_absent() {
    let project = ProjectBuilder::new("f-ui-config-port")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-config-port"))
        .build();

    let (listener, port) = reserve_ui_port();
    append_acton_toml(
        &project,
        &format!(
            r"
[test]
ui = true
ui-port = {port}
"
        ),
    );

    let output = project.acton().test().run().failure();

    output
        .assert_not_contains("Starting UI server at")
        .assert_stderr_contains("Failed to start UI server on 127.0.0.1:")
        .assert_stderr_contains("Choose another port with --ui-port")
        .assert_stderr_contains("Or stop the process currently listening on that port");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_bind_failure.stderr.txt",
        );
    }
}

#[test]
fn ui_bind_failure_skips_custom_trace_output() {
    let project = ProjectBuilder::new("f-ui-custom-trace")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-custom-trace"))
        .build();

    let (listener, port) = reserve_ui_port();

    let output = project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .arg("--save-test-trace")
        .arg("custom-traces")
        .run()
        .failure();

    output
        .assert_stderr_contains("Failed to start UI server on 127.0.0.1:")
        .assert_stderr_contains("Choose another port with --ui-port")
        .assert_stderr_contains("Or stop the process currently listening on that port");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_bind_failure.stderr.txt",
        );
    }

    let default_trace = project.path().join("build/traces");
    assert!(
        !default_trace.exists(),
        "Default UI trace directory must not be created when bind fails: {}",
        default_trace.display()
    );

    let custom_trace = project.path().join("custom-traces");
    assert!(
        !custom_trace.exists(),
        "Custom UI trace directory must not be created when bind fails: {}",
        custom_trace.display()
    );
}
