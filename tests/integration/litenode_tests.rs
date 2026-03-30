use crate::common::{assertion, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output_preserve_escapes;
use base64::Engine;
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

const PRINT_SEND_RESULT_SCRIPT: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"

fun main() {
    val wallet = net.wallet("deployer");

    val childInit = ContractState {
        code: build("child"),
        data: createEmptyCell(),
    };

    val deployChild = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: childInit,
        },
    });

    println(net.send(wallet.address, deployChild));
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
const V3_TRANSACTIONS_TEST_ACCOUNT_A: &str =
    "0:84545d4d2cada0ce811705d534c298ca42d29315d03a16eee794cefd191dfa79";
const V3_TRANSACTIONS_TEST_ACCOUNT_B: &str =
    "0:1111111111111111111111111111111111111111111111111111111111111111";

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
fn litenode_starts_and_serves_masterchain_info() {
    let project = ProjectBuilder::new("litenode-smoke-masterchain-info").build();
    let node = project.litenode().start();

    let response = node.get_json("/api/v2/getMasterchainInfo");
    assert_eq!(
        response["ok"].as_bool(),
        Some(true),
        "Expected getMasterchainInfo to succeed:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert!(
        response["result"]["last"]["seqno"].as_u64().is_some(),
        "Expected getMasterchainInfo result.last.seqno to be present:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );

    node.stop();
}

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
        .before_start(super::super::support::project::ActonCommand::build)
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
        .verify_network("localnet")
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
fn litenode_can_rate_limit_api_endpoints_to_simulate_provider_limits() {
    let project = ProjectBuilder::new("litenode-rate-limit").build();
    let node = project.litenode().args(["--rate-limit", "1"]).start();

    thread::sleep(Duration::from_millis(1100));

    let first = node.get_json("/api/v2/getMasterchainInfo");
    assert_eq!(
        first["ok"].as_bool(),
        Some(true),
        "Expected first API request to succeed:\n{}",
        serde_json::to_string_pretty(&first).unwrap_or_default()
    );

    let (status, rate_limited) = node.get_json_with_status("/api/v2/getMasterchainInfo");
    assert_eq!(status, 429, "Expected second request to be rate-limited");
    assert_eq!(rate_limited["ok"].as_bool(), Some(false));
    assert_eq!(rate_limited["code"].as_i64(), Some(429));
    assert!(
        rate_limited["error"]
            .as_str()
            .is_some_and(|msg| msg.contains("Rate limit exceeded")),
        "Expected rate-limit error message, got:\n{}",
        serde_json::to_string_pretty(&rate_limited).unwrap_or_default()
    );

    let (admin_status, admin_response) = node.get_json_with_status("/admin/state-source");
    assert_eq!(
        admin_status, 200,
        "Admin endpoints must stay available when API rate-limit is enabled"
    );
    assert_eq!(admin_response["ok"].as_bool(), Some(true));

    thread::sleep(Duration::from_millis(1100));

    let (status_after_window, api_after_window) =
        node.get_json_with_status("/api/v2/getMasterchainInfo");
    assert_eq!(
        status_after_window, 200,
        "Expected API requests to recover after rate-limit window"
    );
    assert_eq!(api_after_window["ok"].as_bool(), Some(true));

    node.stop();
}

#[test]
fn litenode_script_println_net_send_in_broadcast_shows_synthetic_hint() {
    let project = ProjectBuilder::new("litenode-broadcast-println-net-send")
        .contract("child", CHILD_CONTRACT)
        .script_file("deploy", PRINT_SEND_RESULT_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .litenode()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let output = project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
        .verify_network("localnet")
        .run()
        .success();

    output
        .assert_contains("Broadcast send (synthetic result)")
        .assert_not_contains("compute phase skipped")
        .assert_snapshot_matches(
            "integration/snapshots/test_litenode_script_println_net_send_in_broadcast_shows_synthetic_hint.stdout.txt",
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
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy.tolk")
        .broadcast()
        .verify_network("localnet")
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
        .before_start(super::super::support::project::ActonCommand::build)
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
        .arg("localnet")
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
    assert!(
        first["hash"]
            .as_str()
            .is_some_and(|api_hash| hashes_equivalent(api_hash, &library_hash)),
        "Expected API hash `{}` to represent the same value as metadata hash `{}`",
        first["hash"].as_str().unwrap_or_default(),
        library_hash
    );
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
    assert!(
        mixed_items[0]["hash"]
            .as_str()
            .is_some_and(|api_hash| hashes_equivalent(api_hash, &library_hash)),
        "Expected mixed API hash `{}` to represent the same value as metadata hash `{}`",
        mixed_items[0]["hash"].as_str().unwrap_or_default(),
        library_hash
    );
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
    assert!(
        rpc_items[0]["hash"]
            .as_str()
            .is_some_and(|api_hash| hashes_equivalent(api_hash, &library_hash)),
        "Expected RPC hash `{}` to represent the same value as metadata hash `{}`",
        rpc_items[0]["hash"].as_str().unwrap_or_default(),
        library_hash
    );

    project
        .acton()
        .library()
        .fetch(&library_hash)
        .arg("--net")
        .arg("localnet")
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
        .before_start(super::super::support::project::ActonCommand::build)
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
        .arg("localnet")
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
        .verify_network("localnet")
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
        .verify_network("localnet")
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

    assert!(
        is_success_response(&response),
        "v3 message failed: {}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    let payload = response_payload(&response);

    let message_hash = payload["message_hash"]
        .as_str()
        .expect("v3 message message_hash must be a string");
    let message_hash_norm = payload["message_hash_norm"]
        .as_str()
        .expect("v3 message message_hash_norm must be a string");

    assert!(
        !message_hash.is_empty(),
        "Expected non-empty message_hash in v3 message response:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert_eq!(message_hash_norm, message_hash);

    let (invalid_status, invalid) = node.post_json_with_status(
        "/api/v3/message",
        &json!({
            "boc": "not-base64"
        }),
    );

    assert_eq!(invalid_status, 500);
    assert!(
        !is_success_response(&invalid),
        "Expected v3 message error for invalid boc, got:\n{}",
        serde_json::to_string_pretty(&invalid).unwrap_or_default()
    );
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
fn litenode_supports_emulate_v1_emulate_trace() {
    let project = ProjectBuilder::new("litenode-emulate-v1-emulate-trace").build();
    let node = project.litenode().start();

    let before = wait_for_ok_response(&node, "/api/v2/getMasterchainInfo", Duration::from_secs(5));
    let seqno_before = before["result"]["last"]["seqno"]
        .as_i64()
        .expect("masterchain seqno must be integer before emulate");

    let response = node.post_json(
        "/api/emulate/v1/emulateTrace",
        &json!({
            "boc": V3_MESSAGE_TEST_BOC,
            "ignore_chksig": false,
            "include_code_data": true,
            "with_actions": true
        }),
    );

    assert!(
        response["trace"].is_object(),
        "Expected object at emulateTrace.trace:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert!(
        response["transactions"].is_object(),
        "Expected object at emulateTrace.transactions:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert!(
        response["actions"].is_array(),
        "Expected array at emulateTrace.actions when with_actions=true:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert!(
        response["code_cells"].is_object() && response["data_cells"].is_object(),
        "Expected code_cells/data_cells when include_code_data=true:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    let code_cells_non_empty = response["code_cells"]
        .as_object()
        .is_some_and(|cells| !cells.is_empty());
    let data_cells_non_empty = response["data_cells"]
        .as_object()
        .is_some_and(|cells| !cells.is_empty());
    assert!(
        code_cells_non_empty || data_cells_non_empty,
        "Expected non-empty code_cells or data_cells when include_code_data=true:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert!(
        response.get("address_book").is_none() && response.get("metadata").is_none(),
        "address_book/metadata must be absent by default:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    assert_eq!(
        response["mc_block_seqno"].as_i64(),
        Some(seqno_before),
        "Unexpected mc_block_seqno in emulateTrace response:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );

    let response_with_seqno = node.post_json(
        "/api/emulate/v1/emulateTrace",
        &json!({
            "boc": V3_MESSAGE_TEST_BOC,
            "ignore_chksig": false,
            "mc_block_seqno": seqno_before
        }),
    );
    assert!(
        response_with_seqno["trace"].is_object(),
        "emulateTrace with mc_block_seqno failed: {}",
        serde_json::to_string_pretty(&response_with_seqno).unwrap_or_default()
    );
    assert_eq!(
        response_with_seqno.get("actions"),
        None,
        "actions must be omitted when with_actions=false:\n{}",
        serde_json::to_string_pretty(&response_with_seqno).unwrap_or_default()
    );
    assert_eq!(
        response_with_seqno["mc_block_seqno"].as_i64(),
        Some(seqno_before),
        "Unexpected mc_block_seqno for explicit emulate request:\n{}",
        serde_json::to_string_pretty(&response_with_seqno).unwrap_or_default()
    );

    let after = wait_for_ok_response(&node, "/api/v2/getMasterchainInfo", Duration::from_secs(5));
    let seqno_after = after["result"]["last"]["seqno"]
        .as_i64()
        .expect("masterchain seqno must be integer after emulate");
    assert_eq!(
        seqno_after, seqno_before,
        "emulateTrace must not commit state. before={seqno_before}, after={seqno_after}"
    );

    let (invalid_status, invalid) = node.post_json_with_status(
        "/api/emulate/v1/emulateTrace",
        &json!({
            "boc": "not-base64"
        }),
    );
    assert_eq!(
        invalid_status, 400,
        "Invalid emulateTrace request must return 400"
    );
    assert!(
        invalid["error"]
            .as_str()
            .unwrap_or_default()
            .contains("invalid request: invalid boc"),
        "Unexpected error for invalid emulateTrace payload:\n{}",
        serde_json::to_string_pretty(&invalid).unwrap_or_default()
    );

    let (missing_boc_status, missing_boc) =
        node.post_json_with_status("/api/emulate/v1/emulateTrace", &json!({}));
    assert_eq!(
        missing_boc_status, 400,
        "Missing boc emulateTrace request must return 400"
    );
    assert!(
        missing_boc["error"]
            .as_str()
            .unwrap_or_default()
            .contains("invalid request: boc is required"),
        "Unexpected error for missing boc emulateTrace payload:\n{}",
        serde_json::to_string_pretty(&missing_boc).unwrap_or_default()
    );

    let (with_extras_status, with_extras) = node.post_json_with_status(
        "/api/emulate/v1/emulateTrace",
        &json!({
            "boc": V3_MESSAGE_TEST_BOC,
            "include_address_book": true,
            "include_metadata": true
        }),
    );
    assert_eq!(
        with_extras_status, 200,
        "include_address_book/include_metadata emulateTrace request must succeed"
    );
    assert!(
        with_extras.get("address_book").is_some(),
        "include_address_book=true must include `address_book` in response:\n{}",
        serde_json::to_string_pretty(&with_extras).unwrap_or_default()
    );
    assert!(
        with_extras.get("metadata").is_some(),
        "include_metadata=true must include `metadata` in response:\n{}",
        serde_json::to_string_pretty(&with_extras).unwrap_or_default()
    );
    assert!(
        with_extras["address_book"].is_object(),
        "`address_book` must be an object:\n{}",
        serde_json::to_string_pretty(&with_extras).unwrap_or_default()
    );
    assert!(
        with_extras["metadata"].is_object(),
        "`metadata` must be an object:\n{}",
        serde_json::to_string_pretty(&with_extras).unwrap_or_default()
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
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
        .broadcast()
        .verify_network("localnet")
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
    let v3_payload = response_payload(&v3_response);

    assert_eq!(
        v3_payload["balance"].as_str(),
        v2_response["result"]["balance"].as_str()
    );
    assert_eq!(
        v3_payload["code"].as_str(),
        v2_response["result"]["code"].as_str()
    );
    assert_eq!(
        v3_payload["data"].as_str(),
        v2_response["result"]["data"].as_str()
    );
    assert_eq!(
        v3_payload["frozen_hash"].as_str(),
        v2_response["result"]["frozen_hash"].as_str()
    );
    assert_eq!(
        v3_payload["last_transaction_hash"].as_str(),
        v2_response["result"]["last_transaction_id"]["hash"].as_str()
    );
    assert_eq!(
        v3_payload["last_transaction_lt"].as_str(),
        v2_response["result"]["last_transaction_id"]["lt"].as_str()
    );
    assert_eq!(
        v3_payload["status"].as_str(),
        v2_response["result"]["state"].as_str()
    );
    assert_eq!(v3_payload["status"].as_str(), Some("active"));

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
    let v3_missing_default_payload = response_payload(&v3_missing_default);
    let v3_missing_use_v2_false_payload = response_payload(&v3_missing_use_v2_false);

    assert_eq!(
        v2_missing["result"]["state"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(
        v3_missing_default_payload["status"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(
        v3_missing_use_v2_false_payload["status"].as_str(),
        Some("uninitialized")
    );
    assert_eq!(
        v3_missing_default_payload["status"].as_str(),
        v3_missing_use_v2_false_payload["status"].as_str()
    );

    node.stop();
}

#[test]
fn litenode_supports_v3_transactions_endpoints() {
    let project = ProjectBuilder::new("litenode-v3-transactions-endpoints").build();
    let node = project.litenode().start();

    for address in [
        V3_TRANSACTIONS_TEST_ACCOUNT_A,
        V3_TRANSACTIONS_TEST_ACCOUNT_B,
    ] {
        let faucet = node.post_json(
            "/admin/faucet",
            &json!({
                "address": address,
                "amount": 250_000_000u128
            }),
        );
        assert_eq!(
            faucet["ok"].as_bool(),
            Some(true),
            "faucet failed for {address}: {}",
            serde_json::to_string_pretty(&faucet).unwrap_or_default()
        );
    }

    let all_txs_response = wait_for_ok_response(
        &node,
        "/api/v3/transactions?limit=100&sort=desc",
        Duration::from_secs(12),
    );
    let all_txs = v3_transactions_from_response(&all_txs_response);
    assert!(
        !all_txs.is_empty(),
        "Expected non-empty /api/v3/transactions response:\n{}",
        serde_json::to_string_pretty(&all_txs_response).unwrap_or_default()
    );
    assert_transactions_sorted_by_lt_desc(all_txs);

    let tx_for_a = all_txs
        .iter()
        .find(|tx| tx["account"].as_str() == Some(V3_TRANSACTIONS_TEST_ACCOUNT_A))
        .unwrap_or_else(|| {
            panic!(
                "Expected transaction for account {} in /api/v3/transactions:\n{}",
                V3_TRANSACTIONS_TEST_ACCOUNT_A,
                serde_json::to_string_pretty(&all_txs_response).unwrap_or_default()
            )
        });

    let tx_hash = tx_for_a["hash"]
        .as_str()
        .expect("transaction hash must be string")
        .to_owned();
    let tx_lt = tx_for_a["lt"]
        .as_str()
        .expect("transaction lt must be string")
        .parse::<u64>()
        .expect("transaction lt must parse as u64");
    let tx_now = tx_for_a["now"]
        .as_u64()
        .expect("transaction now must be integer") as u32;
    let tx_mc_seqno = tx_for_a["mc_block_seqno"]
        .as_u64()
        .expect("transaction mc_block_seqno must be integer") as u32;
    let in_msg_hash = tx_for_a["in_msg"]["hash"]
        .as_str()
        .expect("transaction in_msg.hash must be string")
        .to_owned();
    let in_msg_body_hash = tx_for_a["in_msg"]["message_content"]["hash"]
        .as_str()
        .expect("transaction in_msg.message_content.hash must be string")
        .to_owned();
    let tx_hash_query = encode_query_component(&tx_hash);
    let in_msg_hash_query = encode_query_component(&in_msg_hash);
    let in_msg_body_hash_query = encode_query_component(&in_msg_body_hash);

    let by_hash = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?hash={tx_hash_query}&limit=10"),
        Duration::from_secs(12),
    );
    let by_hash_txs = v3_transactions_from_response(&by_hash);
    assert_eq!(by_hash_txs.len(), 1);
    assert_eq!(by_hash_txs[0]["hash"].as_str(), Some(tx_hash.as_str()));

    let by_lt = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?lt={tx_lt}&limit=10"),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&by_lt), &tx_hash),
        "Expected to find tx {tx_hash} by lt filter:\n{}",
        serde_json::to_string_pretty(&by_lt).unwrap_or_default()
    );

    let by_account = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?account={V3_TRANSACTIONS_TEST_ACCOUNT_A}&limit=50"),
        Duration::from_secs(12),
    );
    for tx in v3_transactions_from_response(&by_account) {
        assert_eq!(
            tx["account"].as_str(),
            Some(V3_TRANSACTIONS_TEST_ACCOUNT_A),
            "Expected only account-filtered transactions:\n{}",
            serde_json::to_string_pretty(&by_account).unwrap_or_default()
        );
    }

    let by_account_b = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?account={V3_TRANSACTIONS_TEST_ACCOUNT_B}&limit=100"),
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&by_account_b)
            .iter()
            .all(|tx| tx["account"].as_str() == Some(V3_TRANSACTIONS_TEST_ACCOUNT_B)),
        "Unexpected account in single-account filter:\n{}",
        serde_json::to_string_pretty(&by_account_b).unwrap_or_default()
    );

    let excluded_account = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?exclude_account={V3_TRANSACTIONS_TEST_ACCOUNT_A}&limit=100"),
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&excluded_account)
            .iter()
            .all(|tx| tx["account"].as_str() != Some(V3_TRANSACTIONS_TEST_ACCOUNT_A)),
        "exclude_account filter returned excluded account:\n{}",
        serde_json::to_string_pretty(&excluded_account).unwrap_or_default()
    );

    let by_mc_seqno = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?mc_seqno={tx_mc_seqno}&limit=100"),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&by_mc_seqno), &tx_hash),
        "Expected tx {tx_hash} in mc_seqno-filtered response:\n{}",
        serde_json::to_string_pretty(&by_mc_seqno).unwrap_or_default()
    );

    let by_block_id = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/transactions?workchain=0&shard=8000000000000000&seqno={tx_mc_seqno}&limit=100"
        ),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&by_block_id), &tx_hash),
        "Expected tx {tx_hash} in workchain/shard/seqno-filtered response:\n{}",
        serde_json::to_string_pretty(&by_block_id).unwrap_or_default()
    );

    let by_wrong_workchain = wait_for_ok_response(
        &node,
        "/api/v3/transactions?workchain=-1&limit=10",
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&by_wrong_workchain).is_empty(),
        "Expected no transactions for unsupported workchain:\n{}",
        serde_json::to_string_pretty(&by_wrong_workchain).unwrap_or_default()
    );

    let start_utime_strict = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?hash={tx_hash_query}&start_utime={tx_now}&limit=10"),
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&start_utime_strict).is_empty(),
        "start_utime must be strict (after):\n{}",
        serde_json::to_string_pretty(&start_utime_strict).unwrap_or_default()
    );

    let end_utime_strict = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?hash={tx_hash_query}&end_utime={tx_now}&limit=10"),
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&end_utime_strict).is_empty(),
        "end_utime must be strict (before):\n{}",
        serde_json::to_string_pretty(&end_utime_strict).unwrap_or_default()
    );

    let start_lt_inclusive = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?hash={tx_hash_query}&start_lt={tx_lt}&limit=10"),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&start_lt_inclusive), &tx_hash),
        "start_lt must be inclusive:\n{}",
        serde_json::to_string_pretty(&start_lt_inclusive).unwrap_or_default()
    );

    let end_lt_inclusive = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactions?hash={tx_hash_query}&end_lt={tx_lt}&limit=10"),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&end_lt_inclusive), &tx_hash),
        "end_lt must be inclusive:\n{}",
        serde_json::to_string_pretty(&end_lt_inclusive).unwrap_or_default()
    );

    let start_lt_exclusive = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/transactions?hash={tx_hash_query}&start_lt={}&limit=10",
            tx_lt.saturating_add(1)
        ),
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&start_lt_exclusive).is_empty(),
        "Expected no transactions when start_lt is greater than tx.lt:\n{}",
        serde_json::to_string_pretty(&start_lt_exclusive).unwrap_or_default()
    );

    let end_lt_exclusive = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/transactions?hash={tx_hash_query}&end_lt={}&limit=10",
            tx_lt.saturating_sub(1)
        ),
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&end_lt_exclusive).is_empty(),
        "Expected no transactions when end_lt is less than tx.lt:\n{}",
        serde_json::to_string_pretty(&end_lt_exclusive).unwrap_or_default()
    );

    let asc = wait_for_ok_response(
        &node,
        "/api/v3/transactions?limit=100&sort=asc",
        Duration::from_secs(12),
    );
    let asc_txs = v3_transactions_from_response(&asc);
    assert_transactions_sorted_by_lt_asc(asc_txs);

    let desc = wait_for_ok_response(
        &node,
        "/api/v3/transactions?limit=100&sort=desc",
        Duration::from_secs(12),
    );
    let desc_txs = v3_transactions_from_response(&desc);
    assert_transactions_sorted_by_lt_desc(desc_txs);

    if desc_txs.len() > 1 {
        let first_hash = desc_txs[0]["hash"]
            .as_str()
            .expect("transaction hash must be string")
            .to_owned();
        let offset_response = wait_for_ok_response(
            &node,
            "/api/v3/transactions?limit=1&offset=1&sort=desc",
            Duration::from_secs(12),
        );
        let offset_txs = v3_transactions_from_response(&offset_response);
        assert_eq!(offset_txs.len(), 1);
        assert_ne!(offset_txs[0]["hash"].as_str(), Some(first_hash.as_str()));
    }

    let by_msg_hash = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/transactionsByMessage?msg_hash={in_msg_hash_query}&direction=in&limit=50"
        ),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&by_msg_hash), &tx_hash),
        "Expected tx {tx_hash} in transactionsByMessage by msg_hash:\n{}",
        serde_json::to_string_pretty(&by_msg_hash).unwrap_or_default()
    );

    let by_body_hash = wait_for_ok_response(
        &node,
        &format!("/api/v3/transactionsByMessage?body_hash={in_msg_body_hash_query}&limit=50"),
        Duration::from_secs(12),
    );
    assert!(
        contains_tx_hash(v3_transactions_from_response(&by_body_hash), &tx_hash),
        "Expected tx {tx_hash} in transactionsByMessage by body_hash:\n{}",
        serde_json::to_string_pretty(&by_body_hash).unwrap_or_default()
    );

    let direction_out = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/transactionsByMessage?msg_hash={in_msg_hash_query}&direction=out&limit=50"
        ),
        Duration::from_secs(12),
    );
    assert!(
        !contains_tx_hash(v3_transactions_from_response(&direction_out), &tx_hash),
        "Inbound message hash must not match direction=out:\n{}",
        serde_json::to_string_pretty(&direction_out).unwrap_or_default()
    );

    if let Some(opcode) = tx_for_a["in_msg"]["opcode"].as_u64() {
        let by_opcode_hex = wait_for_ok_response(
            &node,
            &format!("/api/v3/transactionsByMessage?opcode=0x{opcode:08x}&limit=50"),
            Duration::from_secs(12),
        );
        assert!(
            contains_tx_hash(v3_transactions_from_response(&by_opcode_hex), &tx_hash),
            "Expected tx {tx_hash} in transactionsByMessage by opcode (hex):\n{}",
            serde_json::to_string_pretty(&by_opcode_hex).unwrap_or_default()
        );

        let opcode_signed = opcode as u32 as i32;
        let by_opcode_signed = wait_for_ok_response(
            &node,
            &format!("/api/v3/transactionsByMessage?opcode={opcode_signed}&limit=50"),
            Duration::from_secs(12),
        );
        assert!(
            contains_tx_hash(v3_transactions_from_response(&by_opcode_signed), &tx_hash),
            "Expected tx {tx_hash} in transactionsByMessage by opcode (signed decimal):\n{}",
            serde_json::to_string_pretty(&by_opcode_signed).unwrap_or_default()
        );
    } else {
        let by_opcode = wait_for_ok_response(
            &node,
            "/api/v3/transactionsByMessage?opcode=0x0&limit=10",
            Duration::from_secs(12),
        );
        assert!(
            is_success_response(&by_opcode),
            "transactionsByMessage with opcode filter must succeed:\n{}",
            serde_json::to_string_pretty(&by_opcode).unwrap_or_default()
        );
    }

    let by_message_offset = wait_for_ok_response(
        &node,
        "/api/v3/transactionsByMessage?limit=1&offset=1",
        Duration::from_secs(12),
    );
    assert!(
        v3_transactions_from_response(&by_message_offset).len() <= 1,
        "Expected limit=1 for transactionsByMessage:\n{}",
        serde_json::to_string_pretty(&by_message_offset).unwrap_or_default()
    );

    let pending = wait_for_ok_response(
        &node,
        "/api/v3/pendingTransactions",
        Duration::from_secs(12),
    );
    let pending_payload = response_payload(&pending);
    assert!(
        pending_payload["address_book"].is_object(),
        "Expected address_book object in pendingTransactions:\n{}",
        serde_json::to_string_pretty(&pending).unwrap_or_default()
    );
    assert!(
        pending_payload["transactions"].is_array(),
        "Expected transactions array in pendingTransactions:\n{}",
        serde_json::to_string_pretty(&pending).unwrap_or_default()
    );

    let pending_with_filters = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/pendingTransactions?account={V3_TRANSACTIONS_TEST_ACCOUNT_A}&trace_id={tx_hash_query}"
        ),
        Duration::from_secs(12),
    );
    let pending_with_filters_payload = response_payload(&pending_with_filters);
    assert!(
        pending_with_filters_payload["transactions"].is_array(),
        "pendingTransactions with filters must return transactions array:\n{}",
        serde_json::to_string_pretty(&pending_with_filters).unwrap_or_default()
    );

    let (status, response) =
        node.get_json_with_status("/api/v3/transactions?shard=8000000000000000");
    assert_v3_bad_request(status, &response, "`shard` requires `workchain`");
    let (status, response) = node.get_json_with_status("/api/v3/transactions?workchain=0&seqno=1");
    assert_v3_bad_request(
        status,
        &response,
        "`seqno` requires both `workchain` and `shard`",
    );
    let (status, response) = node.get_json_with_status("/api/v3/transactions?sort=invalid");
    assert_v3_bad_request(status, &response, "Invalid `sort`");
    let (status, response) = node.get_json_with_status("/api/v3/transactions?limit=0");
    assert_v3_bad_request(status, &response, "`limit` must be between 1 and 1000");
    let (status, response) =
        node.get_json_with_status("/api/v3/transactions?account=not-an-address");
    assert_v3_bad_request(status, &response, "Invalid address format");
    let (status, response) =
        node.get_json_with_status("/api/v3/transactionsByMessage?direction=sideways");
    assert_v3_bad_request(status, &response, "Invalid `direction`");
    let (status, response) = node.get_json_with_status("/api/v3/transactionsByMessage?opcode=oops");
    assert_v3_bad_request(status, &response, "`opcode`");
    let (status, response) =
        node.get_json_with_status("/api/v3/transactionsByMessage?msg_hash=bad-hash");
    assert_v3_bad_request(status, &response, "Invalid hash format");
    let (status, response) =
        node.get_json_with_status("/api/v3/pendingTransactions?account=bad-account");
    assert_v3_bad_request(status, &response, "Invalid address format");
    let (status, response) =
        node.get_json_with_status("/api/v3/pendingTransactions?trace_id=bad-hash");
    assert_v3_bad_request(status, &response, "Invalid hash format");

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
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
        .broadcast()
        .verify_network("localnet")
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

    assert!(
        is_success_response(&response),
        "v3 runGetMethod failed: {}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );
    let payload = response_payload(&response);
    assert_eq!(payload["exit_code"].as_i64(), Some(0));
    assert_eq!(payload["stack"][0]["type"].as_str(), Some("num"));
    assert_eq!(payload["stack"][0]["value"].as_str(), Some("17"));

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
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
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

fn is_success_response(response: &Value) -> bool {
    match response.get("ok").and_then(Value::as_bool) {
        Some(ok) => ok,
        None => response.get("error").is_none(),
    }
}

fn response_payload(response: &Value) -> &Value {
    if response.get("ok").and_then(Value::as_bool) == Some(true) {
        response.get("result").unwrap_or_else(|| {
            panic!(
                "Expected `result` for wrapped successful response:\n{}",
                serde_json::to_string_pretty(response).unwrap_or_default()
            )
        })
    } else if response.get("ok").is_none() && response.get("error").is_none() {
        response
    } else {
        panic!(
            "Expected successful response, got:\n{}",
            serde_json::to_string_pretty(response).unwrap_or_default()
        )
    }
}

fn wait_for_ok_response(
    node: &crate::support::litenode::LiteNodeHandle,
    query: &str,
    timeout: Duration,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json(query);
        if is_success_response(&response) {
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

fn v3_transactions_from_response(response: &Value) -> &[Value] {
    response_payload(response)
        .get("transactions")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| {
            panic!(
                "Expected `transactions` array in response payload:\n{}",
                serde_json::to_string_pretty(response).unwrap_or_default()
            )
        })
}

fn hashes_equivalent(left: &str, right: &str) -> bool {
    normalize_hash_to_bytes(left) == normalize_hash_to_bytes(right)
}

fn normalize_hash_to_bytes(hash: &str) -> Option<[u8; 32]> {
    let trimmed = hash.trim();

    if let Ok(bytes) = hex::decode(trimmed)
        && bytes.len() == 32
    {
        let mut out = [0_u8; 32];
        out.copy_from_slice(&bytes);
        return Some(out);
    }

    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(bytes) = engine.decode(trimmed)
            && bytes.len() == 32
        {
            let mut out = [0_u8; 32];
            out.copy_from_slice(&bytes);
            return Some(out);
        }
    }

    None
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn contains_tx_hash(transactions: &[Value], hash: &str) -> bool {
    transactions
        .iter()
        .any(|tx| tx["hash"].as_str() == Some(hash))
}

fn assert_transactions_sorted_by_lt_asc(transactions: &[Value]) {
    for window in transactions.windows(2) {
        let left = window[0]["lt"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let right = window[1]["lt"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        assert!(
            left <= right,
            "Transactions are not sorted by lt asc:\n{}",
            serde_json::to_string_pretty(transactions).unwrap_or_default()
        );
    }
}

fn assert_transactions_sorted_by_lt_desc(transactions: &[Value]) {
    for window in transactions.windows(2) {
        let left = window[0]["lt"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let right = window[1]["lt"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        assert!(
            left >= right,
            "Transactions are not sorted by lt desc:\n{}",
            serde_json::to_string_pretty(transactions).unwrap_or_default()
        );
    }
}

fn assert_v3_bad_request(status: u16, response: &Value, expected_error_fragment: &str) {
    assert_eq!(
        status,
        400,
        "Expected HTTP 400 for v3 bad request response:\n{}",
        serde_json::to_string_pretty(response).unwrap_or_default()
    );
    if let Some(ok) = response.get("ok").and_then(Value::as_bool) {
        assert!(
            !ok,
            "Expected v3 bad request response:\n{}",
            serde_json::to_string_pretty(response).unwrap_or_default()
        );
    } else {
        assert!(
            response.get("error").is_some(),
            "Expected v3 bad request response with `error` field:\n{}",
            serde_json::to_string_pretty(response).unwrap_or_default()
        );
    }
    assert_eq!(
        response["code"].as_i64(),
        Some(400),
        "Expected code=400 in v3 bad request response:\n{}",
        serde_json::to_string_pretty(response).unwrap_or_default()
    );
    assert!(
        response["error"]
            .as_str()
            .unwrap_or_default()
            .contains(expected_error_fragment),
        "Expected error to contain `{expected_error_fragment}`:\n{}",
        serde_json::to_string_pretty(response).unwrap_or_default()
    );
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
