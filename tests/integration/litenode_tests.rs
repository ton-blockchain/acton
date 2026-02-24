use crate::common::{assertion, strip_ansi};
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output_preserve_escapes;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

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
