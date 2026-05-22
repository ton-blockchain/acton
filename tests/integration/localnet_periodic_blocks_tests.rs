use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use snapbox::Data;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

const ADDRESS_ONE: &str = "EQBvDB_H7FFBs0nF4ap_DBdcOrwY_rMIpNVVOR6SWYFHByMJ";
const ADDRESS_TWO: &str = "EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot";
const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;

const TRACE_RECEIVER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const PERIODIC_WAIT_FOR_TRACE_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../../lib/types/big_array"

fun main() {
    val wallet = scripts.wallet("deployer");
    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };

    val txs = net.send(wallet.address, createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: receiverInit,
        },
    }));

    val trace = txs.waitForTrace(true, 30, 100);
    if (trace == null) {
        println("PERIODIC_TRACE_NULL");
        return;
    }

    println("PERIODIC_TRACE_READY");
}
"#;

#[test]
fn periodic_blocks_batch_queued_transactions() {
    let project = ProjectBuilder::new("localnet-periodic-blocks").build();
    let node = project
        .localnet()
        .args(["--periodic-blocks", "--block-interval", "1s"])
        .start();

    let initial_seqno = latest_seqno(&node);
    let empty_seqno = wait_for_next_seqno(&node, initial_seqno);
    let empty_block = block_transactions(&node, empty_seqno);

    let base_url = node.base_url();
    let accepted_one = thread::spawn({
        let base_url = base_url.clone();
        move || {
            post_json(
                &base_url,
                "/acton_fundAccount",
                &json!({
                    "address": ADDRESS_ONE,
                    "amount": 1_000_000_000_u128,
                }),
            )
        }
    });
    let accepted_two = thread::spawn(move || {
        post_json(
            &base_url,
            "/acton_fundAccount",
            &json!({
                "address": ADDRESS_TWO,
                "amount": 2_000_000_000_u128,
            }),
        )
    });

    let accepted_one = accepted_one.join().expect("first request must finish");
    let accepted_two = accepted_two.join().expect("second request must finish");
    let mined_seqno = wait_for_next_seqno(&node, empty_seqno);
    let mined_block = block_transactions(&node, mined_seqno);

    let snapshot = json!({
        "empty_block": {
            "seqno_delta": empty_seqno - initial_seqno,
            "transaction_count": transaction_count(&empty_block),
        },
        "accepted": [
            accepted_one.get("ok").and_then(Value::as_bool).unwrap_or(false),
            accepted_two.get("ok").and_then(Value::as_bool).unwrap_or(false),
        ],
        "accepted_with_message_hash": [
            faucet_response_has_message_hash(&accepted_one),
            faucet_response_has_message_hash(&accepted_two),
        ],
        "batched_block": {
            "seqno_delta": mined_seqno - empty_seqno,
            "transaction_count": transaction_count(&mined_block),
        },
    });

    let snapshot_text =
        serde_json::to_string_pretty(&snapshot).expect("snapshot JSON must serialize") + "\n";
    assert_snapshot(
        "integration/snapshots/localnet-periodic-blocks/periodic_blocks_batch_queued_transactions.txt",
        &snapshot_text,
    );

    node.stop();
}

#[test]
fn periodic_blocks_startup_accounts_wait_for_mined_state() {
    let project = ProjectBuilder::new("localnet-periodic-startup-accounts").build();
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .args([
            "--periodic-blocks",
            "--block-interval",
            "100ms",
            "--accounts",
            "deployer",
        ])
        .ready_timeout(Duration::from_secs(20))
        .start();

    let startup_wallets = node.get_json("/acton_getStartupWallets");
    let status = node.get_json("/acton_nodeInfo");
    let snapshot = json!({
        "startup_wallet_count": startup_wallets["result"]
            .as_array()
            .expect("startup wallets must be an array")
            .len(),
        "has_mined_blocks": status["result"]["last_block_seqno"]
            .as_u64()
            .expect("last block seqno must be a u64") > 0,
    });
    let snapshot_text =
        serde_json::to_string_pretty(&snapshot).expect("snapshot JSON must serialize") + "\n";
    assert_snapshot(
        "integration/snapshots/localnet-periodic-blocks/periodic_blocks_startup_accounts_wait_for_mined_state.txt",
        &snapshot_text,
    );

    node.stop();
}

#[test]
fn periodic_blocks_script_wait_for_trace_resolves() {
    let project = ProjectBuilder::new("localnet-periodic-script-wait-for-trace")
        .contract("receiver", TRACE_RECEIVER_CONTRACT)
        .script_file("wait_for_trace", PERIODIC_WAIT_FOR_TRACE_SCRIPT)
        .build();
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .args([
            "--periodic-blocks",
            "--block-interval",
            "100ms",
            "--accounts",
            "deployer",
        ])
        .ready_timeout(Duration::from_secs(20))
        .start();
    append_localnet_network(project.path(), &node.base_url());

    project
        .acton()
        .script("scripts/wait_for_trace.tolk")
        .verify_network("localnet")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/localnet-periodic-blocks/periodic_blocks_script_wait_for_trace_resolves.stdout.txt",
        );

    node.stop();
}

fn append_localnet_network(project_path: &Path, base_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.localnet]
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
"#
    );
    fs::write(&acton_toml_path, acton_toml).expect("failed to write localnet network config");
}

fn latest_seqno(node: &crate::support::localnet::LocalnetHandle) -> u64 {
    node.get_json("/api/v2/getMasterchainInfo")["result"]["last"]["seqno"]
        .as_u64()
        .expect("masterchain seqno must be a u64")
}

fn wait_for_next_seqno(node: &crate::support::localnet::LocalnetHandle, current: u64) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let seqno = latest_seqno(node);
        if seqno > current {
            return seqno;
        }
        if Instant::now() >= deadline {
            panic!("timed out waiting for localnet seqno after {current}");
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn block_transactions(node: &crate::support::localnet::LocalnetHandle, seqno: u64) -> Value {
    node.get_json(&format!("/api/v2/getBlockTransactionsExt?seqno={seqno}"))
}

fn post_json(base_url: &str, path: &str, payload: &Value) -> Value {
    let url = format!("{base_url}{path}");
    let response = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("HTTP client must be created")
        .post(&url)
        .json(payload)
        .send()
        .unwrap_or_else(|err| panic!("Failed POST {url}: {err}"));
    let status = response.status();
    let body = response
        .text()
        .unwrap_or_else(|err| panic!("Failed to read POST {url} response body: {err}"));
    assert!(
        status.is_success(),
        "POST {url} failed with status {status}: {body}"
    );
    serde_json::from_str(&body)
        .unwrap_or_else(|err| panic!("POST {url} returned invalid JSON: {err}\n{body}"))
}

fn faucet_response_has_message_hash(response: &Value) -> bool {
    response["result"]["result"]["msg_hash"].as_str().is_some()
}

fn transaction_count(block: &Value) -> usize {
    block["result"]["transactions"]
        .as_array()
        .expect("block transactions must be an array")
        .len()
}

fn assert_snapshot(path: &str, content: &str) {
    let mut snapshot_path = std::env::current_dir().expect("current dir must be available");
    snapshot_path.push("tests");
    snapshot_path.push(path);

    assertion().eq(content, Data::read_from(&snapshot_path, None));
}
