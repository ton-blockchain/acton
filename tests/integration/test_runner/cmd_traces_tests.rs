use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::PathBuf;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const TRACE_TEST_PREPARE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/emulation/tracing"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage() {
    val init = ContractState {
        code: build("simple"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init };
}

fun deployCounter() {
    val counter = Counter.fromStorage();
    val deployer = net.treasury("deployer");
    val deployMsg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });

    val deployTxs = net.send(deployer.address, deployMsg);
    expect(deployTxs.size()).toEqual(1);

    val sender = net.treasury("sender");
    val ping = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: counter.address,
    });

    val pingTxs = net.send(sender.address, ping);
    expect(pingTxs.size()).toEqual(1);
}
"#;

fn trace_project(name: &str, test_cases: &str) -> crate::support::project::Project {
    let source = format!("{TRACE_TEST_PREPARE}\n{test_cases}");
    ProjectBuilder::new(name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("trace", &source)
        .build()
}

fn read_json_from_project(
    project: &crate::support::project::Project,
    relative_path: &str,
) -> serde_json::Value {
    let full_path = project.path().join(relative_path);
    let content = fs::read_to_string(&full_path)
        .unwrap_or_else(|e| panic!("Failed to read JSON file {}: {}", full_path.display(), e));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse JSON file {}: {}", full_path.display(), e))
}

fn assert_trace_json_contract(
    project: &crate::support::project::Project,
    relative_path: &str,
    expected_test_name: &str,
) {
    let trace = read_json_from_project(project, relative_path);

    assert_eq!(
        trace["name"].as_str(),
        Some(expected_test_name),
        "Unexpected trace test name in {relative_path}"
    );
    let uri = trace["pos"]["uri"]
        .as_str()
        .unwrap_or_else(|| panic!("Missing trace source URI in {relative_path}"));
    let actual_uri = dunce::canonicalize(uri).unwrap_or_else(|_| PathBuf::from(uri));
    let expected_uri = dunce::canonicalize(project.path().join("tests/trace.test.tolk"))
        .unwrap_or_else(|_| project.path().join("tests/trace.test.tolk"));
    assert_eq!(
        actual_uri, expected_uri,
        "Unexpected trace source URI in {relative_path}: {uri}"
    );

    let contracts = trace["contracts"]
        .as_array()
        .unwrap_or_else(|| panic!("Missing contracts array in {relative_path}"));
    assert!(
        contracts.iter().any(|c| c.as_str() == Some("simple")),
        "Expected `simple` contract in contracts list for {relative_path}"
    );

    let traces = trace["traces"]
        .as_array()
        .unwrap_or_else(|| panic!("Missing traces array in {relative_path}"));
    assert!(
        !traces.is_empty(),
        "Expected at least one trace chain in {relative_path}"
    );

    let mut has_dest_contract_info = false;
    for chain in traces {
        let transactions = chain["transactions"]
            .as_array()
            .unwrap_or_else(|| panic!("Missing transactions list in {relative_path}"));
        assert!(
            !transactions.is_empty(),
            "Expected non-empty transactions in {relative_path}"
        );

        for tx in transactions {
            assert!(
                tx["lt"].as_str().is_some(),
                "Missing transaction lt in {relative_path}"
            );
            assert!(
                tx["raw_transaction"].as_str().is_some(),
                "Missing raw_transaction in {relative_path}"
            );
            assert!(
                tx["vm_log_diff"].as_str().is_some(),
                "Missing vm_log_diff in {relative_path}"
            );
            assert!(
                tx["executor_logs"].as_str().is_some(),
                "Missing executor_logs in {relative_path}"
            );
            assert!(
                tx["executor_actions"].is_array() || tx["executor_actions"].is_null(),
                "executor_actions must be an array or absent in {relative_path}"
            );
            if tx["dest_contract_info"].as_str() == Some("simple") {
                has_dest_contract_info = true;
            }
        }
    }

    assert!(
        has_dest_contract_info,
        "Expected at least one transaction with dest_contract_info=simple in {relative_path}"
    );

    let wallets = trace["wallets"]
        .as_object()
        .unwrap_or_else(|| panic!("Missing wallets object in {relative_path}"));
    assert!(
        wallets
            .values()
            .any(|name| name.as_str() == Some("deployer")),
        "Expected deployer wallet in {relative_path}"
    );
    assert!(
        wallets.values().any(|name| name.as_str() == Some("sender")),
        "Expected sender wallet in {relative_path}"
    );
}

#[test]
fn save_test_trace_without_path_uses_default_directory() {
    let project = trace_project(
        "h-save-trace-default-dir",
        r#"
        get fun `test-default-trace`() {
            deployCounter();
        }
        "#,
    );

    let output = project
        .acton()
        .test()
        .arg("--save-test-trace")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_without_path_uses_default_directory.stdout.txt",
        )
        .assert_file_exists(".acton/traces/test-default-trace_trace.json")
        .assert_file_exists(".acton/traces/contracts/simple.json")
        .assert_file_snapshot_matches(
            ".acton/traces/contracts/simple.json",
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_without_path_uses_default_directory.contract.txt",
        );

    assert_trace_json_contract(
        &project,
        ".acton/traces/test-default-trace_trace.json",
        "test-default-trace",
    );
}

#[test]
fn save_test_trace_with_custom_directory_uses_regular_non_ui_flow() {
    let project = trace_project(
        "h-save-trace-custom-dir",
        r#"
        get fun `test-custom-trace`() {
            deployCounter();
        }
        "#,
    );

    let output = project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("custom-traces")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_not_contains("UI")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_with_custom_directory_uses_regular_non_ui_flow.stdout.txt",
        )
        .assert_file_exists("custom-traces/test-custom-trace_trace.json")
        .assert_file_exists("custom-traces/contracts/simple.json");

    assert_trace_json_contract(
        &project,
        "custom-traces/test-custom-trace_trace.json",
        "test-custom-trace",
    );

    let default_trace_dir = project.path().join(".acton/traces");
    assert!(
        !default_trace_dir.exists(),
        "Default trace dir should not be created for custom trace path: {}",
        default_trace_dir.display()
    );
}

#[test]
fn save_test_trace_creates_trace_per_test_and_single_contract_file() {
    let project = trace_project(
        "h-save-trace-multi",
        r#"
        get fun `test-trace-first`() {
            deployCounter();
        }

        get fun `test-trace-second`() {
            deployCounter();
        }
        "#,
    );

    let output = project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("trace-multi")
        .run()
        .success();

    output
        .assert_passed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_creates_trace_per_test_and_single_contract_file.stdout.txt",
        )
        .assert_file_exists("trace-multi/test-trace-first_trace.json")
        .assert_file_exists("trace-multi/test-trace-second_trace.json")
        .assert_file_exists("trace-multi/contracts/simple.json");

    assert_trace_json_contract(
        &project,
        "trace-multi/test-trace-first_trace.json",
        "test-trace-first",
    );
    assert_trace_json_contract(
        &project,
        "trace-multi/test-trace-second_trace.json",
        "test-trace-second",
    );

    let contracts_dir = project.path().join("trace-multi/contracts");
    let contract_json_files = fs::read_dir(&contracts_dir)
        .unwrap_or_else(|e| {
            panic!(
                "Failed to read contracts trace dir {}: {}",
                contracts_dir.display(),
                e
            )
        })
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .count();

    assert_eq!(
        contract_json_files,
        1,
        "Expected exactly one contract trace file in {}",
        contracts_dir.display()
    );
}

#[test]
fn save_test_trace_keeps_custom_trace_names() {
    let project = trace_project(
        "h-save-trace-custom-names",
        r#"
        get fun `test-custom-trace-names`() {
            val counter = Counter.fromStorage();
            val deployer = net.treasury("deployer");
            val deployMsg = createMessage({
                bounce: false,
                value: ton("1.0"),
                dest: {
                    stateInit: counter.init,
                },
            });

            val deployTxs = net.send(deployer.address, deployMsg);
            tracing.save(deployTxs, "deploy-counter");
            expect(deployTxs.size()).toEqual(1);

            val sender = net.treasury("sender");
            val ping = createMessage({
                bounce: false,
                value: ton("0.2"),
                dest: counter.address,
            });

            val pingTxs = net.send(sender.address, ping);
            tracing.save(pingTxs, "ping-counter");
            expect(pingTxs.size()).toEqual(1);
        }
        "#,
    );

    let output = project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("trace-custom-names")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_file_exists("trace-custom-names/test-custom-trace-names_trace.json")
        .assert_file_exists("trace-custom-names/contracts/simple.json");

    assert_trace_json_contract(
        &project,
        "trace-custom-names/test-custom-trace-names_trace.json",
        "test-custom-trace-names",
    );

    let trace = read_json_from_project(
        &project,
        "trace-custom-names/test-custom-trace-names_trace.json",
    );
    let traces = trace["traces"]
        .as_array()
        .unwrap_or_else(|| panic!("Missing traces array in custom names trace json"));
    let trace_names = traces
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect::<Vec<_>>();

    assert!(
        trace_names.contains(&"deploy-counter"),
        "Expected custom name `deploy-counter` in trace names: {:?}",
        trace_names
    );
    assert!(
        trace_names.contains(&"ping-counter"),
        "Expected custom name `ping-counter` in trace names: {:?}",
        trace_names
    );
}

#[test]
fn regular_run_without_trace_flag_does_not_create_trace_artifacts() {
    let project = trace_project(
        "h-regular-run-no-trace",
        r#"
        get fun `test-no-trace`() {
            deployCounter();
        }
        "#,
    );

    let output = project.acton().test().run().success();

    output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/regular_run_without_trace_flag_does_not_create_trace_artifacts.stdout.txt",
        );

    let trace_dir = project.path().join(".acton/traces");
    assert!(
        !trace_dir.exists(),
        "Trace dir should not exist without --save-test-trace: {}",
        trace_dir.display()
    );
}

#[test]
fn save_test_trace_can_be_enabled_after_regular_run() {
    let project = trace_project(
        "h-trace-after-regular-run",
        r#"
        get fun `test-after-regular`() {
            deployCounter();
        }
        "#,
    );

    let regular_output = project.acton().test().run().success();
    regular_output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_can_be_enabled_after_regular_run.regular.stdout.txt",
        );

    let default_trace_dir = project.path().join(".acton/traces");
    assert!(
        !default_trace_dir.exists(),
        "Trace dir should not exist after regular run: {}",
        default_trace_dir.display()
    );

    let traced_output = project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("trace-after-regular")
        .run()
        .success();

    traced_output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_can_be_enabled_after_regular_run.trace.stdout.txt",
        )
        .assert_file_exists("trace-after-regular/test-after-regular_trace.json")
        .assert_file_exists("trace-after-regular/contracts/simple.json");

    assert_trace_json_contract(
        &project,
        "trace-after-regular/test-after-regular_trace.json",
        "test-after-regular",
    );
}
