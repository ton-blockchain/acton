use crate::common::{assertion, strip_ansi};
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output_preserve_escapes;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use tycho_types::boc::Boc;

const CHILD_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const DEPLOYER_CONTRACT: &str = r#"
import "../gen/child_code.tolk"

fun onInternalMessage(_: InMessage) {
    val childInit = ContractState {
        code: childCompiledCode(),
        data: createEmptyCell(),
    };

    val deployChild = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: childInit,
        },
    });
    deployChild.send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DEPLOY_SCRIPT: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"

fun main() {
    val wallet = net.wallet("deployer");

    val deployerInit = ContractState {
        code: build("deployer"),
        data: createEmptyCell(),
    };
    val deployerAddress = AutoDeployAddress {
        stateInit: deployerInit,
    }.calculateAddress();

    val deployDeployer = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: deployerInit,
        },
    });
    net.send(wallet.address, deployDeployer);

    println1("DEPLOYER_CONTRACT={}", deployerAddress);
}
"#;

const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;

const V3_GETTER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun addTen(value: int): int {
    return value + 10;
}
";

const V3_DEPLOY_GETTER_SCRIPT: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"

fun main() {
    val wallet = net.wallet("deployer");

    val getterInit = ContractState {
        code: build("getter"),
        data: createEmptyCell(),
    };
    val getterAddress = AutoDeployAddress {
        stateInit: getterInit,
    }.calculateAddress();

    val deployGetter = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: getterInit,
        },
    });
    net.send(wallet.address, deployGetter);

    println1("GETTER_CONTRACT={}", getterAddress);
}
"#;

const V3_MESSAGE_TEST_BOC: &str = "te6ccgEBCAEA3gACq0gA3hg/j9iig2aTi8NU/hguuHV4Mf1mEUmqqnI9JLMCjg8ALmmY2giNrr7xsgbsuxgdjCwn44jNXXhSczUiwyp4TxsQ7msoAAAAAAAAAAAAANL430UZAgEAEAAAAAAAAAAAART/APSkE/S88sgLAwIBYgcEAgFYBgUAF7itDtRNDTHzHXCx+AAFu+F4AJzQ+JGRMOAg1ywj9DsnfI4YMe1E0AHXCx8B1h/XCx9YoAHIzssfye1U4NcsIdOpeDQxjhIw7UTQ1h8wyM7PkAAAAALJ7VTggQ/2AccA8vQ=";

const LIBRARY_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const LIBRARY_WORKER_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val refundMsg = createMessage({
        bounce: false,
        value: 0,
        dest: in.senderAddress,
    });
    refundMsg.send(SEND_MODE_DESTROY | SEND_MODE_CARRY_ALL_BALANCE);
}

fun onBouncedMessage(_: InMessageBounced) {}
";

const LIBRARY_MANAGER_CONTRACT: &str = r#"
import "../gen/worker_code.tolk"

fun workerStateInit(): ContractState {
    return ContractState {
        code: workerCompiledCode(),
        data: createEmptyCell(),
    };
}

fun onInternalMessage(in: InMessage) {
    val workerInit = workerStateInit();
    val workerAddress = AutoDeployAddress {
        stateInit: workerInit,
    }.calculateAddress();

    if (in.valueCoins >= ton("0.2")) {
        val deployWorkerMsg = createMessage({
            bounce: false,
            value: ton("0.3"),
            dest: {
                stateInit: workerInit,
            },
        });
        deployWorkerMsg.send(SEND_MODE_REGULAR);
        return;
    }

    val destroyWorkerMsg = createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: workerAddress,
        body: beginCell().storeUint(1, 1).endCell(),
    });
    destroyWorkerMsg.send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DEPLOY_MANAGER_AND_WORKER_SCRIPT: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../gen/worker_code.tolk"

fun main() {
    val wallet = net.wallet("deployer");

    val managerInit = ContractState {
        code: build("manager"),
        data: createEmptyCell(),
    };
    val managerAddress = AutoDeployAddress {
        stateInit: managerInit,
    }.calculateAddress();

    val workerAddress = AutoDeployAddress {
        stateInit: {
            code: workerCompiledCode(),
            data: createEmptyCell(),
        },
    }.calculateAddress();

    val deployManagerMsg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: managerInit,
        },
    });
    net.send(wallet.address, deployManagerMsg);

    val deployWorkerViaManagerMsg = createMessage({
        bounce: false,
        value: ton("0.4"),
        dest: managerAddress,
    });
    net.send(wallet.address, deployWorkerViaManagerMsg);

    println1("MANAGER_CONTRACT={}", managerAddress);
    println1("WORKER_CONTRACT={}", workerAddress);
}
"#;

const DESTROY_WORKER_VIA_MANAGER_SCRIPT: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../gen/worker_code.tolk"

fun main() {
    val wallet = net.wallet("deployer");

    val managerAddress = AutoDeployAddress {
        stateInit: {
            code: build("manager"),
            data: createEmptyCell(),
        },
    }.calculateAddress();

    val workerAddress = AutoDeployAddress {
        stateInit: {
            code: workerCompiledCode(),
            data: createEmptyCell(),
        },
    }.calculateAddress();

    val triggerDestroyMsg = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: managerAddress,
        body: beginCell().storeUint(1, 1).endCell(),
    });
    net.send(wallet.address, triggerDestroyMsg);

    println1("MANAGER_CONTRACT={}", managerAddress);
    println1("WORKER_CONTRACT={}", workerAddress);
}
"#;

#[test]
fn litenode_supports_pre_start_commands_and_get_out_msg_queue_size() {
    let project = ProjectBuilder::new("litenode-pre-start-commands")
        .contract("child", CHILD_CONTRACT)
        .contract_with_deps("deployer", DEPLOYER_CONTRACT, vec!["child"])
        .script_file("deploy", DEPLOY_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(|cmd| cmd.build())
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let response = node.get_json("/api/v2/getOutMsgQueueSize");
    let mut response = response;
    normalize_out_msg_queue_size_for_snapshot(&mut response);

    let response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&response).expect("Failed to serialize JSON response")
    );

    assertion().eq(
        normalize_output_preserve_escapes(&response_json, project.path()),
        snapbox::file!("snapshots/test_litenode_get_out_msg_queue_size.response.json"),
    );

    let script_result = project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
        .verify_network("custom:localnet")
        .run();
    let script_stdout = String::from_utf8(script_result.output.get_output().stdout.clone())
        .expect("Failed to decode deploy script stdout");
    let script_stderr = String::from_utf8(script_result.output.get_output().stderr.clone())
        .expect("Failed to decode deploy script stderr");
    let script_status = script_result.output.get_output().status.code().unwrap_or(1);

    assert_eq!(
        script_status, 0,
        "Deploy script failed with status {script_status}\nstdout:\n{script_stdout}\nstderr:\n{script_stderr}"
    );

    let deployer_address = extract_marker_value(&script_stdout, "DEPLOYER_CONTRACT=");
    let tx_query = format!("/api/v2/getTransactionsStd?address={deployer_address}&limit=10");
    let deadline = Instant::now() + Duration::from_secs(12);
    let tx_std_response = loop {
        let response = node.get_json(&tx_query);
        let has_transactions = response
            .pointer("/result/transactions")
            .and_then(Value::as_array)
            .is_some_and(|txs| !txs.is_empty());
        if has_transactions || Instant::now() >= deadline {
            break response;
        }
        thread::sleep(Duration::from_millis(200));
    };

    let transactions = tx_std_response
        .pointer("/result/transactions")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "Expected array at /result/transactions, got:\n{}",
                serde_json::to_string_pretty(&tx_std_response).unwrap_or_default()
            )
        });
    assert!(
        !transactions.is_empty(),
        "Expected non-empty transactions for deployer contract address {deployer_address}, got:\n{}",
        serde_json::to_string_pretty(&tx_std_response).unwrap_or_default()
    );
    assert!(
        transactions.iter().any(|tx| tx
            .get("out_msgs")
            .and_then(Value::as_array)
            .is_some_and(|out| !out.is_empty())),
        "Expected at least one deployer transaction with outbound message (child deploy), got:\n{}",
        serde_json::to_string_pretty(&tx_std_response).unwrap_or_default()
    );

    let mut tx_std_response = tx_std_response;
    normalize_transactions_std_for_snapshot(&mut tx_std_response);

    let tx_std_response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&tx_std_response)
            .expect("Failed to serialize getTransactionsStd JSON response")
    );

    assertion().eq(
        normalize_output_preserve_escapes(&tx_std_response_json, project.path()),
        snapbox::file!("snapshots/test_litenode_get_transactions_std.response.json"),
    );

    node.stop();
}

#[test]
fn litenode_supports_try_locate_transaction_endpoints() {
    let project = ProjectBuilder::new("litenode-try-locate-endpoints")
        .contract("child", CHILD_CONTRACT)
        .contract_with_deps("deployer", DEPLOYER_CONTRACT, vec!["child"])
        .script_file("deploy", DEPLOY_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(|cmd| cmd.build())
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
        .verify_network("custom:localnet")
        .run();
    let script_stdout = String::from_utf8(script_result.output.get_output().stdout.clone())
        .expect("Failed to decode deploy script stdout");
    let script_stderr = String::from_utf8(script_result.output.get_output().stderr.clone())
        .expect("Failed to decode deploy script stderr");
    let script_status = script_result.output.get_output().status.code().unwrap_or(1);

    assert_eq!(
        script_status, 0,
        "Deploy script failed with status {script_status}\nstdout:\n{script_stdout}\nstderr:\n{script_stderr}"
    );

    let deployer_address = extract_marker_value(&script_stdout, "DEPLOYER_CONTRACT=");

    let deadline = Instant::now() + Duration::from_secs(12);
    let (source_tx_hash, source, destination, created_lt) = loop {
        let response = node.get_json(&format!(
            "/api/v2/getTransactions?address={deployer_address}&limit=10"
        ));
        if let Some(locator) = extract_first_outgoing_message_locator(&response) {
            break locator;
        }
        assert!(
            Instant::now() < deadline,
            "Failed to find outgoing message in deployer transactions:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    };

    let try_locate_tx_query = format!(
        "/api/v2/tryLocateTx?source={source}&destination={destination}&created_lt={created_lt}"
    );
    let try_locate_tx = wait_for_ok_response(&node, &try_locate_tx_query, Duration::from_secs(12));
    assert_eq!(
        try_locate_tx["result"]["@type"].as_str(),
        Some("ext.transaction")
    );
    assert_eq!(
        try_locate_tx["result"]["account"].as_str(),
        Some(destination.as_str())
    );

    let try_locate_result_tx_query = format!(
        "/api/v2/tryLocateResultTx?source={source}&destination={destination}&created_lt={created_lt}"
    );
    let try_locate_result_tx =
        wait_for_ok_response(&node, &try_locate_result_tx_query, Duration::from_secs(12));
    assert_eq!(
        try_locate_result_tx["result"]["hash"].as_str(),
        try_locate_tx["result"]["hash"].as_str()
    );

    let try_locate_source_tx = node.get_json(&format!(
        "/api/v2/tryLocateSourceTx?source={source}&destination={destination}&created_lt={created_lt}"
    ));
    assert_eq!(
        try_locate_source_tx["ok"].as_bool(),
        Some(true),
        "tryLocateSourceTx failed: {}",
        serde_json::to_string_pretty(&try_locate_source_tx).unwrap_or_default()
    );
    assert_eq!(
        try_locate_source_tx["result"]["hash"].as_str(),
        Some(source_tx_hash.as_str())
    );
    assert_eq!(
        try_locate_source_tx["result"]["account"].as_str(),
        Some(source.as_str())
    );

    let try_locate_tx_rpc = node.post_json(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tryLocateTx",
            "params": {
                "source": source,
                "destination": destination,
                "created_lt": created_lt
            }
        }),
    );
    assert_eq!(try_locate_tx_rpc["ok"].as_bool(), Some(true));
    assert_eq!(
        try_locate_tx_rpc["result"]["hash"].as_str(),
        try_locate_tx["result"]["hash"].as_str()
    );

    node.stop();
}

#[test]
fn litenode_supports_library_publish_and_get_libraries_endpoint() {
    let project = ProjectBuilder::new("litenode-library-support")
        .contract("library_contract", LIBRARY_CONTRACT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(|cmd| cmd.build())
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    project
        .acton()
        .library()
        .publish()
        .arg("library_contract")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("custom:localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .arg("--api-key")
        .arg("local-test-api-key")
        .run()
        .success();

    let libraries_toml = fs::read_to_string(project.path().join("libraries.toml"))
        .expect("Failed to read libraries.toml");
    let libraries_doc: toml::Value = toml::from_str(&libraries_toml).expect("Invalid TOML");
    let libraries = libraries_doc["libraries"]
        .as_table()
        .expect("`libraries` table is missing");
    let (_, lib_entry) = libraries
        .iter()
        .next()
        .expect("Expected at least one published library");
    let library_hash = lib_entry["hash"]
        .as_str()
        .expect("Library hash is missing")
        .to_string();
    let library_code_b64 = lib_entry["code"]
        .as_str()
        .expect("Library code is missing")
        .to_string();

    let query = format!("/api/v2/getLibraries?libraries={library_hash}");
    let deadline = Instant::now() + Duration::from_secs(12);
    let get_libraries_response = loop {
        let response = node.get_json(&query);
        let has_result = response
            .pointer("/result/result")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
        if has_result || Instant::now() >= deadline {
            break response;
        }
        thread::sleep(Duration::from_millis(200));
    };

    let result_items = get_libraries_response
        .pointer("/result/result")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "Expected getLibraries result array, got:\n{}",
                serde_json::to_string_pretty(&get_libraries_response).unwrap_or_default()
            )
        });
    assert!(
        !result_items.is_empty(),
        "Expected non-empty getLibraries result, got:\n{}",
        serde_json::to_string_pretty(&get_libraries_response).unwrap_or_default()
    );
    let first = &result_items[0];
    assert_eq!(first["@type"].as_str(), Some("smc.libraryEntry"));
    assert_eq!(first["hash"].as_str(), Some(library_hash.as_str()));
    assert_eq!(first["data"].as_str(), Some(library_code_b64.as_str()));

    #[allow(clippy::manual_strip)]
    let missing_hash = if library_hash.starts_with('0') {
        format!("1{}", &library_hash[1..])
    } else {
        format!("0{}", &library_hash[1..])
    };

    let mixed_query = format!("/api/v2/getLibraries?libraries={missing_hash},,{library_hash}");
    let mixed_response = node.get_json(&mixed_query);
    assert_eq!(mixed_response["ok"].as_bool(), Some(true));
    let mixed_items = mixed_response
        .pointer("/result/result")
        .and_then(Value::as_array)
        .expect("Mixed getLibraries response must contain result array");
    assert_eq!(
        mixed_items.len(),
        1,
        "Expected only found libraries in response"
    );
    assert_eq!(mixed_items[0]["@type"].as_str(), Some("smc.libraryEntry"));
    assert_eq!(mixed_items[0]["hash"].as_str(), Some(library_hash.as_str()));
    assert_eq!(
        mixed_items[0]["data"].as_str(),
        Some(library_code_b64.as_str())
    );

    let empty_libraries_response = node.get_json("/api/v2/getLibraries?libraries=,,");
    assert_eq!(empty_libraries_response["ok"].as_bool(), Some(false));
    assert!(
        empty_libraries_response["error"]
            .as_str()
            .unwrap_or_default()
            .contains("`libraries` query parameter is required"),
        "Unexpected error for empty libraries query: {}",
        serde_json::to_string_pretty(&empty_libraries_response).unwrap_or_default()
    );

    let invalid_libraries_response = node.get_json("/api/v2/getLibraries?libraries=not-a-hash");
    assert_eq!(invalid_libraries_response["ok"].as_bool(), Some(false));
    assert!(
        invalid_libraries_response["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Invalid hash format"),
        "Unexpected error for invalid hash query: {}",
        serde_json::to_string_pretty(&invalid_libraries_response).unwrap_or_default()
    );

    let rpc_libraries = node.post_json(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getLibraries",
            "params": {
                "libraries": format!("{missing_hash},{library_hash}")
            }
        }),
    );
    assert_eq!(rpc_libraries["ok"].as_bool(), Some(true));
    let rpc_items = rpc_libraries["result"]["result"]
        .as_array()
        .expect("JSON-RPC getLibraries response must contain result array");
    assert_eq!(rpc_items.len(), 1);
    assert_eq!(rpc_items[0]["@type"].as_str(), Some("smc.libraryEntry"));
    assert_eq!(rpc_items[0]["hash"].as_str(), Some(library_hash.as_str()));

    project
        .acton()
        .library()
        .fetch(&library_hash)
        .arg("--net")
        .arg("custom:localnet")
        .arg("--api-key")
        .arg("local-test-api-key")
        .arg("--output")
        .arg("fetched_library.boc")
        .run()
        .success();

    let fetched_boc = fs::read(project.path().join("fetched_library.boc"))
        .expect("Failed to read fetched library boc");
    let fetched_cell = Boc::decode(&fetched_boc).expect("Fetched library BOC must be valid");
    let fetched_hash = hex::encode(fetched_cell.repr_hash().as_array());
    assert_eq!(fetched_hash, library_hash);

    node.stop();
}

#[test]
#[ignore]
fn litenode_supports_library_ref_contract_deploy_and_destroy_flow() {
    let project = ProjectBuilder::new("litenode-library-ref-contract-flow")
        .contract("worker", LIBRARY_WORKER_CONTRACT)
        .contract_with_detailed_deps(
            "manager",
            LIBRARY_MANAGER_CONTRACT,
            vec![("worker", Some("library_ref"), None, None)],
        )
        .script_file(
            "deploy_manager_and_worker",
            DEPLOY_MANAGER_AND_WORKER_SCRIPT,
        )
        .script_file(
            "destroy_worker_via_manager",
            DESTROY_WORKER_VIA_MANAGER_SCRIPT,
        )
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(|cmd| cmd.build())
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    project
        .acton()
        .library()
        .publish()
        .arg("worker")
        .arg("--wallet")
        .arg("deployer")
        .arg("--net")
        .arg("custom:localnet")
        .arg("--duration")
        .arg("1y")
        .arg("--amount")
        .arg("5")
        .arg("--yes")
        .arg("--local")
        .arg("--api-key")
        .arg("local-test-api-key")
        .run()
        .success();

    let deploy_result = project
        .acton()
        .script("scripts/deploy_manager_and_worker.tolk")
        .broadcast()
        .verify_network("custom:localnet")
        .run();
    let deploy_stdout = String::from_utf8(deploy_result.output.get_output().stdout.clone())
        .expect("Failed to decode deploy script stdout");
    let deploy_stderr = String::from_utf8(deploy_result.output.get_output().stderr.clone())
        .expect("Failed to decode deploy script stderr");
    let deploy_status = deploy_result.output.get_output().status.code().unwrap_or(1);
    assert_eq!(
        deploy_status, 0,
        "Deploy script failed with status {deploy_status}\nstdout:\n{deploy_stdout}\nstderr:\n{deploy_stderr}"
    );

    let manager_address = extract_marker_value(&deploy_stdout, "MANAGER_CONTRACT=");
    let worker_address = extract_marker_value(&deploy_stdout, "WORKER_CONTRACT=");
    let worker_raw_address = unpack_address(&node, &worker_address);

    wait_until_address_state_active(&node, &manager_address, Duration::from_secs(12));
    wait_until_address_state_active(&node, &worker_address, Duration::from_secs(12));

    let manager_info_before = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={manager_address}"),
        Duration::from_secs(12),
    );
    let worker_info_before = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={worker_address}"),
        Duration::from_secs(12),
    );

    assert_eq!(
        manager_info_before["result"]["state"].as_str(),
        Some("active")
    );
    assert_eq!(
        worker_info_before["result"]["state"].as_str(),
        Some("active")
    );
    assert!(
        !manager_info_before["result"]["code"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "Manager code is unexpectedly empty:\n{}",
        serde_json::to_string_pretty(&manager_info_before).unwrap_or_default()
    );
    assert!(
        !worker_info_before["result"]["code"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "Worker code is unexpectedly empty:\n{}",
        serde_json::to_string_pretty(&worker_info_before).unwrap_or_default()
    );

    let manager_balance_before = parse_address_balance(&manager_info_before);
    let worker_balance_before = parse_address_balance(&worker_info_before);
    assert!(
        worker_balance_before > 0,
        "Worker balance should be positive after deploy, got:\n{}",
        serde_json::to_string_pretty(&worker_info_before).unwrap_or_default()
    );

    let destroy_result = project
        .acton()
        .script("scripts/destroy_worker_via_manager.tolk")
        .broadcast()
        .verify_network("custom:localnet")
        .run();
    let destroy_stdout = String::from_utf8(destroy_result.output.get_output().stdout.clone())
        .expect("Failed to decode destroy script stdout");
    let destroy_stderr = String::from_utf8(destroy_result.output.get_output().stderr.clone())
        .expect("Failed to decode destroy script stderr");
    let destroy_status = destroy_result
        .output
        .get_output()
        .status
        .code()
        .unwrap_or(1);
    assert_eq!(
        destroy_status, 0,
        "Destroy script failed with status {destroy_status}\nstdout:\n{destroy_stdout}\nstderr:\n{destroy_stderr}"
    );
    assert_eq!(
        extract_marker_value(&destroy_stdout, "MANAGER_CONTRACT="),
        manager_address
    );
    assert_eq!(
        extract_marker_value(&destroy_stdout, "WORKER_CONTRACT="),
        worker_address
    );

    let worker_info_query = format!("/api/v2/getAddressInformation?address={worker_address}");
    let deadline = Instant::now() + Duration::from_secs(12);
    let worker_info_after = loop {
        let response = node.get_json(&worker_info_query);
        if response["ok"].as_bool() == Some(true)
            && response["result"]["state"]
                .as_str()
                .is_some_and(|state| state != "active")
        {
            break response;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for worker contract `{worker_address}` to be destroyed:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(
        worker_info_after["result"]["state"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(worker_info_after["result"]["code"].as_str(), Some(""));
    assert_eq!(worker_info_after["result"]["data"].as_str(), Some(""));
    assert_eq!(parse_address_balance(&worker_info_after), 0);

    let manager_info_after = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={manager_address}"),
        Duration::from_secs(12),
    );
    assert_eq!(
        manager_info_after["result"]["state"].as_str(),
        Some("active")
    );
    let manager_balance_after = parse_address_balance(&manager_info_after);
    assert!(
        manager_balance_after > manager_balance_before,
        "Expected manager balance to increase after worker self-destruct. before={manager_balance_before}, after={manager_balance_after}\nmanager_before:\n{}\nmanager_after:\n{}",
        serde_json::to_string_pretty(&manager_info_before).unwrap_or_default(),
        serde_json::to_string_pretty(&manager_info_after).unwrap_or_default()
    );

    let manager_txs_query = format!("/api/v2/getTransactions?address={manager_address}&limit=20");
    let tx_deadline = Instant::now() + Duration::from_secs(12);
    let manager_txs = loop {
        let response = node.get_json(&manager_txs_query);
        if has_incoming_transaction_from_source(&response, &worker_raw_address) {
            break response;
        }
        assert!(
            Instant::now() < tx_deadline,
            "Timed out waiting for incoming transaction from worker `{worker_address}` (`{worker_raw_address}`) to manager `{manager_address}`:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    };
    assert!(
        has_incoming_transaction_from_source(&manager_txs, &worker_raw_address),
        "Expected manager transactions to include inbound transfer from worker `{worker_address}` (`{worker_raw_address}`):\n{}",
        serde_json::to_string_pretty(&manager_txs).unwrap_or_default()
    );

    node.stop();
}

#[test]
fn litenode_supports_config_endpoints() {
    let project = ProjectBuilder::new("litenode-config-endpoints").build();
    let node = project.litenode().start();

    let get_config_all = node.get_json("/api/v2/getConfigAll");
    assert_eq!(
        get_config_all["ok"].as_bool(),
        Some(true),
        "getConfigAll failed: {}",
        serde_json::to_string_pretty(&get_config_all).unwrap_or_default()
    );
    assert_eq!(
        get_config_all["result"]["@type"].as_str(),
        Some("configInfo")
    );
    assert_eq!(
        get_config_all["result"]["config"]["@type"].as_str(),
        Some("tvm.cell")
    );
    let all_bytes = get_config_all["result"]["config"]["bytes"]
        .as_str()
        .expect("getConfigAll result.config.bytes must be a string")
        .to_owned();
    assert!(
        !all_bytes.is_empty(),
        "getConfigAll returned an empty config cell"
    );

    let get_config_param = node.get_json("/api/v2/getConfigParam?param=8");
    assert_eq!(
        get_config_param["ok"].as_bool(),
        Some(true),
        "getConfigParam failed: {}",
        serde_json::to_string_pretty(&get_config_param).unwrap_or_default()
    );
    assert_eq!(
        get_config_param["result"]["@type"].as_str(),
        Some("configInfo")
    );
    let param_bytes = get_config_param["result"]["config"]["bytes"]
        .as_str()
        .expect("getConfigParam result.config.bytes must be a string")
        .to_owned();
    assert!(
        !param_bytes.is_empty(),
        "getConfigParam returned an empty parameter cell"
    );
    assert_ne!(
        all_bytes, param_bytes,
        "Expected param cell BOC to differ from full config BOC"
    );

    let get_config_param_alias = node.get_json("/api/v2/getConfigParam?config_id=8");
    assert_eq!(
        get_config_param_alias["ok"].as_bool(),
        Some(true),
        "getConfigParam with config_id failed: {}",
        serde_json::to_string_pretty(&get_config_param_alias).unwrap_or_default()
    );
    assert_eq!(
        get_config_param_alias["result"]["config"]["bytes"].as_str(),
        Some(param_bytes.as_str())
    );

    let rpc_get_config_all = node.post_json(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getConfigAll",
            "params": {}
        }),
    );
    assert_eq!(rpc_get_config_all["ok"].as_bool(), Some(true));
    assert_eq!(
        rpc_get_config_all["result"]["config"]["bytes"].as_str(),
        Some(all_bytes.as_str())
    );

    let rpc_get_config_param = node.post_json(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "getConfigParam",
            "params": {
                "param": 8
            }
        }),
    );
    assert_eq!(rpc_get_config_param["ok"].as_bool(), Some(true));
    assert_eq!(
        rpc_get_config_param["result"]["config"]["bytes"].as_str(),
        Some(param_bytes.as_str())
    );

    node.stop();
}

#[test]
fn litenode_supports_v3_message_endpoint() {
    let project = ProjectBuilder::new("litenode-v3-message-endpoint").build();
    let node = project.litenode().start();

    let response = node.post_json(
        "/api/v3/message",
        &json!({
            "boc": V3_MESSAGE_TEST_BOC
        }),
    );

    assert_eq!(
        response["ok"].as_bool(),
        Some(true),
        "v3 message failed: {}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );

    let message_hash = response["result"]["message_hash"]
        .as_str()
        .expect("v3 message result.message_hash must be a string");
    let message_hash_norm = response["result"]["message_hash_norm"]
        .as_str()
        .expect("v3 message result.message_hash_norm must be a string");

    assert!(
        !message_hash.is_empty(),
        "Expected non-empty message_hash in v3 message response:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert_eq!(message_hash_norm, message_hash);

    let invalid = node.post_json(
        "/api/v3/message",
        &json!({
            "boc": "not-base64"
        }),
    );

    assert_eq!(invalid["ok"].as_bool(), Some(false));
    assert!(
        invalid["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Invalid BOC base64"),
        "Unexpected error for invalid v3 message payload:\n{}",
        serde_json::to_string_pretty(&invalid).unwrap_or_default()
    );

    node.stop();
}

#[test]
fn litenode_supports_v3_address_information_endpoint() {
    let project = ProjectBuilder::new("litenode-v3-address-information")
        .contract("getter", V3_GETTER_CONTRACT)
        .script_file("deploy_getter", V3_DEPLOY_GETTER_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(|cmd| cmd.build())
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
        .broadcast()
        .verify_network("custom:localnet")
        .run();
    let script_stdout = String::from_utf8(script_result.output.get_output().stdout.clone())
        .expect("Failed to decode deploy script stdout");
    let script_stderr = String::from_utf8(script_result.output.get_output().stderr.clone())
        .expect("Failed to decode deploy script stderr");
    let script_status = script_result.output.get_output().status.code().unwrap_or(1);

    assert_eq!(
        script_status, 0,
        "Deploy script failed with status {script_status}\nstdout:\n{script_stdout}\nstderr:\n{script_stderr}"
    );

    let getter_address = extract_marker_value(&script_stdout, "GETTER_CONTRACT=");
    wait_until_address_state_active(&node, &getter_address, Duration::from_secs(12));

    let v2_query = format!("/api/v2/getAddressInformation?address={getter_address}");
    let v2_response = wait_for_ok_response(&node, &v2_query, Duration::from_secs(12));

    let v3_query = format!("/api/v3/addressInformation?address={getter_address}");
    let v3_response = wait_for_ok_response(&node, &v3_query, Duration::from_secs(12));

    assert_eq!(
        v3_response["result"]["balance"].as_str(),
        v2_response["result"]["balance"].as_str()
    );
    assert_eq!(
        v3_response["result"]["code"].as_str(),
        v2_response["result"]["code"].as_str()
    );
    assert_eq!(
        v3_response["result"]["data"].as_str(),
        v2_response["result"]["data"].as_str()
    );
    assert_eq!(
        v3_response["result"]["frozen_hash"].as_str(),
        v2_response["result"]["frozen_hash"].as_str()
    );
    assert_eq!(
        v3_response["result"]["last_transaction_hash"].as_str(),
        v2_response["result"]["last_transaction_id"]["hash"].as_str()
    );
    assert_eq!(
        v3_response["result"]["last_transaction_lt"].as_str(),
        v2_response["result"]["last_transaction_id"]["lt"].as_str()
    );
    assert_eq!(
        v3_response["result"]["status"].as_str(),
        v2_response["result"]["state"].as_str()
    );
    assert_eq!(v3_response["result"]["status"].as_str(), Some("active"));

    let missing_address = "0:1111111111111111111111111111111111111111111111111111111111111111";

    let v2_missing = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={missing_address}"),
        Duration::from_secs(12),
    );
    let v3_missing_default = wait_for_ok_response(
        &node,
        &format!("/api/v3/addressInformation?address={missing_address}"),
        Duration::from_secs(12),
    );
    let v3_missing_use_v2_false = wait_for_ok_response(
        &node,
        &format!("/api/v3/addressInformation?address={missing_address}&use_v2=false"),
        Duration::from_secs(12),
    );

    assert_eq!(
        v2_missing["result"]["state"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(
        v3_missing_default["result"]["status"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(
        v3_missing_use_v2_false["result"]["status"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(
        v3_missing_default["result"]["status"].as_str(),
        v3_missing_use_v2_false["result"]["status"].as_str()
    );

    node.stop();
}

#[test]
fn litenode_supports_v3_run_get_method() {
    let project = ProjectBuilder::new("litenode-v3-run-get-method")
        .contract("getter", V3_GETTER_CONTRACT)
        .script_file("deploy_getter", V3_DEPLOY_GETTER_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(|cmd| cmd.build())
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
        .broadcast()
        .verify_network("custom:localnet")
        .run();
    let script_stdout = String::from_utf8(script_result.output.get_output().stdout.clone())
        .expect("Failed to decode deploy script stdout");
    let script_stderr = String::from_utf8(script_result.output.get_output().stderr.clone())
        .expect("Failed to decode deploy script stderr");
    let script_status = script_result.output.get_output().status.code().unwrap_or(1);

    assert_eq!(
        script_status, 0,
        "Deploy script failed with status {script_status}\nstdout:\n{script_stdout}\nstderr:\n{script_stderr}"
    );

    let getter_address = extract_marker_value(&script_stdout, "GETTER_CONTRACT=");
    wait_until_address_state_active(&node, &getter_address, Duration::from_secs(12));

    let response = node.post_json(
        "/api/v3/runGetMethod",
        &json!({
            "address": getter_address,
            "method": "addTen",
            "stack": [
                {
                    "type": "num",
                    "value": "7"
                }
            ]
        }),
    );

    assert_eq!(
        response["ok"].as_bool(),
        Some(true),
        "v3 runGetMethod failed: {}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert_eq!(response["result"]["exit_code"].as_i64(), Some(0));
    assert_eq!(response["result"]["stack"][0]["type"].as_str(), Some("num"));
    assert_eq!(response["result"]["stack"][0]["value"].as_str(), Some("17"));

    node.stop();
}

#[test]
fn litenode_supports_utils_detect_and_pack_endpoints() {
    let project = ProjectBuilder::new("litenode-utils-endpoints").build();
    let node = project.litenode().start();

    let raw_address = "0:84545d4d2cada0ce811705d534c298ca42d29315d03a16eee794cefd191dfa79";
    let detect_address = node.get_json(&format!("/api/v2/detectAddress?address={raw_address}"));
    assert_eq!(
        detect_address["ok"].as_bool(),
        Some(true),
        "detectAddress failed: {}",
        serde_json::to_string_pretty(&detect_address).unwrap_or_default()
    );
    assert_eq!(
        detect_address["result"]["@type"].as_str(),
        Some("ext.utils.detectedAddress")
    );
    assert_eq!(
        detect_address["result"]["raw_form"].as_str(),
        Some(raw_address)
    );
    assert_eq!(
        detect_address["result"]["given_type"].as_str(),
        Some("raw_form")
    );
    assert_eq!(detect_address["result"]["test_only"].as_bool(), Some(false));

    let bounceable_b64url = detect_address["result"]["bounceable"]["b64url"]
        .as_str()
        .expect("Missing bounceable b64url")
        .to_string();
    let detect_address_friendly = node.get_json(&format!(
        "/api/v2/detectAddress?address={bounceable_b64url}"
    ));
    assert_eq!(
        detect_address_friendly["result"]["given_type"].as_str(),
        Some("friendly_bounceable")
    );
    assert_eq!(
        detect_address_friendly["result"]["raw_form"].as_str(),
        Some(raw_address)
    );

    let pack_address = node.get_json(&format!("/api/v2/packAddress?address={raw_address}"));
    let packed = pack_address["result"]
        .as_str()
        .expect("packAddress result must be string");
    assert_eq!(packed, bounceable_b64url);

    let unpack_address = node.get_json(&format!("/api/v2/unpackAddress?address={packed}"));
    assert_eq!(unpack_address["result"].as_str(), Some(raw_address));

    let hex_hash = "abababababababababababababababababababababababababababababababab";
    let detect_hash_hex = node.get_json(&format!("/api/v2/detectHash?hash={hex_hash}"));
    assert_eq!(
        detect_hash_hex["ok"].as_bool(),
        Some(true),
        "detectHash(hex) failed: {}",
        serde_json::to_string_pretty(&detect_hash_hex).unwrap_or_default()
    );
    assert_eq!(
        detect_hash_hex["result"]["@type"].as_str(),
        Some("ext.utils.detectedHash")
    );
    assert_eq!(detect_hash_hex["result"]["hex"].as_str(), Some(hex_hash));

    let hash_b64url = detect_hash_hex["result"]["b64url"]
        .as_str()
        .expect("Missing detectHash b64url");
    let detect_hash_b64url = node.get_json(&format!("/api/v2/detectHash?hash={hash_b64url}"));
    assert_eq!(detect_hash_b64url["result"]["hex"].as_str(), Some(hex_hash));

    node.stop();
}

fn append_localnet_network(project_path: &Path, base_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("Failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r#"

[networks.localnet]
v2-url = "{base_url}/api/v2"
"#
    ));
    fs::write(&acton_toml_path, acton_toml).expect("Failed to write Acton.toml with localnet");
}

fn extract_marker_value(output: &str, marker: &str) -> String {
    let cleaned = strip_ansi(output);
    cleaned
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(marker).map(ToOwned::to_owned))
        .unwrap_or_else(|| panic!("Marker `{marker}` not found in output:\n{cleaned}"))
}

fn wait_for_ok_response(
    node: &crate::support::litenode::LiteNodeHandle,
    query: &str,
    timeout: Duration,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json(query);
        if response["ok"].as_bool() == Some(true) {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for successful response from `{query}`:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn wait_until_address_state_active(
    node: &crate::support::litenode::LiteNodeHandle,
    address: &str,
    timeout: Duration,
) {
    let query = format!("/api/v2/getAddressState?address={address}");
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json(&query);
        if response["ok"].as_bool() == Some(true) && response["result"].as_str() == Some("active") {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for address `{address}` to become active:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn parse_address_balance(address_information: &Value) -> u128 {
    address_information["result"]["balance"]
        .as_str()
        .unwrap_or_else(|| {
            panic!(
                "Expected string balance field in getAddressInformation response:\n{}",
                serde_json::to_string_pretty(address_information).unwrap_or_default()
            )
        })
        .parse::<u128>()
        .unwrap_or_else(|e| {
            panic!(
                "Failed to parse balance from getAddressInformation response: {e}\n{}",
                serde_json::to_string_pretty(address_information).unwrap_or_default()
            )
        })
}

fn unpack_address(node: &crate::support::litenode::LiteNodeHandle, address: &str) -> String {
    let response = wait_for_ok_response(
        node,
        &format!("/api/v2/unpackAddress?address={address}"),
        Duration::from_secs(12),
    );
    response["result"]
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| {
            panic!(
                "Expected string result from unpackAddress for `{address}`:\n{}",
                serde_json::to_string_pretty(&response).unwrap_or_default()
            )
        })
}

fn has_incoming_transaction_from_source(response: &Value, source: &str) -> bool {
    response["result"].as_array().is_some_and(|txs| {
        txs.iter().any(|tx| {
            tx["in_msg"]["source"]
                .as_str()
                .is_some_and(|tx_source| tx_source == source)
        })
    })
}

fn extract_first_outgoing_message_locator(
    response: &Value,
) -> Option<(String, String, String, u64)> {
    let txs = response.get("result")?.as_array()?;
    for tx in txs {
        let tx_hash = tx.get("hash")?.as_str()?;
        let out_msgs = tx.get("out_msgs")?.as_array()?;
        for out_msg in out_msgs {
            let source = out_msg.get("source")?.as_str()?;
            let destination = out_msg.get("destination")?.as_str()?;
            let created_lt = out_msg.get("created_lt")?.as_str()?.parse::<u64>().ok()?;
            if !source.is_empty() && !destination.is_empty() && created_lt > 0 {
                return Some((
                    tx_hash.to_owned(),
                    source.to_owned(),
                    destination.to_owned(),
                    created_lt,
                ));
            }
        }
    }
    None
}

fn normalize_transactions_std_for_snapshot(response: &mut Value) {
    if let Some(extra) = response.get_mut("@extra") {
        *extra = json!("[EXTRA]");
    }
    redact_dynamic_transaction_fields(response);
}

fn normalize_out_msg_queue_size_for_snapshot(response: &mut Value) {
    if let Some(extra) = response.get_mut("@extra") {
        *extra = json!("[EXTRA]");
    }

    if let Some(shards) = response
        .pointer_mut("/result/shards")
        .and_then(Value::as_array_mut)
    {
        for shard in shards {
            if let Some(id) = shard.get_mut("id").and_then(Value::as_object_mut) {
                if let Some(file_hash) = id.get_mut("file_hash") {
                    *file_hash = json!("[HASH]");
                }
                if let Some(root_hash) = id.get_mut("root_hash") {
                    *root_hash = json!("[HASH]");
                }
                if let Some(seqno) = id.get_mut("seqno") {
                    *seqno = json!("[SEQNO]");
                }
            }
        }
    }
}

fn redact_dynamic_transaction_fields(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                redact_dynamic_transaction_fields(item);
            }
        }
        Value::Object(map) => {
            for (key, inner) in map.iter_mut() {
                match key.as_str() {
                    "hash" | "body_hash" => *inner = json!("[HASH]"),
                    "lt" | "created_lt" => *inner = json!("[LT]"),
                    "utime" | "created_at" => *inner = json!("[TIME]"),
                    "data" => *inner = json!("[DATA]"),
                    "body" | "init_state" => *inner = json!("[BOC]"),
                    _ => redact_dynamic_transaction_fields(inner),
                }
            }
        }
        _ => {}
    }
}
