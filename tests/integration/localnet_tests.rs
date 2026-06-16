use crate::common::{assertion, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output_preserve_escapes;
use acton::wallets;
use base64::Engine;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton::ton_core::types::TonAddress;
use ton::ton_wallet::{Mnemonic, TonWallet, WalletVersion};
use ton_api::Network;
use ton_localnet::types::Hash256;
use tycho_types::boc::{Boc, BocRepr};
use tycho_types::cell::{Cell, CellBuilder, CellFamily, CellSliceParts, Store};
use tycho_types::models::{
    AccountState, CurrencyCollection, ExtInMsgInfo, IntAddr, IntMsgInfo, Message, MsgInfo,
    OwnedMessage, ShardAccount, StdAddr,
};
use tycho_types::num::Tokens;
use tycho_types::prelude::HashBytes;

const CHILD_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const DEPLOYER_CONTRACT: &str = r#"
import "../gen/child.code.tolk"

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
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");

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

    println("DEPLOYER_CONTRACT={}", deployerAddress);
}
"#;

const PRINT_SEND_RESULT_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");

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

const DEPLOY_TRACKED_CONTRACT_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");

    val trackedInit = ContractState {
        code: build("tracked"),
        data: createEmptyCell(),
    };
    val trackedAddress = AutoDeployAddress {
        stateInit: trackedInit,
    }.calculateAddress();

    val deployTracked = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: trackedInit,
        },
    });
    net.send(wallet.address, deployTracked);

    println("TRACKED_CONTRACT={}", trackedAddress);
}
"#;

const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;
const DEPLOYER_MNEMONIC: &str = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later";
const CATALOG_WALLET_V4R2_CODE_HASH: &str =
    "feb5ff6820e2ff0d9483e7e0d62c817d846789fb4ae580c878866d959dabd5c0";

const V3_GETTER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun addTen(value: int): int {
    return value + 10;
}
";

const PREVBLOCKS_GETTER_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

@pure
fun prevMcBlocks(): tuple
    asm "PREVMCBLOCKS"

@pure
fun prevKeyBlock(): tuple
    asm "PREVKEYBLOCK"

@pure
fun prevMcBlocks100(): tuple
    asm "PREVMCBLOCKS_100"

fun blockSeqno(block: tuple): int {
    return block.get(2) as int;
}

get fun prevMcBlocksCount(): int {
    return prevMcBlocks().size();
}

get fun latestPrevMcSeqno(): int {
    return blockSeqno(prevMcBlocks().first() as tuple);
}

get fun prevKeySeqno(): int {
    return blockSeqno(prevKeyBlock());
}

get fun prevMcBlocks100FirstSeqno(): int {
    return blockSeqno(prevMcBlocks100().first() as tuple);
}
"#;

const V3_DEPLOY_GETTER_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");

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

    println("GETTER_CONTRACT={}", getterAddress);
}
"#;

const LOCALNET_CACHE_COUNTER_TYPES: &str = r"
struct Storage {
    id: uint32
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

struct (0x7e8764ef) IncreaseCounter {
    increaseBy: uint32
}
";

const LOCALNET_CACHE_COUNTER_CONTRACT: &str = r#"
import "types"

contract Counter {
    storage: Storage
    incomingMessages: IncreaseCounter
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy IncreaseCounter.fromSlice(in.body);
    var storage = lazy Storage.load();
    storage.counter += msg.increaseBy;
    storage.save();
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();
    return storage.counter;
}
"#;

const LOCALNET_CACHE_REFRESH_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../contracts/types"

fun main() {
    val wallet = scripts.wallet("deployer");

    val counterInit = ContractState {
        code: build("counter"),
        data: Storage {
            id: 0,
            counter: 7,
        }.toCell(),
    };
    val counterAddress = AutoDeployAddress {
        stateInit: counterInit,
    }.calculateAddress();

    if (net.send(wallet.address, createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: {
            stateInit: counterInit,
        },
    })).waitForFirstTransaction(true, 40, 25) == null) {
        println("DEPLOY_NULL");
        return;
    }

    val before: int = net.runGetMethod(counterAddress, "currentCounter");
    println("BEFORE={}", before);

    if (net.send(wallet.address, createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: counterAddress,
        body: IncreaseCounter {
            increaseBy: 5,
        },
    })).waitForFirstTransaction(true, 40, 25) == null) {
        println("INCREASE_NULL");
        return;
    }

    val after: int = net.runGetMethod(counterAddress, "currentCounter");
    println("AFTER={}", after);
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
import "../gen/worker.code.tolk"

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
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../gen/worker.code.tolk"

fun main() {
    val wallet = scripts.wallet("deployer");

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

    println("MANAGER_CONTRACT={}", managerAddress);
    println("WORKER_CONTRACT={}", workerAddress);
}
"#;

const DESTROY_WORKER_VIA_MANAGER_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../gen/worker.code.tolk"

fun main() {
    val wallet = scripts.wallet("deployer");

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

    println("MANAGER_CONTRACT={}", managerAddress);
    println("WORKER_CONTRACT={}", workerAddress);
}
"#;

#[test]
fn localnet_starts_and_serves_masterchain_info() {
    let project = ProjectBuilder::new("localnet-smoke-masterchain-info").build();
    let node = project.localnet().start();

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
fn localnet_mines_empty_blocks_on_interval_without_transactions() {
    let project = ProjectBuilder::new("localnet-empty-interval-blocks").build();
    let node = project.localnet().start();

    let initial_seqno = latest_masterchain_seqno(&node);
    let empty_block_seqno = initial_seqno + 1;
    let target_seqno = initial_seqno + 2;
    let reached_seqno =
        wait_for_masterchain_seqno_at_least(&node, target_seqno, Duration::from_secs(5));

    let block = wait_for_ok_response(
        &node,
        &format!("/api/v2/getBlockTransactionsExt?seqno={empty_block_seqno}"),
        Duration::from_secs(5),
    );
    let block_payload = response_payload(&block);
    let transactions = block_payload["transactions"]
        .as_array()
        .expect("getBlockTransactionsExt must return transactions array");

    let snapshot = json!({
        "target_seqno_reached": reached_seqno >= target_seqno,
        "empty_block": {
            "type": block_payload["@type"].as_str(),
            "req_count": block_payload["req_count"].as_u64(),
            "transaction_count": transactions.len(),
            "incomplete": block_payload["incomplete"].as_bool(),
        }
    });

    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_empty_interval_blocks.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_no_mining_mines_only_on_request() {
    let project = ProjectBuilder::new("localnet-no-mining-manual-blocks").build();
    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str("\n[localnet]\nno-mining = true\n");
    fs::write(&acton_toml_path, acton_toml).expect("failed to enable no-mining in Acton.toml");

    let node = project.localnet().require_auth().start();
    let token = node
        .auth_token()
        .expect("protected localnet test must expose auth token");
    let initial_seqno = latest_masterchain_seqno(&node);
    thread::sleep(Duration::from_millis(250));
    let after_sleep_seqno = latest_masterchain_seqno(&node);

    let mine_default = node.post_json("/acton_mine", &json!({}));
    let default_payload = response_payload(&mine_default);
    let after_default_seqno = latest_masterchain_seqno(&node);

    project
        .acton()
        .arg("localnet")
        .arg("mine")
        .arg("2")
        .arg("--port")
        .arg(&node.port().to_string())
        .env("ACTON_LOCALNET_AUTH_TOKEN", token)
        .run()
        .success();
    let after_cli_seqno = latest_masterchain_seqno(&node);

    let target = "0:2222222222222222222222222222222222222222222222222222222222222222";
    let before_faucet = node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let fund = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": target,
            "amount": 1_000_000_000_u64
        }),
    );
    let after_faucet_before_mine =
        node.get_json(&format!("/api/v2/getAddressInformation?address={target}"));
    let mine_faucet = node.post_json("/acton_mine", &json!({}));
    let after_faucet_mine =
        wait_for_address_balance_at_least(&node, target, 1_000_000_000, Duration::from_secs(3));

    let mut invalid_zero = node.post_json("/acton_mine", &json!({ "blocks": 0 }));
    normalize_extra_for_snapshot(&mut invalid_zero);

    let snapshot = json!({
        "config_no_mining_kept_seqno": after_sleep_seqno == initial_seqno,
        "default_mine": {
            "ok": mine_default["ok"].as_bool(),
            "blocks_mined": default_payload["blocks_mined"].as_u64(),
            "block_count": default_payload["blocks"].as_array().map(Vec::len),
            "seqno_delta": after_default_seqno - initial_seqno,
        },
        "cli_mine": {
            "seqno_delta": after_cli_seqno - after_default_seqno,
        },
        "queued_faucet": {
            "fund_ok": fund["ok"].as_bool(),
            "balance_before": parse_address_balance(&before_faucet).to_string(),
            "balance_after_queue": parse_address_balance(&after_faucet_before_mine).to_string(),
            "mine_ok": mine_faucet["ok"].as_bool(),
            "balance_after_mine": parse_address_balance(&after_faucet_mine).to_string(),
        },
        "invalid_zero": {
            "ok": invalid_zero["ok"].as_bool(),
            "code": invalid_zero["code"].as_i64(),
            "error": invalid_zero["error"].as_str(),
        }
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_no_mining_manual_blocks.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_no_mining_bootstraps_startup_accounts_in_fork_mode() {
    let project = ProjectBuilder::new("localnet-no-mining-startup-accounts-fork").build();
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let source_node = project.localnet().start();
    append_custom_localnet_network(project.path(), "fork-source", &source_node.base_url());

    let forked_node = project
        .localnet()
        .args([
            "--fork-net",
            "custom:fork-source",
            "--accounts",
            "deployer",
            "--no-mining",
        ])
        .start();

    let startup_wallets = forked_node.get_json("/acton_getStartupWallets");
    let startup_wallets_payload = response_payload(&startup_wallets);
    let wallets = startup_wallets_payload
        .as_array()
        .expect("startup wallets response must contain an array");
    let wallet = wallets
        .first()
        .expect("forked no-mining localnet must bootstrap deployer wallet");
    let address = wallet["address"]
        .as_str()
        .expect("startup wallet must expose address");

    wait_until_address_state_active(&forked_node, address, Duration::from_secs(5));
    let node_info = forked_node.get_json("/acton_nodeInfo");
    let initial_seqno = latest_masterchain_seqno(&forked_node);
    thread::sleep(Duration::from_millis(250));
    let after_sleep_seqno = latest_masterchain_seqno(&forked_node);
    let mine_response = forked_node.post_json("/acton_mine", &json!({}));
    let after_mine_seqno = latest_masterchain_seqno(&forked_node);

    let snapshot = json!({
        "startup_wallet": {
            "count": wallets.len(),
            "name": wallet["name"].as_str(),
            "version": wallet["version"].as_str(),
            "network": wallet["network"].as_str(),
            "address_present": !address.is_empty(),
        },
        "state_source": {
            "state_source": node_info["result"]["state_source"].as_str(),
            "fork_network": node_info["result"]["fork_network"].as_str(),
            "fork_block_number": node_info["result"]["fork_block_number"].as_u64(),
        },
        "manual_mining": {
            "seqno_stable_after_start": after_sleep_seqno == initial_seqno,
            "mine_ok": mine_response["ok"].as_bool(),
            "seqno_delta": after_mine_seqno - after_sleep_seqno,
        }
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!(
            "snapshots/localnet/test_localnet_no_mining_startup_accounts_fork.summary.json"
        ),
    );

    forked_node.stop();
    source_node.stop();
}

#[test]
fn localnet_records_api_calls_for_dashboard() {
    let project = ProjectBuilder::new("localnet-api-calls-dashboard").build();
    let node = project.localnet().start();

    let mut initial_log = node.get_json("/acton_getApiCalls");
    normalize_api_calls_for_snapshot(&mut initial_log);

    let _admin_wallets = node.get_json("/acton_getStartupWallets");
    let _admin_status = node.get_json("/acton_nodeInfo");
    let _v2_status = node.get_json("/api/v2/getMasterchainInfo");
    let _successful_rpc = node.post_json(
        "/api/v2/jsonRPC",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getMasterchainInfo",
            "params": {}
        }),
    );
    let (failed_status, _failed_rpc) = node.post_json_with_status(
        "/api/v2/jsonRPC",
        &json!({
            "jsonrpc": "2.0",
            "id": "missing",
            "method": "missingMethod",
            "params": {}
        }),
    );

    let mut logged_calls = node.get_json("/acton_getApiCalls?limit=10");
    normalize_api_calls_for_snapshot(&mut logged_calls);

    let snapshot = json!({
        "initial_log": initial_log,
        "failed_status": failed_status,
        "logged_calls": logged_calls,
    });

    assertion().eq(
        format!("{}\n", pretty_json_for_snapshot(&snapshot, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_api_calls_dashboard.response.json"),
    );

    node.stop();
}

#[test]
fn localnet_serves_get_shard_account_cell_for_empty_account() {
    let project = ProjectBuilder::new("localnet-shard-account-cell-empty").build();
    let node = project.localnet().start();
    let address = "0:1111111111111111111111111111111111111111111111111111111111111111";

    let mut response = node.get_json(&format!("/api/v2/getShardAccountCell?address={address}"));
    normalize_extra_for_snapshot(&mut response);

    let _parsed = decode_shard_account_cell_response(&response);

    let mut rpc_response = node.post_json(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getShardAccountCell",
            "params": {
                "address": address
            }
        }),
    );
    normalize_extra_for_snapshot(&mut rpc_response);
    let _rpc_parsed = decode_shard_account_cell_response(&rpc_response);

    let mut invalid_response =
        node.get_json("/api/v2/getShardAccountCell?address=not-a-ton-address");
    normalize_extra_for_snapshot(&mut invalid_response);

    let snapshot = json!({
        "empty_http": response,
        "empty_json_rpc": rpc_response,
        "invalid_address": invalid_response,
    });

    let response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&snapshot).expect("Failed to serialize JSON response")
    );
    assertion().eq(
        normalize_output_preserve_escapes(&response_json, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_get_shard_account_cell_empty.response.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_serves_get_shard_account_cell_for_active_account() {
    let project = ProjectBuilder::new("localnet-shard-account-cell-active")
        .contract("tracked", CHILD_CONTRACT)
        .script_file("deploy_tracked", DEPLOY_TRACKED_CONTRACT_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let output = project
        .acton()
        .script("scripts/deploy_tracked.tolk")
        .verify_network("localnet")
        .run()
        .success();
    let stdout = output.get_stdout();
    let tracked_address = extract_marker_value(&stdout, "TRACKED_CONTRACT=");

    wait_until_address_state_active(&node, &tracked_address, Duration::from_secs(12));
    let raw_address = unpack_address(&node, &tracked_address);

    let http_response = wait_for_ok_response(
        &node,
        &format!("/api/v2/getShardAccountCell?address={tracked_address}"),
        Duration::from_secs(12),
    );
    let rpc_response = node.post_json(
        "/api/v2",
        &json!({
            "jsonrpc": "2.0",
            "id": "tracked",
            "method": "getShardAccountCell",
            "params": {
                "address": tracked_address
            }
        }),
    );

    let http_boc = shard_account_cell_boc64(&http_response);
    let rpc_boc = shard_account_cell_boc64(&rpc_response);
    let snapshot = json!({
        "http": summarize_shard_account_cell_response(&http_response, Some(&raw_address)),
        "json_rpc": summarize_shard_account_cell_response(&rpc_response, Some(&raw_address)),
        "same_cell_bytes": http_boc == rpc_boc,
    });

    let response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&snapshot)
            .expect("Failed to serialize active shard account summary")
    );
    assertion().eq(
        normalize_output_preserve_escapes(&response_json, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_get_shard_account_cell_active.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_serves_embedded_ui_and_spa_routes() {
    let project = ProjectBuilder::new("localnet-ui-smoke").build();
    let node = project.localnet().start();
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client for Localnet UI smoke test");

    for path in ["", "/explorer"] {
        let url = format!("{}{}", node.base_url(), path);
        let response = client
            .get(&url)
            .send()
            .unwrap_or_else(|error| panic!("Failed GET {url}: {error}"));
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|error| panic!("Failed to read GET {url} response body: {error}"));

        assert!(
            status.is_success(),
            "GET {url} failed with status {status}: {body}"
        );
        assert!(
            body.contains("<title>TON Localnet UI</title>"),
            "Expected Localnet UI HTML from {url}, got:\n{body}"
        );
        assert!(
            body.contains("<div id=\"root\"></div>"),
            "Expected Localnet UI root container from {url}, got:\n{body}"
        );
    }

    let response = node.get_json("/api/v2/getMasterchainInfo");
    assert_eq!(response["ok"].as_bool(), Some(true));

    node.stop();
}

#[test]
fn localnet_require_auth_protects_http_api() {
    let project = ProjectBuilder::new("localnet-require-auth").build();
    let node = project.localnet().require_auth().start();
    let token = node
        .auth_token()
        .expect("protected localnet test must expose auth token");
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client");

    let unauthorized_v2 = get_json_with_status(
        &client,
        &format!("{}/api/v2/getMasterchainInfo", node.base_url()),
    );
    let unauthorized_control =
        get_json_with_status(&client, &format!("{}/acton_nodeInfo", node.base_url()));
    let unauthorized_emulate = post_json_with_status(
        &client,
        &format!("{}/api/emulate/v1/emulateTrace", node.base_url()),
        &json!({}),
    );
    let unauthorized_sse = post_json_with_status(
        &client,
        &format!("{}/api/streaming/v2/sse", node.base_url()),
        &json!({
            "addresses": [V3_TRANSACTIONS_TEST_ACCOUNT_A],
            "types": ["transactions"]
        }),
    );
    let ui_response = client
        .get(node.base_url())
        .send()
        .expect("UI request must be sent");
    let ui_status = ui_response.status().as_u16();
    let ui_body = ui_response
        .text()
        .expect("UI response body must be readable");

    let authorized_v2 = client
        .get(format!("{}/api/v2/getMasterchainInfo", node.base_url()))
        .bearer_auth(token)
        .send()
        .expect("authorized v2 request must be sent");
    let authorized_v2_status = authorized_v2.status().as_u16();
    let authorized_v2_json: Value = authorized_v2
        .json()
        .expect("authorized v2 response must be JSON");

    let api_key_v2 = client
        .get(format!("{}/api/v2/getMasterchainInfo", node.base_url()))
        .header("X-API-Key", token)
        .send()
        .expect("x-api-key v2 request must be sent");
    let api_key_v2_status = api_key_v2.status().as_u16();
    let api_key_v2_json: Value = api_key_v2
        .json()
        .expect("x-api-key v2 response must be JSON");

    let options_status = client
        .request(
            reqwest::Method::OPTIONS,
            format!("{}/api/v2/getMasterchainInfo", node.base_url()),
        )
        .header("Origin", "http://127.0.0.1:3000")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .expect("OPTIONS request must be sent")
        .status()
        .as_u16();

    let status_output = project
        .acton()
        .arg("localnet")
        .arg("status")
        .arg("--json")
        .arg("--port")
        .arg(&node.port().to_string())
        .env("ACTON_LOCALNET_AUTH_TOKEN", token)
        .run()
        .success();
    let mut status_payload: Value = serde_json::from_str(&status_output.get_stdout())
        .expect("protected status output must be JSON");
    normalize_localnet_status_json(&mut status_payload, node.port());

    project
        .acton()
        .arg("localnet")
        .arg("airdrop")
        .arg(V3_TRANSACTIONS_TEST_ACCOUNT_A)
        .arg("--amount")
        .arg("0.25")
        .arg("--port")
        .arg(&node.port().to_string())
        .env("ACTON_LOCALNET_AUTH_TOKEN", token)
        .run()
        .success();

    let summary = json!({
        "unauthorized": {
            "v2": summarize_auth_error(unauthorized_v2),
            "control": summarize_auth_error(unauthorized_control),
            "emulate": summarize_auth_error(unauthorized_emulate),
            "sse": summarize_auth_error(unauthorized_sse),
        },
        "authorized": {
            "bearer_status": authorized_v2_status,
            "bearer_ok": authorized_v2_json["ok"].as_bool(),
            "x_api_key_status": api_key_v2_status,
            "x_api_key_ok": api_key_v2_json["ok"].as_bool(),
            "options_status": options_status,
            "status_command_running": status_payload["running"].as_bool(),
        },
        "ui": {
            "status": ui_status,
            "html_shell": ui_body.contains("<div id=\"root\"></div>"),
            "leaks_token": ui_body.contains(token),
            "has_bootstrap_token": ui_body.contains("__ACTON_LOCALNET__"),
        },
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_require_auth.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_batches_address_name_lookup() {
    let project = ProjectBuilder::new("localnet-address-name-batch").build();
    let node = project.localnet().start();
    let named_address = "0:2222222222222222222222222222222222222222222222222222222222222222";
    let unnamed_address = "0:3333333333333333333333333333333333333333333333333333333333333333";

    node.post_json(
        "/acton_setAddressName",
        &json!({
            "address": named_address,
            "name": "treasury"
        }),
    );

    let mut response = node.get_json(&format!(
        "/acton_getAddressName?address={}&address={}",
        encode_query_component(named_address),
        encode_query_component(unnamed_address)
    ));
    normalize_extra_for_snapshot(&mut response);
    let response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&response)
            .expect("Failed to serialize address name batch response")
    );

    assertion().eq(
        response_json,
        snapbox::file!("snapshots/localnet/test_localnet_address_name_batch.response.json"),
    );

    node.stop();
}

#[test]
fn localnet_supports_pre_start_commands_and_get_out_msg_queue_size() {
    let project = ProjectBuilder::new("localnet-pre-start-commands")
        .contract("child", CHILD_CONTRACT)
        .contract_with_deps("deployer", DEPLOYER_CONTRACT, vec!["child"])
        .script_file("deploy", DEPLOY_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
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
        snapbox::file!("snapshots/localnet/test_localnet_get_out_msg_queue_size.response.json"),
    );

    let script_result = project
        .acton()
        .script("scripts/deploy.tolk")
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

    let parent_transaction = transactions
        .iter()
        .find(|tx| {
            tx.get("out_msgs")
                .and_then(Value::as_array)
                .is_some_and(|out| !out.is_empty())
        })
        .expect("deployer trace must include a parent transaction with out messages");
    let parent_tx_hash = parent_transaction
        .pointer("/transaction_id/hash")
        .and_then(Value::as_str)
        .expect("parent transaction hash must be present");
    let traces = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/traces?hash={}",
            encode_query_component(parent_tx_hash)
        ),
        Duration::from_secs(12),
    );
    let trace_payload = response_payload(&traces);
    let trace = trace_payload["traces"]
        .as_array()
        .and_then(|items| items.first())
        .expect("v3 traces must contain a first trace");
    let transactions_map = trace["transactions"]
        .as_object()
        .expect("v3 trace must include transactions map");
    let parent_v3_tx = transactions_map
        .get(parent_tx_hash)
        .unwrap_or_else(|| panic!("v3 trace must include parent transaction {parent_tx_hash}"));
    let legacy_child_lts = parent_v3_tx["child_transactions"]
        .as_array()
        .expect("v3 transaction entry must include legacy child_transactions");
    let trace_root_children = trace["trace"]["children"].as_array();
    let parent_trace_node = find_trace_node(&trace["trace"], parent_tx_hash);
    let parent_trace_children = parent_trace_node
        .and_then(|node| node.get("children"))
        .and_then(Value::as_array);
    let first_tree_child_lt = parent_trace_children
        .and_then(|children| children.first())
        .and_then(|child| child.get("tx_hash"))
        .and_then(Value::as_str)
        .and_then(|child_hash| transactions_map.get(child_hash))
        .and_then(|tx| tx.get("lt"))
        .and_then(Value::as_str);
    let first_legacy_child_lt = legacy_child_lts.first().and_then(Value::as_str);
    let parent_account_state_after = &parent_v3_tx["account_state_after"];

    let trace_legacy_summary = json!({
        "trace_root_children_count": trace_root_children.map_or(0, Vec::len),
        "parent_trace_node_present": parent_trace_node.is_some(),
        "parent_trace_node_children_count": parent_trace_children.map_or(0, Vec::len),
        "parent_legacy_child_transactions_count": legacy_child_lts.len(),
        "first_child_lt_matches_legacy_child_transaction": first_tree_child_lt.is_some()
            && first_tree_child_lt == first_legacy_child_lt,
        "parent_account_state_after_has_code_boc": parent_account_state_after["code_boc"]
            .as_str()
            .is_some_and(|boc| !boc.is_empty()),
        "parent_account_state_after_has_data_boc": parent_account_state_after["data_boc"]
            .as_str()
            .is_some_and(|boc| !boc.is_empty()),
    });
    let trace_legacy_summary_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&trace_legacy_summary)
            .expect("Failed to serialize v3 trace compatibility summary")
    );
    assertion().eq(
        trace_legacy_summary_json,
        snapbox::file!("snapshots/localnet/test_localnet_v3_trace_children.summary.json"),
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
        snapbox::file!("snapshots/localnet/test_localnet_get_transactions_std.response.json"),
    );

    node.stop();
}

#[test]
fn localnet_can_rate_limit_api_endpoints_to_simulate_provider_limits() {
    let project = ProjectBuilder::new("localnet-rate-limit").build();
    let node = project.localnet().args(["--rate-limit", "1"]).start();

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

    let (admin_status, admin_response) = node.get_json_with_status("/acton_nodeInfo");
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
fn localnet_can_update_response_delay_while_running() {
    let project = ProjectBuilder::new("localnet-response-delay-runtime").build();
    let node = project.localnet().start();

    let initial_info_response = node.get_json("/acton_nodeInfo");
    let initial_info = response_payload(&initial_info_response);
    let initial_response_delay_ms = initial_info["network_conditions"]["response_delay_ms"].clone();

    let set_conditions_response = node.post_json(
        "/acton_setNetworkConditions",
        &json!({ "response_delay_ms": 250 }),
    );
    let set_conditions = response_payload(&set_conditions_response);
    let set_response_delay_ms = set_conditions["response_delay_ms"].clone();

    let info_after_set_response = node.get_json("/acton_nodeInfo");
    let info_after_set = response_payload(&info_after_set_response);
    let node_info_response_delay_ms =
        info_after_set["network_conditions"]["response_delay_ms"].clone();

    let started_at = Instant::now();
    let delayed_api_response = node.get_json("/api/v2/getMasterchainInfo");
    let delayed_api_elapsed = started_at.elapsed();

    let reset_conditions_response = node.post_json(
        "/acton_setNetworkConditions",
        &json!({ "response_delay_ms": 0 }),
    );
    let reset_conditions = response_payload(&reset_conditions_response);
    let reset_response_delay_ms = reset_conditions["response_delay_ms"].clone();

    let info_after_reset_response = node.get_json("/acton_nodeInfo");
    let info_after_reset = response_payload(&info_after_reset_response);
    let node_info_after_reset_response_delay_ms =
        info_after_reset["network_conditions"]["response_delay_ms"].clone();

    let summary = json!({
        "initial_response_delay_ms": initial_response_delay_ms,
        "set_response_delay_ms": set_response_delay_ms,
        "node_info_response_delay_ms": node_info_response_delay_ms,
        "api_delay_observed": delayed_api_elapsed >= Duration::from_millis(220),
        "api_request_ok": delayed_api_response["ok"].as_bool(),
        "reset_response_delay_ms": reset_response_delay_ms,
        "node_info_after_reset_response_delay_ms": node_info_after_reset_response_delay_ms,
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_response_delay_runtime.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_status_json_reports_running_node_details() {
    let project = ProjectBuilder::new("localnet-status-running").build();
    let node = project.localnet().start();
    let output = project
        .acton()
        .arg("localnet")
        .arg("status")
        .arg("--json")
        .arg("--port")
        .arg(&node.port().to_string())
        .run()
        .success();

    let mut payload: Value =
        serde_json::from_str(&output.get_stdout()).expect("status --json must return valid JSON");
    normalize_localnet_status_json(&mut payload, node.port());
    assertion().eq(
        pretty_json_for_snapshot(&payload, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_status_json_running.response.json"),
    );

    node.stop();
}

#[test]
fn localnet_status_human_reports_running_node_details() {
    let project = ProjectBuilder::new("localnet-status-human-running").build();
    let node = project.localnet().start();
    let output = project
        .acton()
        .arg("localnet")
        .arg("status")
        .arg("--port")
        .arg(&node.port().to_string())
        .run()
        .success();

    assertion().eq(
        normalize_localnet_status_stdout(&strip_ansi(&output.get_stdout()), node.port()),
        snapbox::file!("snapshots/localnet/test_localnet_status_human_running.stdout.txt"),
    );

    node.stop();
}

#[test]
fn localnet_status_json_reports_stopped_node() {
    let project = ProjectBuilder::new("localnet-status-stopped").build();
    let node = project.localnet().start();
    let port = node.port();
    node.stop();

    let output = project
        .acton()
        .arg("localnet")
        .arg("status")
        .arg("--json")
        .arg("--port")
        .arg(&port.to_string())
        .run()
        .success();

    let mut payload: Value =
        serde_json::from_str(&output.get_stdout()).expect("status --json must return valid JSON");
    normalize_localnet_status_json(&mut payload, port);
    assertion().eq(
        pretty_json_for_snapshot(&payload, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_status_json_stopped.response.json"),
    );
}

#[test]
fn localnet_status_json_reports_stopped_for_non_localnet_http_server() {
    let project = ProjectBuilder::new("localnet-status-non-localnet-http").build();
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind fake status server");
    listener
        .set_nonblocking(true)
        .expect("failed to make fake status server non-blocking");
    let port = listener
        .local_addr()
        .expect("failed to resolve fake status server address")
        .port();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0u8; 1024];
                    let _ = stream.read(&mut request);
                    let body = "<html>not an acton localnet</html>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("failed to write fake status response");
                    return;
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(err) => panic!("failed to accept fake status request: {err}"),
            }
        }
    });

    let output = project
        .acton()
        .arg("localnet")
        .arg("status")
        .arg("--json")
        .arg("--port")
        .arg(&port.to_string())
        .run()
        .success();
    server
        .join()
        .expect("fake status server thread must finish");

    let mut payload: Value =
        serde_json::from_str(&output.get_stdout()).expect("status --json must return valid JSON");
    normalize_localnet_status_json(&mut payload, port);
    assertion().eq(
        pretty_json_for_snapshot(&payload, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_status_json_non_localnet_http.response.json"
        ),
    );
}

#[test]
fn localnet_admin_dump_and_load_state_roundtrip() {
    let project = ProjectBuilder::new("localnet-admin-state-roundtrip").build();
    let node = project.localnet().start();
    let snapshot_path = project.path().join("localnet-state.json");
    let address_before = "0:1111111111111111111111111111111111111111111111111111111111111111";
    let address_after = "0:2222222222222222222222222222222222222222222222222222222222222222";

    let funded_before = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": address_before,
            "amount": 1_000_000_000u128,
        }),
    );

    let before_info = wait_for_address_balance_at_least(
        &node,
        address_before,
        1_000_000_000,
        Duration::from_secs(5),
    );
    let before_balance = parse_address_balance(&before_info);

    let dumped = node.post_json(
        "/acton_dumpState",
        &json!({
            "path": snapshot_path.display().to_string(),
        }),
    );

    let funded_after = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": address_after,
            "amount": 2_000_000_000u128,
        }),
    );

    let after_info = wait_for_address_balance_at_least(
        &node,
        address_after,
        2_000_000_000,
        Duration::from_secs(5),
    );
    let after_balance_before_load = parse_address_balance(&after_info);

    let loaded = node.post_json(
        "/acton_loadState",
        &json!({
            "path": snapshot_path.display().to_string(),
        }),
    );

    let before_info_reloaded = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={address_before}"),
        Duration::from_secs(5),
    );
    let before_balance_after_load = parse_address_balance(&before_info_reloaded);

    let after_info_reloaded = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={address_after}"),
        Duration::from_secs(5),
    );
    let after_balance_after_load = parse_address_balance(&after_info_reloaded);

    let snapshot = json!({
        "fund_before": summarize_admin_response(&funded_before),
        "dump": summarize_admin_response(&dumped),
        "snapshot_file_created": snapshot_path.is_file(),
        "fund_after": summarize_admin_response(&funded_after),
        "load": summarize_admin_response(&loaded),
        "balances": {
            "before_after_fund": before_balance.to_string(),
            "after_after_fund": after_balance_before_load.to_string(),
            "before_after_load": before_balance_after_load.to_string(),
            "after_after_load": after_balance_after_load.to_string(),
        }
    });

    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_admin_dump_and_load_state_roundtrip.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_admin_set_shard_account_updates_selected_account() {
    let project = ProjectBuilder::new("localnet-admin-set-shard-account").build();
    let node = project.localnet().start();
    let source = "0:1111111111111111111111111111111111111111111111111111111111111111";
    let target = "0:2222222222222222222222222222222222222222222222222222222222222222";

    let fund = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": source,
            "amount": 1_000_000_000u128,
        }),
    );
    let source_info =
        wait_for_address_balance_at_least(&node, source, 1_000_000_000, Duration::from_secs(5));
    let source_balance = parse_address_balance(&source_info);
    let source_shard_response = wait_for_ok_response(
        &node,
        &format!("/api/v2/getShardAccountCell?address={source}"),
        Duration::from_secs(5),
    );
    let source_shard_boc = shard_account_cell_boc64(&source_shard_response).to_owned();

    let set = node.post_json(
        "/acton_setShardAccount",
        &json!({
            "address": target,
            "shard_account": source_shard_boc,
        }),
    );
    let target_info_after_set = wait_for_ok_response(
        &node,
        &format!("/api/v2/getAddressInformation?address={target}"),
        Duration::from_secs(5),
    );
    let target_balance_after_set = parse_address_balance(&target_info_after_set);
    let target_shard_response = wait_for_ok_response(
        &node,
        &format!("/api/v2/getShardAccountCell?address={target}"),
        Duration::from_secs(5),
    );
    let target_shard_boc = shard_account_cell_boc64(&target_shard_response).to_owned();

    let invalid = node.post_json(
        "/acton_setShardAccount",
        &json!({
            "address": target,
            "shard_account": "not-a-boc",
        }),
    );

    let snapshot = json!({
        "fund": summarize_admin_response(&fund),
        "set": summarize_admin_response(&set),
        "source_balance": source_balance.to_string(),
        "target_balance_after_set": target_balance_after_set.to_string(),
        "target_cell_matches_source": target_shard_boc == source_shard_boc,
        "target_after_set": summarize_shard_account_cell_response(&target_shard_response, None),
        "invalid": summarize_admin_response(&invalid),
    });

    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_admin_set_shard_account_updates_selected_account.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_raw_internal_messages_use_acton_endpoint() {
    let project = ProjectBuilder::new("localnet-raw-internal-message").build();
    let node = project.localnet().start();
    let internal_boc = build_localnet_internal_boc();
    let target = "0:2222222222222222222222222222222222222222222222222222222222222222";

    let send_boc = node.post_json(
        "/api/v2/sendBoc",
        &json!({
            "boc": internal_boc,
        }),
    );
    let send_boc_return_hash = node.post_json(
        "/api/v2/sendBocReturnHash",
        &json!({
            "boc": internal_boc,
        }),
    );
    let (json_rpc_send_boc_status, json_rpc_send_boc) = node.post_json_with_status(
        "/api/v2/jsonRPC",
        &json!({
            "jsonrpc": "2.0",
            "id": "send",
            "method": "sendBoc",
            "params": {
                "boc": internal_boc,
            },
        }),
    );
    let (json_rpc_send_boc_return_hash_status, json_rpc_send_boc_return_hash) = node
        .post_json_with_status(
            "/api/v2/jsonRPC",
            &json!({
                "jsonrpc": "2.0",
                "id": "send-return-hash",
                "method": "sendBocReturnHash",
                "params": {
                    "boc": internal_boc,
                },
            }),
        );
    let (message_status, message) = node.post_json_with_status(
        "/api/v3/message",
        &json!({
            "boc": internal_boc,
        }),
    );
    let acton_send = node.post_json(
        "/acton_sendInternalMessage",
        &json!({
            "boc": internal_boc,
        }),
    );
    let target_info =
        wait_for_address_balance_at_least(&node, target, 50_000_000, Duration::from_secs(5));

    let snapshot = json!({
        "send_boc": summarize_admin_response(&send_boc),
        "send_boc_return_hash": summarize_admin_response(&send_boc_return_hash),
        "json_rpc_send_boc_status": json_rpc_send_boc_status,
        "json_rpc_send_boc": summarize_admin_response(&json_rpc_send_boc),
        "json_rpc_send_boc_return_hash_status": json_rpc_send_boc_return_hash_status,
        "json_rpc_send_boc_return_hash": summarize_admin_response(&json_rpc_send_boc_return_hash),
        "message_status": message_status,
        "message": message,
        "acton_send": summarize_admin_response(&acton_send),
        "target_balance": parse_address_balance(&target_info).to_string(),
    });

    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_raw_internal_messages_use_acton_endpoint.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_script_println_net_send_in_broadcast_shows_synthetic_hint() {
    let project = ProjectBuilder::new("localnet-broadcast-println-net-send")
        .contract("child", CHILD_CONTRACT)
        .script_file("deploy", PRINT_SEND_RESULT_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let output = project
        .acton()
        .script("scripts/deploy.tolk")
        .verify_network("localnet")
        .run()
        .success();

    output
        .assert_contains("Broadcast send (synthetic result)")
        .assert_not_contains("compute phase skipped")
        .assert_snapshot_matches(
            "integration/snapshots/localnet/test_localnet_script_println_net_send_in_broadcast_shows_synthetic_hint.stdout.txt",
        );

    node.stop();
}

#[test]
fn localnet_script_invalidates_remote_cache_after_broadcast_before_get_method() {
    let project = ProjectBuilder::new("localnet-script-cache-refresh")
        .file("contracts/types", LOCALNET_CACHE_COUNTER_TYPES)
        .contract("counter", LOCALNET_CACHE_COUNTER_CONTRACT)
        .script_file("cache_refresh", LOCALNET_CACHE_REFRESH_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let output = project
        .acton()
        .script("scripts/cache_refresh.tolk")
        .verify_network("localnet")
        .run()
        .success();

    output
        .assert_contains("BEFORE=7")
        .assert_contains("AFTER=12")
        .assert_snapshot_matches(
            "integration/snapshots/localnet/test_localnet_script_invalidates_remote_cache_after_broadcast_before_get_method.stdout.txt",
        );

    node.stop();
}

#[test]
fn localnet_supports_try_locate_transaction_endpoints() {
    let project = ProjectBuilder::new("localnet-try-locate-endpoints")
        .contract("child", CHILD_CONTRACT)
        .contract_with_deps("deployer", DEPLOYER_CONTRACT, vec!["child"])
        .script_file("deploy", DEPLOY_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy.tolk")
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
fn localnet_supports_library_publish_and_get_libraries_endpoint() {
    let project = ProjectBuilder::new("localnet-library-support")
        .contract("library_contract", LIBRARY_CONTRACT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
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
fn localnet_supports_library_ref_contract_deploy_and_destroy_flow() {
    let project = ProjectBuilder::new("localnet-library-ref-contract-flow")
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
        .localnet()
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
        .run()
        .success();

    let deploy_result = project
        .acton()
        .script("scripts/deploy_manager_and_worker.tolk")
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
fn localnet_supports_config_endpoints() {
    let project = ProjectBuilder::new("localnet-config-endpoints").build();
    let node = project.localnet().start();

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
fn localnet_supports_v3_message_endpoint() {
    let project = ProjectBuilder::new("localnet-v3-message-endpoint").build();
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project.localnet().args(["--accounts", "deployer"]).start();
    let message_boc = build_localnet_ext_in_boc();
    let (expected_hash, expected_hash_norm) = compute_message_hashes_base64(&message_boc);

    let response = node.post_json(
        "/api/v3/message",
        &json!({
            "boc": message_boc
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
    assert_eq!(message_hash, expected_hash);
    assert_eq!(message_hash_norm, expected_hash_norm);

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
fn localnet_uses_normalized_hash_for_send_boc_return_hash_and_v3_lookup() {
    let project = ProjectBuilder::new("localnet-send-boc-return-hash-norm").build();
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project.localnet().args(["--accounts", "deployer"]).start();
    let message_boc = build_localnet_ext_in_boc();
    let (expected_hash, expected_hash_norm) = compute_message_hashes_base64(&message_boc);

    let response = node.post_json(
        "/api/v2/sendBocReturnHash",
        &json!({
            "boc": message_boc
        }),
    );
    assert!(
        is_success_response(&response),
        "sendBocReturnHash failed: {}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );

    let payload = response_payload(&response);
    let message_hash = payload["hash"]
        .as_str()
        .expect("sendBocReturnHash hash must be a string");
    let message_hash_norm = payload["hash_norm"]
        .as_str()
        .expect("sendBocReturnHash hash_norm must be a string");

    assert_eq!(message_hash, expected_hash);
    assert_eq!(message_hash_norm, expected_hash_norm);

    let normalized_hash_query = encode_query_component(message_hash_norm);

    let traces = wait_for_non_empty_v3_traces_response(
        &node,
        &format!("/api/v3/traces?msg_hash={normalized_hash_query}"),
        Duration::from_secs(12),
    );
    let traces_payload = response_payload(&traces)["traces"]
        .as_array()
        .expect("v3 traces must contain a traces array");
    assert!(
        !traces_payload.is_empty(),
        "Expected at least one trace for normalized msg_hash:\n{}",
        serde_json::to_string_pretty(&traces).unwrap_or_default()
    );
    assert_eq!(
        traces_payload[0]["external_hash"].as_str(),
        Some(message_hash)
    );

    let by_msg_hash = wait_for_v3_transactions_response(
        &node,
        &format!(
            "/api/v3/transactionsByMessage?msg_hash={normalized_hash_query}&direction=in&limit=50"
        ),
        Duration::from_secs(12),
    );
    let matched = v3_transactions_from_response(&by_msg_hash);
    assert!(
        !matched.is_empty(),
        "Expected transactionsByMessage to match normalized msg_hash:\n{}",
        serde_json::to_string_pretty(&by_msg_hash).unwrap_or_default()
    );
    assert_eq!(
        matched[0]["in_msg"]["hash_norm"].as_str(),
        Some(message_hash_norm),
        "Expected normalized hash in matched inbound message:\n{}",
        serde_json::to_string_pretty(&by_msg_hash).unwrap_or_default()
    );

    node.stop();
}

#[test]
fn localnet_v3_traces_unknown_hashes_return_empty_traces() {
    let project = ProjectBuilder::new("localnet-v3-traces-unknown-hashes").build();
    let node = project.localnet().start();

    let unknown_msg_hash = encode_query_component(&Hash256([0x42; 32]).to_base64());
    let unknown_tx_hash = encode_query_component(&Hash256([0x43; 32]).to_base64());

    let (msg_status, msg_response) =
        node.get_json_with_status(&format!("/api/v3/traces?msg_hash={unknown_msg_hash}"));
    let (tx_status, tx_response) =
        node.get_json_with_status(&format!("/api/v3/traces?tx_hash={unknown_tx_hash}"));

    let summary = json!({
        "msg_hash": {
            "status": msg_status,
            "response": msg_response,
        },
        "tx_hash": {
            "status": tx_status,
            "response": tx_response,
        },
    });
    snapbox::assert_data_eq!(
        format!("{}\n", pretty_json_for_snapshot(&summary, project.path())),
        snapbox::file!("snapshots/localnet/test_localnet_v3_traces_unknown_hashes.response.json")
    );

    node.stop();
}

#[test]
fn localnet_send_boc_return_hash_waits_for_scheduled_block_before_transaction_appears() {
    let project = ProjectBuilder::new("localnet-send-boc-scheduled-mining").build();
    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .args(["--accounts", "deployer", "--block-interval-ms", "3000"])
        .start();
    let initial_seqno = latest_masterchain_seqno(&node);
    wait_for_masterchain_seqno_at_least(&node, initial_seqno + 1, Duration::from_secs(6));

    let message_boc = build_localnet_ext_in_boc();
    let (expected_hash, expected_hash_norm) = compute_message_hashes_base64(&message_boc);

    let response = node.post_json(
        "/api/v2/sendBocReturnHash",
        &json!({
            "boc": message_boc
        }),
    );
    let payload = response_payload(&response);
    let message_hash = payload["hash"]
        .as_str()
        .expect("sendBocReturnHash hash must be a string");
    let message_hash_norm = payload["hash_norm"]
        .as_str()
        .expect("sendBocReturnHash hash_norm must be a string");
    let normalized_hash_query = encode_query_component(message_hash_norm);
    let by_message_query = format!(
        "/api/v3/transactionsByMessage?msg_hash={normalized_hash_query}&direction=in&limit=50"
    );

    let before_tick = wait_for_ok_response(&node, &by_message_query, Duration::from_secs(2));
    let before_tick_transactions = v3_transactions_from_response(&before_tick);

    let after_tick =
        wait_for_v3_transactions_response(&node, &by_message_query, Duration::from_secs(8));
    let after_tick_transactions = v3_transactions_from_response(&after_tick);
    let matched_normalized_hash = after_tick_transactions
        .iter()
        .any(|tx| tx["in_msg"]["hash_norm"].as_str() == Some(message_hash_norm));

    let snapshot = json!({
        "accepted": {
            "hash_matches": message_hash == expected_hash,
            "hash_norm_matches": message_hash_norm == expected_hash_norm,
        },
        "before_next_tick": {
            "transaction_count": before_tick_transactions.len(),
        },
        "after_next_tick": {
            "has_transaction": !after_tick_transactions.is_empty(),
            "matched_normalized_hash": matched_normalized_hash,
        }
    });

    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_send_boc_waits_for_scheduled_block.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_supports_emulate_v1_emulate_trace() {
    let project = ProjectBuilder::new("localnet-emulate-v1-emulate-trace").build();
    let node = project.localnet().start();

    let before = wait_for_ok_response(&node, "/api/v2/getMasterchainInfo", Duration::from_secs(5));
    let seqno_before = before["result"]["last"]["seqno"]
        .as_i64()
        .expect("masterchain seqno must be integer before emulate");
    let transactions_before = wait_for_ok_response(
        &node,
        "/api/v3/transactions?limit=100",
        Duration::from_secs(5),
    );
    let tx_count_before = v3_transactions_from_response(&transactions_before).len();

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
    let response_seqno = response["mc_block_seqno"]
        .as_i64()
        .expect("emulateTrace response must include mc_block_seqno");
    assert!(
        response_seqno >= seqno_before,
        "Unexpected mc_block_seqno in emulateTrace response; expected at least {seqno_before}:\n{}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );

    let explicit_seqno =
        wait_for_masterchain_seqno_at_least(&node, response_seqno.max(1), Duration::from_secs(5));
    let response_with_seqno = node.post_json(
        "/api/emulate/v1/emulateTrace",
        &json!({
            "boc": V3_MESSAGE_TEST_BOC,
            "ignore_chksig": false,
            "mc_block_seqno": explicit_seqno
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
        Some(explicit_seqno),
        "Unexpected mc_block_seqno for explicit emulate request:\n{}",
        serde_json::to_string_pretty(&response_with_seqno).unwrap_or_default()
    );

    let transactions_after = wait_for_ok_response(
        &node,
        "/api/v3/transactions?limit=100",
        Duration::from_secs(5),
    );
    assert_eq!(
        v3_transactions_from_response(&transactions_after).len(),
        tx_count_before,
        "emulateTrace must not commit transactions. before:\n{}\nafter:\n{}",
        serde_json::to_string_pretty(&transactions_before).unwrap_or_default(),
        serde_json::to_string_pretty(&transactions_after).unwrap_or_default()
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
fn localnet_supports_v3_address_information_endpoint() {
    let project = ProjectBuilder::new("localnet-v3-address-information")
        .contract("getter", V3_GETTER_CONTRACT)
        .script_file("deploy_getter", V3_DEPLOY_GETTER_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
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
fn localnet_supports_v3_transactions_endpoints() {
    let project = ProjectBuilder::new("localnet-v3-transactions-endpoints").build();
    let node = project.localnet().start();

    for address in [
        V3_TRANSACTIONS_TEST_ACCOUNT_A,
        V3_TRANSACTIONS_TEST_ACCOUNT_B,
    ] {
        let faucet = node.post_json(
            "/acton_fundAccount",
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

    let all_txs_response = wait_for_v3_transactions_response(
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
fn localnet_batches_pending_faucet_messages_into_one_scheduled_block() {
    let project = ProjectBuilder::new("localnet-batch-faucet-scheduled-block").build();
    let node = project
        .localnet()
        .args(["--block-interval-ms", "3000"])
        .start();
    let initial_seqno = latest_masterchain_seqno(&node);
    wait_for_masterchain_seqno_at_least(&node, initial_seqno + 1, Duration::from_secs(6));

    let first_faucet = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": V3_TRANSACTIONS_TEST_ACCOUNT_A,
            "amount": 250_000_000u128
        }),
    );
    let second_faucet = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": V3_TRANSACTIONS_TEST_ACCOUNT_B,
            "amount": 250_000_000u128
        }),
    );

    let first_account_response = wait_for_v3_transactions_response(
        &node,
        &format!("/api/v3/transactions?account={V3_TRANSACTIONS_TEST_ACCOUNT_A}&limit=10"),
        Duration::from_secs(8),
    );
    let second_account_response = wait_for_v3_transactions_response(
        &node,
        &format!("/api/v3/transactions?account={V3_TRANSACTIONS_TEST_ACCOUNT_B}&limit=10"),
        Duration::from_secs(8),
    );

    let first_tx = &v3_transactions_from_response(&first_account_response)[0];
    let second_tx = &v3_transactions_from_response(&second_account_response)[0];
    let first_seqno = first_tx["mc_block_seqno"]
        .as_u64()
        .expect("first transaction mc_block_seqno must be integer");
    let second_seqno = second_tx["mc_block_seqno"]
        .as_u64()
        .expect("second transaction mc_block_seqno must be integer");
    let block = wait_for_ok_response(
        &node,
        &format!("/api/v2/getBlockTransactionsExt?seqno={first_seqno}"),
        Duration::from_secs(5),
    );
    let block_payload = response_payload(&block);
    let block_transactions = block_payload["transactions"]
        .as_array()
        .expect("getBlockTransactionsExt must return transactions array");

    let snapshot = json!({
        "faucet": {
            "first_ok": is_success_response(&first_faucet),
            "second_ok": is_success_response(&second_faucet),
        },
        "transactions": {
            "first_account": first_tx["account"].as_str(),
            "second_account": second_tx["account"].as_str(),
            "same_mc_block_seqno": first_seqno == second_seqno,
        },
        "block": {
            "req_count": block_payload["req_count"].as_u64(),
            "transaction_count": block_transactions.len(),
            "has_at_least_two_transactions": block_transactions.len() >= 2,
        }
    });

    assertion().eq(
        pretty_json_for_snapshot(&snapshot, project.path()),
        snapbox::file!(
            "snapshots/localnet/test_localnet_batches_faucet_messages_into_one_block.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_supports_v3_account_states_endpoint() {
    let project = ProjectBuilder::new("localnet-v3-account-states")
        .without_acton_toml()
        .file_from_path(
            "contracts/JettonMinter",
            "src/commands/new/templates/jetton/contracts/JettonMinter.tolk",
        )
        .file_from_path(
            "contracts/JettonWallet",
            "src/commands/new/templates/jetton/contracts/JettonWallet.tolk",
        )
        .file_from_path(
            "contracts/errors",
            "src/commands/new/templates/jetton/contracts/errors.tolk",
        )
        .file_from_path(
            "contracts/fees-management",
            "src/commands/new/templates/jetton/contracts/fees-management.tolk",
        )
        .file_from_path(
            "contracts/jetton-utils",
            "src/commands/new/templates/jetton/contracts/jetton-utils.tolk",
        )
        .file_from_path(
            "contracts/messages",
            "src/commands/new/templates/jetton/contracts/messages.tolk",
        )
        .file_from_path(
            "contracts/storage",
            "src/commands/new/templates/jetton/contracts/storage.tolk",
        )
        .file_from_path(
            "contracts/sharding",
            "src/commands/new/templates/jetton/contracts/sharding.tolk",
        )
        .file_from_path(
            "wrappers/JettonMinter.gen",
            "src/commands/new/templates/jetton/wrappers/JettonMinter.gen.tolk",
        )
        .file_from_path(
            "wrappers/JettonWallet.gen",
            "src/commands/new/templates/jetton/wrappers/JettonWallet.gen.tolk",
        )
        .file_from_path(
            "wrappers/utils",
            "src/commands/new/templates/jetton/wrappers/utils.tolk",
        )
        .file_from_path(
            "scripts/deploy",
            "src/commands/new/templates/jetton/scripts/deploy.tolk",
        )
        .file_from_path(
            "scripts/mint",
            "src/commands/new/templates/jetton/scripts/mint.tolk",
        )
        .file_from_path(
            "scripts/utils/common",
            "src/commands/new/templates/jetton/scripts/utils/common.tolk",
        )
        .build();
    project.acton().init().run().success();

    let acton_toml_path = project.path().join("Acton.toml");
    let acton_toml = fs::read_to_string(&acton_toml_path).unwrap();
    let acton_toml = acton_toml.replace(
        "[contracts.JettonMinter]\ndisplay-name = \"JettonMinter\"\nsrc = \"contracts/JettonMinter.tolk\"\ndepends = []",
        "[contracts.JettonMinter]\ndisplay-name = \"JettonMinter\"\nsrc = \"contracts/JettonMinter.tolk\"\ndepends = [\"JettonWallet\"]",
    );
    fs::write(&acton_toml_path, acton_toml).unwrap();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy.tolk")
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

    let minter_address = extract_prefixed_line_value(&script_stdout, "JETTON MINTER_ADDRESS=")
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned();
    let owner_address = extract_prefixed_line_value(&script_stdout, "JETTON_ADMIN OWNER_ADDRESS=");
    wait_until_address_state_active(&node, &minter_address, Duration::from_secs(12));

    let mint_result = project
        .acton()
        .script("scripts/mint.tolk")
        .verify_network("localnet")
        .env("JETTON_ADMIN", "deployer")
        .env("JETTON_MINTER_ADDRESS", &minter_address)
        .run();
    let mint_status = mint_result.output.get_output().status.code().unwrap_or(1);
    assert_eq!(mint_status, 0, "Mint script failed");

    let wallets_response = wait_for_ok_response(
        &node,
        &format!("/api/v3/jetton/wallets?owner_address={owner_address}&limit=10"),
        Duration::from_secs(12),
    );
    let wallets = response_payload(&wallets_response)["jetton_wallets"]
        .as_array()
        .expect("jetton_wallets must be an array");
    let jetton_wallet_address = wallets
        .first()
        .and_then(|wallet| wallet["address"].as_str())
        .expect("deployer must have a jetton wallet after mint")
        .to_owned();

    let missing_address = "0:1111111111111111111111111111111111111111111111111111111111111111";
    let response = wait_for_ok_response(
        &node,
        &format!(
            "/api/v3/accountStates?address={owner_address}&address={minter_address}&address={jetton_wallet_address}&address={missing_address}&include_boc=false"
        ),
        Duration::from_secs(12),
    );
    let payload = response_payload(&response);

    let accounts = payload["accounts"]
        .as_array()
        .expect("accountStates must return accounts array");
    assert_eq!(
        accounts.len(),
        4,
        "Expected one response row per requested address:\n{}",
        serde_json::to_string_pretty(payload).unwrap_or_default()
    );

    let owner_state = accounts
        .iter()
        .find(|account| {
            account["interfaces"].as_array().is_some_and(|interfaces| {
                interfaces
                    .iter()
                    .any(|value| value.as_str() == Some("wallet_v4r2"))
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "owner wallet account row with wallet_v4r2 interface missing:\n{}",
                serde_json::to_string_pretty(payload).unwrap_or_default()
            )
        });

    let minter_state = accounts
        .iter()
        .find(|account| {
            account["interfaces"].as_array().is_some_and(|interfaces| {
                interfaces
                    .iter()
                    .any(|value| value.as_str() == Some("jetton_master"))
            })
        })
        .expect("jetton master account row missing");
    assert!(
        minter_state.get("code_boc").is_none(),
        "code_boc must be omitted when include_boc=false:\n{}",
        serde_json::to_string_pretty(minter_state).unwrap_or_default()
    );
    assert!(
        minter_state.get("data_boc").is_none(),
        "data_boc must be omitted when include_boc=false:\n{}",
        serde_json::to_string_pretty(minter_state).unwrap_or_default()
    );

    let wallet_state = accounts
        .iter()
        .find(|account| {
            account["interfaces"].as_array().is_some_and(|interfaces| {
                interfaces
                    .iter()
                    .any(|value| value.as_str() == Some("jetton_wallet"))
            })
        })
        .expect("jetton wallet account row missing");
    assert!(
        wallet_state.get("code_boc").is_none(),
        "code_boc must be omitted when include_boc=false:\n{}",
        serde_json::to_string_pretty(wallet_state).unwrap_or_default()
    );
    assert!(
        wallet_state.get("data_boc").is_none(),
        "data_boc must be omitted when include_boc=false:\n{}",
        serde_json::to_string_pretty(wallet_state).unwrap_or_default()
    );

    let missing_state = accounts
        .iter()
        .find(|account| account["address"].as_str() == Some(missing_address))
        .expect("missing account row missing");
    assert_eq!(missing_state["status"].as_str(), Some("nonexist"));
    assert!(
        missing_state.get("code_hash").is_none(),
        "missing account must omit code_hash:\n{}",
        serde_json::to_string_pretty(missing_state).unwrap_or_default()
    );
    assert!(
        missing_state.get("data_hash").is_none(),
        "missing account must omit data_hash:\n{}",
        serde_json::to_string_pretty(missing_state).unwrap_or_default()
    );
    assert!(
        missing_state.get("frozen_hash").is_none(),
        "missing account must omit frozen_hash:\n{}",
        serde_json::to_string_pretty(missing_state).unwrap_or_default()
    );
    assert!(
        missing_state.get("code_boc").is_none(),
        "missing account must omit code_boc:\n{}",
        serde_json::to_string_pretty(missing_state).unwrap_or_default()
    );
    assert!(
        missing_state.get("data_boc").is_none(),
        "missing account must omit data_boc:\n{}",
        serde_json::to_string_pretty(missing_state).unwrap_or_default()
    );
    assert_eq!(
        missing_state["interfaces"].as_array().map(Vec::len),
        Some(0),
        "Missing account must not have interfaces:\n{}",
        serde_json::to_string_pretty(missing_state).unwrap_or_default()
    );

    let address_book = payload["address_book"]
        .as_object()
        .expect("accountStates must include address_book");
    let metadata = payload["metadata"]
        .as_object()
        .expect("accountStates must include metadata");

    let owner_row = address_book
        .get(
            owner_state["address"]
                .as_str()
                .expect("owner wallet row must expose canonical address"),
        )
        .expect("owner wallet address book row missing");
    assert!(
        owner_row["interfaces"]
            .as_array()
            .is_some_and(|interfaces| interfaces
                .iter()
                .any(|value| value.as_str() == Some("wallet_v4r2"))),
        "owner wallet address book row must expose wallet_v4r2 interface:\n{}",
        serde_json::to_string_pretty(owner_row).unwrap_or_default()
    );

    let minter_row = address_book
        .get(
            minter_state["address"]
                .as_str()
                .expect("minter row must expose canonical address"),
        )
        .expect("jetton master address book row missing");
    assert!(
        minter_row["interfaces"]
            .as_array()
            .is_some_and(|interfaces| interfaces
                .iter()
                .any(|value| value.as_str() == Some("jetton_master"))),
        "jetton master address book row must expose interfaces:\n{}",
        serde_json::to_string_pretty(minter_row).unwrap_or_default()
    );

    let minter_metadata = metadata
        .get(
            minter_state["address"]
                .as_str()
                .expect("minter row must expose canonical address"),
        )
        .expect("jetton master metadata missing");
    assert!(
        minter_metadata["token_info"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["type"].as_str() == Some("jetton_masters"))),
        "jetton master metadata must expose token_info.type:\n{}",
        serde_json::to_string_pretty(minter_metadata).unwrap_or_default()
    );

    let wallet_metadata = metadata
        .get(
            wallet_state["address"]
                .as_str()
                .expect("wallet row must expose canonical address"),
        )
        .expect("jetton wallet metadata missing");
    assert!(
        wallet_metadata["token_info"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["type"].as_str() == Some("jetton_wallets"))),
        "jetton wallet metadata must expose token_info.type:\n{}",
        serde_json::to_string_pretty(wallet_metadata).unwrap_or_default()
    );
    assert!(
        !metadata.contains_key(missing_address),
        "Missing account must not have metadata:\n{}",
        serde_json::to_string_pretty(metadata).unwrap_or_default()
    );

    node.stop();
}

#[test]
fn localnet_supports_v3_run_get_method() {
    let project = ProjectBuilder::new("localnet-v3-run-get-method")
        .contract("getter", V3_GETTER_CONTRACT)
        .script_file("deploy_getter", V3_DEPLOY_GETTER_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
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
fn localnet_run_get_method_on_missing_account_returns_vm_exit_code() {
    let project = ProjectBuilder::new("localnet-run-get-method-missing-account").build();
    let node = project.localnet().start();

    let (v3_status, v3_response) = node.post_json_with_status(
        "/api/v3/runGetMethod",
        &json!({
            "address": "EQDzA78rXj_YgEpIn43GrHcPgffSdYWiBFVZfAAD9SKdc7Vn",
            "method": "get_verification_record",
            "stack": []
        }),
    );
    let (v2_status, v2_response) = node.post_json_with_status(
        "/api/v2/runGetMethod",
        &json!({
            "address": "EQDzA78rXj_YgEpIn43GrHcPgffSdYWiBFVZfAAD9SKdc7Vn",
            "method": "get_verification_record",
            "stack": []
        }),
    );
    let v2_payload = response_payload(&v2_response);
    let (no_code_status, no_code_response) = node.post_json_with_status(
        "/api/v3/runGetMethod",
        &json!({
            "address": "0:5555555555555555555555555555555555555555555555555555555555555555",
            "method": "get_verification_record",
            "stack": []
        }),
    );

    let snapshot = json!({
        "v3": {
            "status": v3_status,
            "has_error": v3_response.get("error").is_some(),
            "gas_used": v3_response["gas_used"],
            "exit_code": v3_response["exit_code"],
            "stack": v3_response["stack"],
        },
        "v2": {
            "status": v2_status,
            "ok": v2_response["ok"],
            "gas_used": v2_payload["gas_used"],
            "exit_code": v2_payload["exit_code"],
            "stack": v2_payload["stack"],
            "last_transaction_id": v2_payload["last_transaction_id"],
        },
        "existing_no_code": {
            "status": no_code_status,
            "has_error": no_code_response.get("error").is_some(),
            "gas_used": no_code_response["gas_used"],
            "exit_code": no_code_response["exit_code"],
            "stack": no_code_response["stack"],
        },
    });

    let snapshot_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&snapshot).expect("snapshot JSON must serialize")
    );
    assertion().eq(
        snapshot_json,
        snapbox::file!(
            "snapshots/localnet/test_localnet_run_get_method_missing_account.summary.json"
        ),
    );

    node.stop();
}

#[test]
fn localnet_contracts_can_read_prevblocks_instructions() {
    let project = ProjectBuilder::new("localnet-prevblocks-get-method")
        .contract("getter", PREVBLOCKS_GETTER_CONTRACT)
        .script_file("deploy_getter", V3_DEPLOY_GETTER_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
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
    let query_seqno = latest_masterchain_seqno(&node) as u32;

    let prev_count =
        run_v3_get_num_at_seqno(&node, &getter_address, "prevMcBlocksCount", query_seqno);
    let latest_prev_seqno =
        run_v3_get_num_at_seqno(&node, &getter_address, "latestPrevMcSeqno", query_seqno);
    let prev_key_seqno =
        run_v3_get_num_at_seqno(&node, &getter_address, "prevKeySeqno", query_seqno);
    let sparse_first_seqno = run_v3_get_num_at_seqno(
        &node,
        &getter_address,
        "prevMcBlocks100FirstSeqno",
        query_seqno,
    );
    let expected_sparse_first_seqno = i64::from(query_seqno - (query_seqno % 100));

    let snapshot = json!({
        "prev_mc_blocks_count_positive": prev_count > 0,
        "latest_prev_mc_seqno_matches_query_seqno": latest_prev_seqno == i64::from(query_seqno),
        "prev_key_seqno_matches_latest_prev_mc_seqno": prev_key_seqno == latest_prev_seqno,
        "prev_mc_blocks_100_first_seqno_matches_expected": sparse_first_seqno == expected_sparse_first_seqno,
    });

    let snapshot_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&snapshot).expect("snapshot JSON must serialize")
    );
    assertion().eq(
        snapshot_json,
        snapbox::file!("snapshots/localnet/test_localnet_prevblocks_get_methods.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_registers_and_serves_compiler_abi_for_localnet_deploys() {
    let project = ProjectBuilder::new("localnet-compiler-abi-registry")
        .contract("getter", V3_GETTER_CONTRACT)
        .script_file("deploy_getter", V3_DEPLOY_GETTER_SCRIPT)
        .build();

    fs::write(project.path().join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("Failed to write wallets.toml");

    let node = project
        .localnet()
        .before_start(super::super::support::project::ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &node.base_url());

    let script_result = project
        .acton()
        .script("scripts/deploy_getter.tolk")
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

    let account_states = wait_for_ok_response(
        &node,
        &format!("/api/v3/accountStates?address={getter_address}&include_boc=false"),
        Duration::from_secs(12),
    );
    let code_hash_b64 = response_payload(&account_states)["accounts"]
        .as_array()
        .and_then(|accounts| accounts.first())
        .and_then(|account| account["code_hash"].as_str())
        .expect("accountStates must include code_hash for deployed getter");
    let code_hash_hex = Hash256::from_base64(code_hash_b64)
        .expect("accountStates code_hash must be valid base64")
        .to_hex();

    let missing_code_hash = "1111111111111111111111111111111111111111111111111111111111111111";
    let abi_response = wait_for_ok_response(
        &node,
        &format!(
            "/acton_getCompilerAbi?code_hash={}&code_hash={CATALOG_WALLET_V4R2_CODE_HASH}&code_hash={missing_code_hash}",
            encode_query_component(&code_hash_hex)
        ),
        Duration::from_secs(12),
    );
    let abi_payload = response_payload(&abi_response);
    let abi = &abi_payload[&code_hash_hex]["compiler_abi"];
    let catalog_abi = &abi_payload[CATALOG_WALLET_V4R2_CODE_HASH]["compiler_abi"];

    let register_override_response = node.post_json(
        "/acton_registerCompilerAbis",
        &json!({
            "entries": [
                {
                    "code_hash": CATALOG_WALLET_V4R2_CODE_HASH,
                    "compiler_abi": {
                        "compiler_name": "tolk",
                        "contract_name": "LocalOverride"
                    }
                }
            ]
        }),
    );
    assert_eq!(
        register_override_response["ok"].as_bool(),
        Some(true),
        "registerCompilerAbis failed: {}",
        serde_json::to_string_pretty(&register_override_response).unwrap_or_default()
    );
    let override_response = wait_for_ok_response(
        &node,
        &format!("/acton_getCompilerAbi?code_hash={CATALOG_WALLET_V4R2_CODE_HASH}"),
        Duration::from_secs(12),
    );
    let override_payload = response_payload(&override_response);

    let abi_summary = json!({
        "compiler_name": abi["compiler_name"],
        "contract_name": abi["contract_name"],
        "has_add_ten_get_method": abi["get_methods"].as_array().is_some_and(|methods| {
            methods
                .iter()
                .any(|method| method["name"].as_str() == Some("addTen"))
        }),
        "catalog_contract_name": catalog_abi["contract_name"],
        "catalog_has_seqno_get_method": catalog_abi["get_methods"].as_array().is_some_and(|methods| {
            methods
                .iter()
                .any(|method| method["name"].as_str() == Some("seqno"))
        }),
        "local_registration_overrides_catalog": override_payload[CATALOG_WALLET_V4R2_CODE_HASH]
            ["compiler_abi"]
            ["contract_name"]
            .as_str()
            == Some("LocalOverride"),
        "missing_code_hash_is_null": abi_payload[missing_code_hash].is_null(),
    });
    let abi_summary_json = format!(
        "{}\n",
        serde_json::to_string_pretty(&abi_summary)
            .expect("Failed to serialize compiler ABI batch summary")
    );

    assertion().eq(
        abi_summary_json,
        snapbox::file!("snapshots/localnet/test_localnet_compiler_abi_batch.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_supports_utils_detect_and_pack_endpoints() {
    let project = ProjectBuilder::new("localnet-utils-endpoints").build();
    let node = project.localnet().start();

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
    append_custom_localnet_network(project_path, "localnet", base_url);
}

fn append_custom_localnet_network(project_path: &Path, network_name: &str, base_url: &str) {
    use std::fmt::Write as _;

    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("Failed to read generated Acton.toml");
    let _ = write!(
        acton_toml,
        r#"

[networks.{network_name}]
api = {{ v2 = "{base_url}/api/v2", v3 = "{base_url}/api/v3" }}
"#
    );
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

fn extract_prefixed_line_value(output: &str, prefix: &str) -> String {
    let cleaned = strip_ansi(output);
    cleaned
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(prefix).map(ToOwned::to_owned))
        .unwrap_or_else(|| panic!("Line starting with `{prefix}` not found in output:\n{cleaned}"))
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

fn run_v3_get_num_at_seqno(
    node: &crate::support::localnet::LocalnetHandle,
    address: &str,
    method: &str,
    seqno: u32,
) -> i64 {
    let response = node.post_json(
        "/api/v3/runGetMethod",
        &json!({
            "address": address,
            "method": method,
            "seqno": seqno,
            "stack": [],
        }),
    );

    assert!(
        is_success_response(&response),
        "v3 runGetMethod {method} failed: {}",
        serde_json::to_string_pretty(&response).unwrap_or_default()
    );

    let payload = response_payload(&response);
    assert_eq!(payload["exit_code"].as_i64(), Some(0));
    assert_eq!(payload["stack"][0]["type"].as_str(), Some("num"));
    payload["stack"][0]["value"]
        .as_str()
        .expect("numeric get-method result must be a string")
        .parse::<i64>()
        .expect("numeric get-method result must parse")
}

fn pretty_json_for_snapshot(value: &Value, project_path: &Path) -> String {
    let response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("Failed to serialize JSON snapshot")
    );
    normalize_output_preserve_escapes(&response_json, project_path)
}

fn get_json_with_status(client: &Client, url: &str) -> (u16, Value) {
    let response = client
        .get(url)
        .send()
        .unwrap_or_else(|error| panic!("Failed GET {url}: {error}"));
    parse_json_response(response, "GET", url)
}

fn post_json_with_status(client: &Client, url: &str, payload: &Value) -> (u16, Value) {
    let response = client
        .post(url)
        .json(payload)
        .send()
        .unwrap_or_else(|error| panic!("Failed POST {url}: {error}"));
    parse_json_response(response, "POST", url)
}

fn parse_json_response(
    response: reqwest::blocking::Response,
    method: &str,
    url: &str,
) -> (u16, Value) {
    let status = response.status().as_u16();
    let body = response
        .text()
        .unwrap_or_else(|error| panic!("Failed to read {method} {url} response body: {error}"));
    let json = serde_json::from_str(&body)
        .unwrap_or_else(|error| panic!("{method} {url} returned invalid JSON: {error}\n{body}"));
    (status, json)
}

fn summarize_auth_error((status, mut response): (u16, Value)) -> Value {
    normalize_extra_for_snapshot(&mut response);
    json!({
        "status": status,
        "ok": response["ok"],
        "code": response["code"],
        "error": response["error"],
    })
}

fn normalize_localnet_status_json(payload: &mut Value, expected_port: u16) {
    match payload.get_mut("port").and_then(|value| value.as_u64()) {
        Some(port) if port == u64::from(expected_port) => {
            payload["port"] = json!("[PORT]");
        }
        _ => panic!(
            "Expected localnet status port {expected_port}, got:\n{}",
            serde_json::to_string_pretty(payload).unwrap_or_default()
        ),
    }

    match payload.get_mut("uptime_seconds") {
        Some(value) if value.as_u64().is_some() => {
            *value = json!("[UPTIME_SECONDS]");
        }
        Some(value) if value.is_null() => {}
        _ => panic!(
            "Expected localnet status uptime_seconds to be a number or null, got:\n{}",
            serde_json::to_string_pretty(payload).unwrap_or_default()
        ),
    }

    match payload.get_mut("last_block_seqno") {
        Some(value) if value.as_u64().is_some() => {
            *value = json!("[LAST_BLOCK_SEQNO]");
        }
        Some(value) if value.is_null() => {}
        _ => panic!(
            "Expected localnet status last_block_seqno to be a number or null, got:\n{}",
            serde_json::to_string_pretty(payload).unwrap_or_default()
        ),
    }
}

fn normalize_localnet_status_stdout(stdout: &str, port: u16) -> String {
    let port_fragment = format!("127.0.0.1:{port}");
    let normalized = stdout.replace(&port_fragment, "127.0.0.1:[PORT]");
    let mut lines = normalized
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("Uptime: ") {
                let indent = &line[..line.len() - trimmed.len()];
                format!("{indent}Uptime: [UPTIME_SECONDS]s")
            } else if trimmed.starts_with("Last block seqno: ") {
                let indent = &line[..line.len() - trimmed.len()];
                format!("{indent}Last block seqno: [LAST_BLOCK_SEQNO]")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.ends_with('\n') {
        lines.push('\n');
    }
    lines
}

fn summarize_admin_response(response: &Value) -> Value {
    let mut response = response.clone();
    normalize_extra_for_snapshot(&mut response);
    if let Some(tx_hash) = response.pointer_mut("/result/result/tx_hash") {
        *tx_hash = json!("[HASH]");
    }
    if let Some(hash) = response.pointer_mut("/result/hash") {
        *hash = json!("[HASH]");
    }
    response
}

fn wait_for_ok_response(
    node: &crate::support::localnet::LocalnetHandle,
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

fn wait_for_non_empty_v3_traces_response(
    node: &crate::support::localnet::LocalnetHandle,
    query: &str,
    timeout: Duration,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        let (status, response) = node.get_json_with_status(query);
        if (200..300).contains(&status)
            && is_success_response(&response)
            && response_payload(&response)["traces"]
                .as_array()
                .is_some_and(|traces| !traces.is_empty())
        {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for non-empty traces response from `{query}`; last status={status}:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn wait_for_v3_transactions_response(
    node: &crate::support::localnet::LocalnetHandle,
    query: &str,
    timeout: Duration,
) -> Value {
    let deadline = Instant::now() + timeout;
    loop {
        let (status, response) = node.get_json_with_status(query);
        if (200..300).contains(&status)
            && is_success_response(&response)
            && !v3_transactions_from_response(&response).is_empty()
        {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for non-empty v3 transactions from `{query}`; last status={status}:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(200));
    }
}

fn latest_masterchain_seqno(node: &crate::support::localnet::LocalnetHandle) -> i64 {
    let response = wait_for_ok_response(node, "/api/v2/getMasterchainInfo", Duration::from_secs(5));
    response["result"]["last"]["seqno"]
        .as_i64()
        .expect("masterchain seqno must be integer")
}

fn wait_for_masterchain_seqno_at_least(
    node: &crate::support::localnet::LocalnetHandle,
    min_seqno: i64,
    timeout: Duration,
) -> i64 {
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json("/api/v2/getMasterchainInfo");
        if let Some(seqno) = response["result"]["last"]["seqno"].as_i64()
            && seqno >= min_seqno
        {
            return seqno;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for masterchain seqno >= {min_seqno}; last response:\n{}",
            serde_json::to_string_pretty(&response).unwrap_or_default()
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_until_address_state_active(
    node: &crate::support::localnet::LocalnetHandle,
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

fn wait_for_address_balance_at_least(
    node: &crate::support::localnet::LocalnetHandle,
    address: &str,
    expected_balance: u128,
    timeout: Duration,
) -> Value {
    let query = format!("/api/v2/getAddressInformation?address={address}");
    let deadline = Instant::now() + timeout;
    loop {
        let response = node.get_json(&query);
        if is_success_response(&response) && parse_address_balance(&response) >= expected_balance {
            return response;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for address `{address}` balance to reach {expected_balance}:\n{}",
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

fn unpack_address(node: &crate::support::localnet::LocalnetHandle, address: &str) -> String {
    let response = wait_for_ok_response(
        node,
        &format!("/api/v2/unpackAddress?address={address}"),
        Duration::from_secs(12),
    );
    response["result"].as_str().map_or_else(
        || {
            panic!(
                "Expected string result from unpackAddress for `{address}`:\n{}",
                serde_json::to_string_pretty(&response).unwrap_or_default()
            )
        },
        ToOwned::to_owned,
    )
}

fn shard_account_cell_boc64(response: &Value) -> &str {
    response["result"]["bytes"].as_str().unwrap_or_else(|| {
        panic!(
            "getShardAccountCell result must contain cell bytes:\n{}",
            serde_json::to_string_pretty(response).unwrap_or_default()
        )
    })
}

fn decode_shard_account_cell_response(response: &Value) -> ShardAccount {
    Boc::decode_base64(shard_account_cell_boc64(response))
        .expect("getShardAccountCell bytes must be a valid BoC")
        .parse::<ShardAccount>()
        .expect("getShardAccountCell bytes must decode as ShardAccount")
}

fn summarize_shard_account_cell_response(
    response: &Value,
    expected_raw_address: Option<&str>,
) -> Value {
    let shard_account = decode_shard_account_cell_response(response);
    let last_trans_lt = if shard_account.last_trans_lt == 0 {
        "zero"
    } else {
        "nonzero"
    };
    let last_trans_hash_nonzero = shard_account.last_trans_hash != HashBytes::ZERO;

    let optional_account = shard_account
        .account
        .load()
        .expect("ShardAccount account reference must load");
    let Some(account) = optional_account.0 else {
        return json!({
            "ok": response["ok"],
            "cell_type": response["result"]["@type"],
            "state": "nonexist",
            "last_trans_lt": last_trans_lt,
            "last_trans_hash_nonzero": last_trans_hash_nonzero,
        });
    };

    let address_matches = expected_raw_address.map(|expected| match &account.address {
        IntAddr::Std(std) => {
            format!("{}:{}", std.workchain, hex::encode(std.address.0)) == expected
        }
        IntAddr::Var(_) => false,
    });
    let balance: u128 = account.balance.tokens.into();
    let (state, code_present, data_present, frozen_hash_present) = match account.state {
        AccountState::Active(state_init) => (
            "active",
            state_init.code.is_some(),
            state_init.data.is_some(),
            false,
        ),
        AccountState::Uninit => ("uninitialized", false, false, false),
        AccountState::Frozen(hash) => ("frozen", false, false, hash != HashBytes::ZERO),
    };

    json!({
        "ok": response["ok"],
        "cell_type": response["result"]["@type"],
        "state": state,
        "balance_positive": balance > 0,
        "address_matches": address_matches,
        "code_present": code_present,
        "data_present": data_present,
        "frozen_hash_present": frozen_hash_present,
        "last_trans_lt": last_trans_lt,
        "last_trans_hash_nonzero": last_trans_hash_nonzero,
    })
}

fn v3_transactions_from_response(response: &Value) -> &[Value] {
    response_payload(response)
        .get("transactions")
        .and_then(Value::as_array)
        .map_or_else(
            || {
                panic!(
                    "Expected `transactions` array in response payload:\n{}",
                    serde_json::to_string_pretty(response).unwrap_or_default()
                )
            },
            Vec::as_slice,
        )
}

fn hashes_equivalent(left: &str, right: &str) -> bool {
    normalize_hash_to_bytes(left) == normalize_hash_to_bytes(right)
}

fn build_localnet_ext_in_boc() -> String {
    let mnemonic = Mnemonic::from_str(DEPLOYER_MNEMONIC, None).expect("invalid deployer mnemonic");
    let key_pair = mnemonic
        .to_key_pair()
        .expect("deployer mnemonic to keypair failed");
    let version = WalletVersion::V4R2;
    let wallet_id = wallets::wallet_id(version, &Network::Localnet);
    let wallet =
        TonWallet::new_with_params(version, key_pair, 0, wallet_id).expect("wallet must build");

    let wallet_addr = ton_address_to_std_addr(&wallet.address);
    let internal_boc = build_internal_message_boc(wallet_addr.clone(), wallet_addr, 50_000_000);
    let internal_cell = TonCell::from_boc(internal_boc).expect("must decode internal TonCell");
    let expire_at = (SystemTime::now() + Duration::from_secs(600))
        .duration_since(UNIX_EPOCH)
        .expect("current time must be after unix epoch")
        .as_secs() as u32;
    wallet
        .create_ext_in_msg(vec![internal_cell], 1, expire_at, false)
        .expect("must build external-in message")
        .to_boc_base64()
        .expect("must encode external-in message boc")
}

fn build_localnet_internal_boc() -> String {
    let source = test_std_addr(0x11);
    let target = test_std_addr(0x22);
    base64::engine::general_purpose::STANDARD
        .encode(build_internal_message_boc(source, target, 50_000_000))
}

fn build_internal_message_boc(source: StdAddr, target: StdAddr, value: u128) -> Vec<u8> {
    let message = OwnedMessage {
        info: MsgInfo::Int(IntMsgInfo {
            ihr_disabled: true,
            bounce: false,
            bounced: false,
            src: IntAddr::Std(source),
            dst: IntAddr::Std(target),
            value: CurrencyCollection::new(value),
            ihr_fee: Default::default(),
            fwd_fee: Default::default(),
            created_at: 0,
            created_lt: 0,
        }),
        init: None,
        body: CellSliceParts::from(CellBuilder::new().build().expect("must build empty body")),
        layout: None,
    };

    BocRepr::encode(message).expect("must encode internal message boc")
}

fn test_std_addr(byte: u8) -> StdAddr {
    StdAddr {
        anycast: None,
        address: HashBytes([byte; 32]),
        workchain: 0,
    }
}

fn compute_message_hashes_base64(boc_b64: &str) -> (String, String) {
    let boc = base64::engine::general_purpose::STANDARD
        .decode(boc_b64)
        .expect("message boc must be valid base64");
    let cell = Boc::decode(&boc).expect("message boc must decode");
    let message = cell
        .parse::<Message<'_>>()
        .expect("message boc must parse as Message");

    (
        Hash256(*cell.repr_hash().as_array()).to_base64(),
        compute_normalized_ext_in_hash_for_test(&message).to_base64(),
    )
}

fn ton_address_to_std_addr(address: &TonAddress) -> StdAddr {
    StdAddr {
        anycast: None,
        address: HashBytes(
            <[u8; 32]>::try_from(address.hash.as_slice())
                .expect("TonAddress hash must be exactly 32 bytes"),
        ),
        workchain: address.workchain as i8,
    }
}

fn compute_normalized_ext_in_hash_for_test(msg: &Message<'_>) -> Hash256 {
    let MsgInfo::ExtIn(info) = &msg.info else {
        panic!("TEP-467 normalization only applies to external-in messages");
    };

    let mut body_builder = CellBuilder::new();
    body_builder
        .store_slice(msg.body)
        .expect("must store message body");
    let body_cell = body_builder.build().expect("must build message body cell");

    let normalized_info = ExtInMsgInfo {
        src: None,
        dst: info.dst.clone(),
        import_fee: Tokens::ZERO,
    };

    let mut builder = CellBuilder::new();
    builder
        .store_small_uint(0b10, 2)
        .expect("must store ext-in tag");
    normalized_info
        .store_into(&mut builder, Cell::empty_context())
        .expect("must store normalized ext-in info");
    builder.store_bit_zero().expect("must clear init");
    builder.store_bit_one().expect("must store body as ref");
    builder
        .store_reference(body_cell)
        .expect("must store normalized body ref");
    Hash256(
        *builder
            .build()
            .expect("must build normalized message")
            .repr_hash()
            .as_array(),
    )
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
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn contains_tx_hash(transactions: &[Value], hash: &str) -> bool {
    transactions
        .iter()
        .any(|tx| tx["hash"].as_str() == Some(hash))
}

fn find_trace_node<'a>(node: &'a Value, tx_hash: &str) -> Option<&'a Value> {
    if node.get("tx_hash").and_then(Value::as_str) == Some(tx_hash) {
        return Some(node);
    }

    node.get("children")
        .and_then(Value::as_array)?
        .iter()
        .find_map(|child| find_trace_node(child, tx_hash))
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
    normalize_extra_for_snapshot(response);
    redact_dynamic_transaction_fields(response);
}

fn normalize_out_msg_queue_size_for_snapshot(response: &mut Value) {
    normalize_extra_for_snapshot(response);

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

fn normalize_api_calls_for_snapshot(response: &mut Value) {
    normalize_extra_for_snapshot(response);

    if let Some(calls) = response
        .pointer_mut("/result/calls")
        .and_then(Value::as_array_mut)
    {
        for call in calls {
            if let Some(timestamp_ms) = call.get_mut("timestamp_ms") {
                *timestamp_ms = json!("[TIMESTAMP_MS]");
            }
            if let Some(duration_ms) = call.get_mut("duration_ms") {
                *duration_ms = json!("[DURATION_MS]");
            }
            if let Some(duration_ns) = call.get_mut("duration_ns") {
                *duration_ns = json!("[DURATION_NS]");
            }
        }
    }
}

fn normalize_extra_for_snapshot(response: &mut Value) {
    if let Some(extra) = response.get_mut("@extra") {
        *extra = json!("[EXTRA]");
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
