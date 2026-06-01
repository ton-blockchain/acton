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

const GAS_PROFILED_DEEP_STACK_CONTRACT: &str = r"
@noinline
fun profileLeaf(seed: int): int {
    var acc = seed;
    repeat (5) {
        acc += seed;
        acc *= 2;
    }
    return acc;
}

@noinline
fun profileLevelFour(seed: int): int {
    return profileLeaf(seed + 4);
}

@noinline
fun profileLevelThree(seed: int): int {
    return profileLevelFour(seed + 3);
}

@noinline
fun profileLevelTwo(seed: int): int {
    return profileLevelThree(seed + 2);
}

@noinline
fun profileLevelOne(seed: int): int {
    return profileLevelTwo(seed + 1);
}

fun onInternalMessage(_: InMessage) {
    val result = profileLevelOne(1);
    if (result == 0) {
        throw 701;
    }
}

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

const GAS_PROFILED_DEEP_STACK_TEST: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/types/big_array"

get fun `test ui gas profile deep stack`() {
    val init = ContractState {
        code: build("deep"),
        data: createEmptyCell(),
    };

    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    val deployer = testing.treasury("deployer");

    expect(net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: init,
        },
    })).size()).toEqual(1);

    expect(net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: address,
    })).size()).toEqual(1);
}
"#;

const GAS_PROFILED_UNIT_HELPER_TEST: &str = r#"
import "@acton/testing/expect"

struct X {
    seed: int
}

fun X.create(): X {
    return X { seed: 17 };
}

@noinline
fun X.mix(self, value: int): int {
    return (value + self.seed) * 3;
}

@noinline
fun X.heavyJob(self): int {
    var acc = self.seed;
    repeat (8) {
        acc = self.mix(acc);
    }
    return acc;
}

get fun `test ui gas profile heavy unit helper`() {
    val x = X.create();
    val result = x.heavyJob();
    expect(result).toNotEqual(0);
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
    spawn_test_ui_with_args(project, port, &[])
}

fn spawn_test_ui_with_args(project: &Project, port: u16, extra_args: &[&str]) -> UiTestProcess {
    let mut command = Command::new(acton_exe());
    command
        .current_dir(project.path())
        .env("PATH", acton_path_env())
        .env("HOME", project.isolated_home())
        .env("USERPROFILE", project.isolated_home())
        .env("ACTON_LOG_DIR", project.path().join(".acton-test-logs"))
        .env("NO_COLOR", "1")
        .env("ACTON_INTERNAL_SKIP_BROWSER", "1")
        .args(["test", "--ui", "--ui-port", &port.to_string()])
        .args(extra_args)
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
fn ui_api_serves_gas_profile_when_enabled() {
    let project = ProjectBuilder::new("f-ui-gas-profile")
        .contract("deep", GAS_PROFILED_DEEP_STACK_CONTRACT)
        .test_file("profile", GAS_PROFILED_DEEP_STACK_TEST)
        .build();

    let port = unused_ui_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut process = spawn_test_ui_with_args(&project, port, &["--gas-profile", "gas.cpuprofile"]);
    wait_for_test_ui(&mut process, &base_url);

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("{base_url}/api/gas-profile"))
        .send()
        .expect("should fetch UI gas profile");
    let status = response.status();
    let profile: Value = response
        .json()
        .expect("gas profile response should be JSON");
    let contracts = profile["contracts"]
        .as_array()
        .expect("gas profile should include contracts");
    let first_contract = contracts
        .first()
        .expect("gas profile should include a contract");
    let samples = first_contract["samples"]
        .as_array()
        .expect("gas profile contract should include samples");
    let frame_names = samples
        .iter()
        .flat_map(|sample| sample["frames"].as_array().into_iter().flatten())
        .filter_map(|frame| frame["function_name"].as_str())
        .collect::<Vec<_>>();
    let test_profiles = profile["tests"]
        .as_array()
        .expect("gas profile should include per-test profiles");
    let first_test_profile = test_profiles
        .first()
        .expect("gas profile should include a test profile");
    let test_contracts = first_test_profile["contracts"]
        .as_array()
        .expect("test gas profile should include contracts");
    let first_test_contract = test_contracts
        .first()
        .expect("test gas profile should include a contract");
    let test_frame_names = first_test_contract["samples"]
        .as_array()
        .expect("test gas profile contract should include samples")
        .iter()
        .flat_map(|sample| sample["frames"].as_array().into_iter().flatten())
        .filter_map(|frame| frame["function_name"].as_str())
        .collect::<Vec<_>>();

    assert_ui_api_snapshot(
        format!(
            "status: {status}\ntotal_gas_positive: {}\ncontracts: {}\nfirst_contract_name: {}\nfirst_contract_gas_positive: {}\nfirst_contract_sample_count_positive: {}\nframes_include_entrypoint: {}\nframes_include_prefixed_entrypoint: {}\nframes_include_level_one: {}\nframes_include_leaf: {}\ntest_profiles: {}\nfirst_test_name: {}\nfirst_test_gas_positive: {}\nfirst_test_contracts: {}\nfirst_test_contract_name: {}\ntest_frames_include_entrypoint: {}\ntest_frames_include_level_one: {}\ntest_frames_include_leaf: {}\nprofile_file_exists: {}\n",
            profile["total_gas"].as_u64().is_some_and(|gas| gas > 0),
            contracts.len(),
            first_contract["name"].as_str().unwrap_or("<missing>"),
            first_contract["total_gas"]
                .as_u64()
                .is_some_and(|gas| gas > 0),
            first_contract["sample_count"]
                .as_u64()
                .is_some_and(|samples| samples > 0),
            frame_names.contains(&"onInternalMessage"),
            frame_names.contains(&"deep:onInternalMessage"),
            frame_names.contains(&"profileLevelOne"),
            frame_names.contains(&"profileLeaf"),
            test_profiles.len(),
            first_test_profile["name"].as_str().unwrap_or("<missing>"),
            first_test_profile["total_gas"]
                .as_u64()
                .is_some_and(|gas| gas > 0),
            test_contracts.len(),
            first_test_contract["name"].as_str().unwrap_or("<missing>"),
            test_frame_names.contains(&"onInternalMessage"),
            test_frame_names.contains(&"profileLevelOne"),
            test_frame_names.contains(&"profileLeaf"),
            project.path().join("gas.cpuprofile").exists(),
        ),
        "integration/snapshots/test-runner/test_runner_ui/ui_api_serves_gas_profile_when_enabled.txt",
    );
}

#[test]
fn ui_api_serves_unit_profile_when_include_tests_enabled() {
    let project = ProjectBuilder::new("f-ui-gas-profile-include-tests")
        .contract("simple", SIMPLE_CONTRACT)
        .mapping("@acton", "../lib")
        .test_file("profile", GAS_PROFILED_UNIT_HELPER_TEST)
        .build();

    let port = unused_ui_port();
    let base_url = format!("http://127.0.0.1:{port}");
    let mut process = spawn_test_ui_with_args(
        &project,
        port,
        &[
            "--gas-profile",
            "gas.cpuprofile",
            "--gas-profile-include-tests",
        ],
    );
    wait_for_test_ui(&mut process, &base_url);

    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("{base_url}/api/gas-profile"))
        .send()
        .expect("should fetch UI gas profile");
    let status = response.status();
    let profile: Value = response
        .json()
        .expect("gas profile response should be JSON");
    let test_profiles = profile["tests"]
        .as_array()
        .expect("gas profile should include per-test profiles");
    let unit_test_profile = test_profiles
        .iter()
        .find(|test| test["name"].as_str() == Some("test ui gas profile heavy unit helper"))
        .expect("gas profile should include the unit test profile");
    let test_contracts = unit_test_profile["contracts"]
        .as_array()
        .expect("unit test gas profile should include contracts");
    let tests_contract = test_contracts
        .iter()
        .find(|contract| contract["name"].as_str() == Some("Tests"))
        .expect("unit test gas profile should include the Tests contract group");
    let frame_names = tests_contract["samples"]
        .as_array()
        .expect("Tests contract should include samples")
        .iter()
        .flat_map(|sample| sample["frames"].as_array().into_iter().flatten())
        .filter_map(|frame| frame["function_name"].as_str())
        .collect::<Vec<_>>();
    let frame_urls = tests_contract["samples"]
        .as_array()
        .expect("Tests contract should include samples")
        .iter()
        .flat_map(|sample| sample["frames"].as_array().into_iter().flatten())
        .filter_map(|frame| frame["url"].as_str())
        .collect::<Vec<_>>();
    let acton_mapping_root = project
        .path()
        .parent()
        .expect("project path should have a temp root")
        .join("lib")
        .to_string_lossy()
        .replace('\\', "/");
    let has_acton_runtime_source = frame_urls.iter().any(|url| {
        let url = url.replace('\\', "/");
        url.contains("@acton/")
            || url.contains("/.acton/")
            || url == acton_mapping_root
            || url
                .strip_prefix(&acton_mapping_root)
                .is_some_and(|rest| rest.starts_with('/'))
    });

    assert_ui_api_snapshot(
        format!(
            "status: {status}\ntotal_gas_positive: {}\ntest_profiles: {}\nunit_test_name: {}\nunit_test_gas_positive: {}\nunit_test_contracts: {}\ntests_contract_gas_positive: {}\nframes_include_test_method: {}\nframes_include_create: {}\nframes_include_heavy_job: {}\nframes_include_mix: {}\nframes_include_acton_runtime_source: {}\nprofile_file_exists: {}\n",
            profile["total_gas"].as_u64().is_some_and(|gas| gas > 0),
            test_profiles.len(),
            unit_test_profile["name"].as_str().unwrap_or("<missing>"),
            unit_test_profile["total_gas"]
                .as_u64()
                .is_some_and(|gas| gas > 0),
            test_contracts.len(),
            tests_contract["total_gas"]
                .as_u64()
                .is_some_and(|gas| gas > 0),
            frame_names.contains(&"test ui gas profile heavy unit helper"),
            frame_names.contains(&"X.create"),
            frame_names.contains(&"X.heavyJob"),
            frame_names.contains(&"X.mix"),
            has_acton_runtime_source,
            project.path().join("gas.cpuprofile").exists(),
        ),
        "integration/snapshots/test-runner/test_runner_ui/ui_api_serves_unit_test_gas_profile_when_include_tests_enabled.txt",
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
