use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::PathBuf;
use tycho_types::boc::Boc;
use tycho_types::models::{IntAddr, MsgInfo, Transaction};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const STEP_TRACE_MESSAGES: &str = r"
struct (0x3101f001) TriggerForward {
    queryId: uint64
    target: address
}

struct (0x3101f002) Notify {
    queryId: uint64
}
";

const STEP_TRACE_FORWARDER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerForward.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.target,
        body: Notify {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const STEP_TRACE_RECEIVER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy Notify.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const TRACE_TEST_PREPARE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/types/big_array"

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
    val deployer = testing.treasury("deployer");
    val deployMsg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });

    val deployTxs = net.send(deployer.address, deployMsg);
    expect(deployTxs.size()).toEqual(1);

    val sender = testing.treasury("sender");
    val ping = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: counter.address,
    });

    val pingTxs = net.send(sender.address, ping);
    expect(pingTxs.size()).toEqual(1);
}
"#;

const GENERATED_POOL_MESSAGES: &str = r"
struct GeneratedPoolStorage {
    parent: address
}

struct GeneratedPositionStorage {
    owner: address
}

struct (0x52a10001) DeployGeneratedPool {
    queryId: uint64
    parent: address
}

struct (0x52a10002) GeneratedPoolPing {
    queryId: uint64
}
";

const GENERATED_FACTORY_CONTRACT: &str = r#"
import "generated_pool_messages"
import "@gen/Pool.code"

contract Factory {
    incomingMessages: DeployGeneratedPool
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy DeployGeneratedPool.fromSlice(in.body);
    val poolInit = ContractState {
        code: poolCompiledCode(),
        data: GeneratedPoolStorage { parent: msg.parent }.toCell(),
    };

    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: { stateInit: poolInit },
        body: GeneratedPoolPing { queryId: msg.queryId }.toCell(),
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const GENERATED_POOL_CONTRACT: &str = r#"
import "generated_pool_messages"

contract Pool {
    storage: GeneratedPoolStorage
    incomingMessages: GeneratedPoolPing
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy GeneratedPoolPing.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const GENERATED_POSITION_CONTRACT: &str = r#"
import "generated_pool_messages"

contract Position {
    storage: GeneratedPositionStorage
}

fun onInternalMessage(_: InMessage) {}

fun onBouncedMessage(_: InMessageBounced) {}
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

fn assert_trace_summary_snapshot(summary: String, snapshot_path: &str) {
    let mut path = std::env::current_dir().expect("Failed to get current dir");
    path.push("tests");
    path.push(snapshot_path);
    assertion().eq(summary, snapbox::Data::read_from(&path, None));
}

fn string_list(value: &serde_json::Value) -> Vec<String> {
    value.as_array().map_or_else(Vec::new, |values| {
        values
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(ToString::to_string)
            .collect()
    })
}

fn replace_contract_display_name(project: &crate::support::project::Project, from: &str, to: &str) {
    let acton_toml_path = project.path().join("Acton.toml");
    let acton_toml =
        fs::read_to_string(&acton_toml_path).expect("should read generated Acton.toml");
    let updated = acton_toml.replace(
        &format!("display-name = \"{from}\""),
        &format!("display-name = \"{to}\""),
    );
    fs::write(&acton_toml_path, updated).expect("should update generated Acton.toml");
}

fn trace_root_wallet_name(
    trace_json: &serde_json::Value,
    trace_chain: &serde_json::Value,
) -> Option<String> {
    let raw_transaction =
        trace_chain["transactions"].as_array()?.first()?["raw_transaction"].as_str()?;
    let transaction = Boc::decode_base64(raw_transaction)
        .ok()?
        .parse::<Transaction>()
        .ok()?;
    let in_msg = transaction.load_in_msg().ok()??;
    let MsgInfo::Int(info) = in_msg.info else {
        return None;
    };
    let IntAddr::Std(src) = info.src else {
        return None;
    };
    let src_key = src.display_base64_url(true).to_string();

    trace_json["wallets"]
        .get(&src_key)?
        .as_str()
        .map(ToString::to_string)
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
fn save_test_trace_recognizes_contract_deployed_from_generated_code() {
    let project = ProjectBuilder::new("h-save-trace-generated-code")
        .mapping("gen", "gen")
        .file(
            "contracts/generated_pool_messages",
            GENERATED_POOL_MESSAGES,
        )
        .contract("Position", GENERATED_POSITION_CONTRACT)
        .contract_with_deps("Pool", GENERATED_POOL_CONTRACT, vec!["Position"])
        .contract_with_deps("Factory", GENERATED_FACTORY_CONTRACT, vec!["Pool", "Position"])
        .test_file(
            "trace",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../contracts/generated_pool_messages"
            import "@gen/Pool.code"

            get fun `test-generated-code-pool-trace`() {
                val deployer = testing.treasury("deployer");
                val factoryInit = ContractState {
                    code: build("Factory"),
                    data: createEmptyCell(),
                };
                val factoryAddress = AutoDeployAddress { stateInit: factoryInit }.calculateAddress();

                val deployFactory = createMessage({
                    bounce: false,
                    value: ton("1.0"),
                    dest: { stateInit: factoryInit },
                });
                expect(net.send(deployer.address, deployFactory)).toHaveSuccessfulDeploy({
                    to: factoryAddress,
                });

                val poolInit = ContractState {
                    code: poolCompiledCode(),
                    data: GeneratedPoolStorage { parent: factoryAddress }.toCell(),
                };
                val poolAddress = AutoDeployAddress { stateInit: poolInit }.calculateAddress();
                val deployPool = createMessage({
                    bounce: false,
                    value: ton("0.4"),
                    dest: factoryAddress,
                    body: DeployGeneratedPool { queryId: 1, parent: factoryAddress }.toCell(),
                });

                val txs = net.send(deployer.address, deployPool);
                expect(txs).toHaveSuccessfulTx<DeployGeneratedPool>({ to: factoryAddress });
                expect(txs).toHaveSuccessfulDeploy({ to: poolAddress });
                expect(txs).toHaveSuccessfulTx<GeneratedPoolPing>({ to: poolAddress });
            }
            "#,
        )
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let acton_toml =
        fs::read_to_string(&acton_toml_path).expect("should read generated Acton.toml");
    let acton_toml = acton_toml
        .replace("[contracts.factory]", "[contracts.Factory]")
        .replace("[contracts.pool]", "[contracts.Pool]")
        .replace("[contracts.position]", "[contracts.Position]");
    fs::write(&acton_toml_path, acton_toml).expect("should update generated Acton.toml");

    project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("trace-generated")
        .run()
        .success()
        .assert_passed(1);

    let trace = read_json_from_project(
        &project,
        "trace-generated/test-generated-code-pool-trace_trace.json",
    );
    let factory_contract =
        read_json_from_project(&project, "trace-generated/contracts/Factory.json");
    let pool_contract = read_json_from_project(&project, "trace-generated/contracts/Pool.json");

    let mut contract_files = fs::read_dir(project.path().join("trace-generated/contracts"))
        .expect("should read generated trace contracts directory")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    contract_files.sort();

    let tx_dest_contracts = trace["traces"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|chain| chain["transactions"].as_array().into_iter().flatten())
        .map(|tx| {
            tx["dest_contract_info"]
                .as_str()
                .unwrap_or("<unknown>")
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_trace_summary_snapshot(
        format!(
            "trace_contracts: {}\ncontract_files: {}\ntx_dest_contracts: {}\nfactory_json_name: {}\npool_json_name: {}\npool_abi_contract_name: {}\npool_storage_ty_idx: {}\npool_incoming_messages: {}\n",
            string_list(&trace["contracts"]).join(","),
            contract_files.join(","),
            tx_dest_contracts.join(" -> "),
            factory_contract["name"].as_str().unwrap_or("<missing>"),
            pool_contract["name"].as_str().unwrap_or("<missing>"),
            pool_contract["abi"]["contract_name"]
                .as_str()
                .unwrap_or("<missing>"),
            pool_contract["abi"]["storage"]["storage_ty_idx"]
                .as_i64()
                .map_or_else(|| "<missing>".to_string(), |value| value.to_string()),
            pool_contract["abi"]["incoming_messages"]
                .as_array()
                .map_or(0, Vec::len),
        ),
        "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_recognizes_contract_deployed_from_generated_code.txt",
    );
}

#[test]
fn save_test_trace_without_path_uses_default_directory() {
    let project = trace_project(
        "h-save-trace-default-dir",
        r"
        get fun `test-default-trace`() {
            deployCounter();
        }
        ",
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
        .assert_file_exists("build/traces/test-default-trace_trace.json")
        .assert_file_exists("build/traces/contracts/simple.json")
        .assert_file_snapshot_matches(
            "build/traces/contracts/simple.json",
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_without_path_uses_default_directory.contract.txt",
        );

    assert_trace_json_contract(
        &project,
        "build/traces/test-default-trace_trace.json",
        "test-default-trace",
    );
}

#[test]
fn save_test_trace_reports_missing_emulations_to_test_stderr() {
    let project = trace_project(
        "h-save-trace-empty-test-stderr",
        r"
        get fun `test-empty-trace-warning`() {
            expect(1).toEqual(1);
        }
        ",
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
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_reports_missing_emulations_to_test_stderr.stdout.txt",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_reports_missing_emulations_to_test_stderr.stderr.txt",
        );

    assert!(
        !project
            .path()
            .join("build/traces/test-empty-trace-warning_trace.json")
            .exists(),
        "trace file should not be written when the test recorded no emulated transactions"
    );
}

#[test]
fn save_test_trace_sanitizes_test_names_for_trace_file_paths() {
    let project = trace_project(
        "h-save-trace-name-with-slash",
        r"
        get fun `test trace/name with slash`() {
            deployCounter();
        }
        ",
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
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_sanitizes_test_names_for_trace_file_paths.stdout.txt",
        )
        .assert_file_exists("build/traces/test_trace_name_with_slash_trace.json");

    assert_trace_json_contract(
        &project,
        "build/traces/test_trace_name_with_slash_trace.json",
        "test trace/name with slash",
    );
}

#[test]
fn save_test_trace_with_custom_directory_uses_regular_non_ui_flow() {
    let project = trace_project(
        "h-save-trace-custom-dir",
        r"
        get fun `test-custom-trace`() {
            deployCounter();
        }
        ",
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

    let default_trace_dir = project.path().join("build/traces");
    assert!(
        !default_trace_dir.exists(),
        "Default trace dir should not be created for custom trace path: {}",
        default_trace_dir.display()
    );
}

#[test]
fn save_test_trace_custom_directory_keeps_display_name_separate_from_contract_name() {
    let project = trace_project(
        "h-save-trace-display-name",
        r"
        get fun `test-display-name-trace`() {
            deployCounter();
        }
        ",
    );
    replace_contract_display_name(&project, "simple", "Pool/Wallet");

    project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("custom-traces")
        .run()
        .success()
        .assert_passed(1)
        .assert_file_exists("custom-traces/test-display-name-trace_trace.json")
        .assert_file_exists("custom-traces/contracts/simple.json");

    let trace =
        read_json_from_project(&project, "custom-traces/test-display-name-trace_trace.json");
    let contract = read_json_from_project(&project, "custom-traces/contracts/simple.json");
    assert_trace_summary_snapshot(
        format!(
            "trace_contracts: {}\ncontract_json_name: {}\ncontract_json_display_name: {}\n",
            trace["contracts"]
                .as_array()
                .and_then(|contracts| contracts.first())
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing>"),
            contract["name"].as_str().unwrap_or("<missing>"),
            contract["display_name"].as_str().unwrap_or("<missing>")
        ),
        "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_custom_directory_keeps_display_name_separate_from_contract_name.txt",
    );
}

#[test]
fn save_test_trace_creates_trace_per_test_and_single_contract_file() {
    let project = trace_project(
        "h-save-trace-multi",
        r"
        get fun `test-trace-first`() {
            deployCounter();
        }

        get fun `test-trace-second`() {
            deployCounter();
        }
        ",
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
            val deployer = testing.treasury("deployer");
            val deployMsg = createMessage({
                bounce: false,
                value: ton("1.0"),
                dest: {
                    stateInit: counter.init,
                },
            });

            val deployTxs = net.send(deployer.address, deployMsg);
            deployTxs.giveName("deploy-counter");
            expect(deployTxs.size()).toEqual(1);

            val sender = testing.treasury("sender");
            val ping = createMessage({
                bounce: false,
                value: ton("0.2"),
                dest: counter.address,
            });

            val pingTxs = net.send(sender.address, ping);
            pingTxs.giveName("ping-counter");
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
        "Expected custom name `deploy-counter` in trace names: {trace_names:?}"
    );
    assert!(
        trace_names.contains(&"ping-counter"),
        "Expected custom name `ping-counter` in trace names: {trace_names:?}"
    );

    let deploy_trace = traces
        .iter()
        .find(|trace| trace["name"].as_str() == Some("deploy-counter"))
        .unwrap_or_else(|| panic!("Missing deploy-counter trace in custom names trace json"));
    assert_eq!(
        trace_root_wallet_name(&trace, deploy_trace).as_deref(),
        Some("deployer"),
        "deploy-counter must stay attached to the deploy chain"
    );

    let ping_trace = traces
        .iter()
        .find(|trace| trace["name"].as_str() == Some("ping-counter"))
        .unwrap_or_else(|| panic!("Missing ping-counter trace in custom names trace json"));
    assert_eq!(
        trace_root_wallet_name(&trace, ping_trace).as_deref(),
        Some("sender"),
        "ping-counter must stay attached to the ping chain"
    );
}

#[test]
fn save_test_trace_merges_step_execution_batches_into_single_named_trace() {
    let project = ProjectBuilder::new("h-save-trace-step-iter-merge")
        .file("contracts/messages", STEP_TRACE_MESSAGES)
        .contract("forwarder", STEP_TRACE_FORWARDER_CONTRACT)
        .contract("receiver", STEP_TRACE_RECEIVER_CONTRACT)
        .test_file(
            "trace",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build"
            import "../../lib/emulation/network"
import "../../lib/emulation/testing"
            import "../../lib/emulation/network"
            import "../contracts/messages"

            get fun `test-step-trace-merge`() {
                val sender = testing.treasury("sender");

                val forwarderInit = ContractState {
                    code: build("forwarder"),
                    data: createEmptyCell(),
                };
                val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

                val receiverInit = ContractState {
                    code: build("receiver"),
                    data: createEmptyCell(),
                };
                val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

                expect(net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: { stateInit: forwarderInit },
                }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

                expect(net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: { stateInit: receiverInit },
                }))).toHaveSuccessfulDeploy({ to: receiverAddress });

                val iter = testing.createTraceIterationCursor(sender.address, createMessage({
                    bounce: false,
                    value: ton("0.5"),
                    dest: forwarderAddress,
                    body: TriggerForward {
                        queryId: 33,
                        target: receiverAddress,
                    },
                }));

                val first = iter.executeN(1);
                expect(first).toHaveLength(1);
                expect(first).toHaveSuccessfulTx<TriggerForward>({
                    from: sender.address,
                    to: forwarderAddress,
                });

                val tail = iter.executeAllRemaining();
                expect(tail).toHaveLength(1);
                expect(tail).toHaveSuccessfulTx<Notify>({
                    from: forwarderAddress,
                    to: receiverAddress,
                });

                tail.giveName("step-forward-trace");
            }
            "#,
        )
        .build();

    let output = project
        .acton()
        .test()
        .arg("--save-test-trace")
        .arg("trace-step-iter")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_merges_step_execution_batches_into_single_named_trace.stdout.txt",
        )
        .assert_file_exists("trace-step-iter/test-step-trace-merge_trace.json")
        .assert_file_exists("trace-step-iter/contracts/forwarder.json")
        .assert_file_exists("trace-step-iter/contracts/receiver.json");

    let trace =
        read_json_from_project(&project, "trace-step-iter/test-step-trace-merge_trace.json");
    let traces = trace["traces"]
        .as_array()
        .unwrap_or_else(|| panic!("Missing traces array in step-iter trace json"));

    let merged_traces = traces
        .iter()
        .filter(|trace| trace["name"].as_str() == Some("step-forward-trace"))
        .collect::<Vec<_>>();
    assert_eq!(
        merged_traces.len(),
        1,
        "step execution should keep a single named logical trace"
    );
    let merged_trace = merged_traces[0];

    let transactions = merged_trace["transactions"]
        .as_array()
        .unwrap_or_else(|| panic!("Missing transactions in step-iter trace json"));
    assert_eq!(
        transactions.len(),
        2,
        "merged step trace should include both batches"
    );

    let root_children = transactions[0]["child_transactions"]
        .as_array()
        .unwrap_or_else(|| panic!("Missing root child_transactions in step-iter trace json"));
    assert_eq!(
        root_children.len(),
        1,
        "root tx should reference merged child tx"
    );
    assert_eq!(root_children[0].as_str(), transactions[1]["lt"].as_str());

    let failed_messages = merged_trace["failed_messages"]
        .as_array()
        .map_or_else(Vec::new, Clone::clone);
    assert!(
        failed_messages.is_empty(),
        "step trace should not fragment failures into extra chains"
    );
}

#[test]
fn profiling_snapshot_merges_step_execution_batches_into_single_named_trace_chain() {
    let project = ProjectBuilder::new("h-profile-step-iter")
        .file("contracts/messages", STEP_TRACE_MESSAGES)
        .contract("forwarder", STEP_TRACE_FORWARDER_CONTRACT)
        .contract("receiver", STEP_TRACE_RECEIVER_CONTRACT)
        .test_file(
            "trace",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build"
            import "../../lib/emulation/network"
import "../../lib/emulation/testing"
            import "../../lib/emulation/network"
            import "../contracts/messages"

            get fun `test step profile merge`() {
                val sender = testing.treasury("sender");

                val forwarderInit = ContractState {
                    code: build("forwarder"),
                    data: createEmptyCell(),
                };
                val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

                val receiverInit = ContractState {
                    code: build("receiver"),
                    data: createEmptyCell(),
                };
                val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

                expect(net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: { stateInit: forwarderInit },
                }))).toHaveSuccessfulDeploy({ to: forwarderAddress });

                expect(net.send(sender.address, createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: { stateInit: receiverInit },
                }))).toHaveSuccessfulDeploy({ to: receiverAddress });

                val iter = testing.createTraceIterationCursor(sender.address, createMessage({
                    bounce: false,
                    value: ton("0.5"),
                    dest: forwarderAddress,
                    body: TriggerForward {
                        queryId: 44,
                        target: receiverAddress,
                    },
                }));

                val first = iter.executeN(1);
                expect(first).toHaveLength(1);
                expect(first).toHaveSuccessfulTx<TriggerForward>({
                    from: sender.address,
                    to: forwarderAddress,
                });

                first.giveName("step-forward-trace");

                val tail = iter.executeAllRemaining();
                expect(tail).toHaveLength(1);
                expect(tail).toHaveSuccessfulTx<Notify>({
                    from: forwarderAddress,
                    to: receiverAddress,
                });
            }
            "#,
        )
        .build();

    let output = project
        .acton()
        .test()
        .arg("--snapshot")
        .arg("step-profile.json")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("Gas snapshot saved to step-profile.json")
        .assert_file_exists("step-profile.json");

    let profile = read_json_from_project(&project, "step-profile.json");
    let trace_chains = profile["trace_chains"]
        .as_object()
        .unwrap_or_else(|| panic!("Missing trace_chains object in step profile snapshot"));

    let merged_traces = trace_chains
        .values()
        .filter(|trace| trace["trace_name"].as_str() == Some("step-forward-trace"))
        .collect::<Vec<_>>();
    assert_eq!(
        merged_traces.len(),
        1,
        "profiling snapshot should keep a single named logical trace"
    );

    let merged_trace = merged_traces[0];
    assert_eq!(
        merged_trace["test_name"].as_str(),
        Some("test step profile merge"),
        "profiling snapshot should keep the owning test name"
    );
    assert_eq!(
        merged_trace["tx_count"].as_u64(),
        Some(2),
        "profiling snapshot should merge both step batches into one trace chain"
    );
}

#[test]
fn regular_run_without_trace_flag_does_not_create_trace_artifacts() {
    let project = trace_project(
        "h-regular-run-no-trace",
        r"
        get fun `test no trace`() {
            deployCounter();
        }
        ",
    );

    let output = project.acton().test().run().success();

    output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/regular_run_without_trace_flag_does_not_create_trace_artifacts.stdout.txt",
        );

    let trace_dir = project.path().join("build/traces");
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
        r"
        get fun `test-after-regular`() {
            deployCounter();
        }
        ",
    );

    let regular_output = project.acton().test().run().success();
    regular_output
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/cmd_agent_h/save_test_trace_can_be_enabled_after_regular_run.regular.stdout.txt",
        );

    let default_trace_dir = project.path().join("build/traces");
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
