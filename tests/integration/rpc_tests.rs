use crate::common::{assertion, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::{ActonCommand, Project, ProjectBuilder};
use crate::support::toncenter::{
    CapturedToncenterRequest, ToncenterV2MockResponse, append_custom_network,
    append_custom_network_with_urls, append_localnet_network,
    spawn_toncenter_v2_mock_with_capture as spawn_toncenter_v2_mock,
    toncenter_v2_account_info_with_code_ok_response as toncenter_v2_account_info_ok_response,
    toncenter_v2_masterchain_info_ok_response,
};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::models::message::IntAddr;

const RAW_INFO_ADDRESS: &str = "0:1111111111111111111111111111111111111111111111111111111111111111";
const MATCHED_INFO_ADDRESS: &str =
    "0:2222222222222222222222222222222222222222222222222222222222222222";
const MATCHED_INFO_OWNER_ADDRESS: &str =
    "0:3333333333333333333333333333333333333333333333333333333333333333";
const TRACE_ROOT_HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TRACE_CHILD_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const TRACE_RETURN_HASH: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const TRACE_COUNTER_TYPES: &str = r"
enum Errors {
    NotOwner = 100
    CounterUnderflow = 0x1001
    InvalidMessage = 0xFFFF
}

struct Storage {
    id: uint32
    owner: address
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

struct (0x283b4c3f) DecreaseCounter {
    decreaseBy: uint32
}

struct (0x3a752f06) ResetCounter {}

struct (0xd53276db) ReturnExcessesBack {
    queryId: uint64
}
";
const TRACE_COUNTER_CONTRACT: &str = r#"
import "types"

contract Counter {
    storage: Storage
    incomingMessages: AllowedMessage
}

type AllowedMessage =
    | IncreaseCounter
    | DecreaseCounter
    | ResetCounter

fun onInternalMessage(in: InMessage) {
    val msg = lazy AllowedMessage.fromSlice(in.body);

    match (msg) {
        IncreaseCounter => {
            var storage = lazy Storage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;

            storage.counter += msg.increaseBy;
            storage.save();

            val _sampleExcessesMsg = createMessage({
                bounce: BounceMode.NoBounce,
                dest: in.senderAddress,
                value: 0,
                body: ReturnExcessesBack { queryId: 7 },
            });
        }
        DecreaseCounter => {
            var storage = lazy Storage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;
            assert (storage.counter >= msg.decreaseBy) throw Errors.CounterUnderflow;

            storage.counter -= msg.decreaseBy;
            storage.save();
        }
        ResetCounter => {
            var storage = lazy Storage.load();
            assert (storage.owner == in.senderAddress) throw Errors.NotOwner;

            storage.counter = 0;
            storage.save();
        }
        else => {
            assert (in.body.isEmpty()) throw Errors.InvalidMessage;
        }
    }
}

fun onBouncedMessage(_in: InMessageBounced) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();

    return storage.counter;
}

get fun owner(): address {
    val storage = lazy Storage.load();

    return storage.owner;
}
"#;
const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
"#;
const PRINT_DEPLOYER_ADDRESS_SCRIPT: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");
    println("DEPLOYER_ADDRESS={}", wallet.address);
}
"#;
const DEPLOY_COUNTER_SCRIPT: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"

fun main() {
    val wallet = scripts.wallet("deployer");
    val counterData = beginCell()
        .storeUint(7, 32)
        .storeAddress(wallet.address)
        .storeUint(42, 32)
        .endCell();

    val counterInit = ContractState {
        code: build("counter"),
        data: counterData,
    };
    val counterAddress = AutoDeployAddress {
        stateInit: counterInit,
    }.calculateAddress();

    val deployCounter = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: {
            stateInit: counterInit,
        },
    });
    net.send(wallet.address, deployCounter);

    println("COUNTER_ADDRESS={}", counterAddress);
}
"#;
const PRECOMPILED_COUNTER_TYPES: &str = r"
struct Storage {
    id: uint32
    owner: address
    counter: uint32
}

contract Precompiled {
    storage: Storage
}
";
#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_info_prints_remote_account_without_local_abi_match() {
    let project = ProjectBuilder::new("rpc-info-raw").build();
    let log_dir = prepare_log_dir(project.path());
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            777_000_000,
            &test_cell_boc64(0xdead_beef),
            &test_cell_boc64(0x1234_5678),
            "active",
            "",
            "17",
            "deadbeef",
        )]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(RAW_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();

    output
        .success()
        .assert_snapshot_matches("integration/snapshots/rpc/test_rpc_info_raw.stdout.txt");

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_eq!(captured[0].method, "GET");
    assert!(
        captured[0]
            .path
            .starts_with("/api/v2/getAddressInformation?address=0%3A1111111111111111111111111111111111111111111111111111111111111111"),
        "unexpected request path: {}",
        captured[0].path
    );
    assert_eq!(
        header_value(&captured[0].headers, "X-API-Key"),
        Some("custom-mock-api-key"),
        "rpc info should send TonCenter API keys for custom networks from MOCK_API_KEY",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_info_forwards_block_number_to_account_info() {
    let project = ProjectBuilder::new("rpc-info-block-number").build();
    let log_dir = prepare_log_dir(project.path());
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            777_000_000,
            &test_cell_boc64(0xdead_beef),
            &test_cell_boc64(0x1234_5678),
            "active",
            "",
            "17",
            "deadbeef",
        )]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(RAW_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .arg("--block-number")
        .arg("123456")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/rpc/test_rpc_info_block_number.stdout.txt");

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_request_snapshot(
        &captured[0],
        "integration/snapshots/rpc/test_rpc_info_block_number.request.txt",
    );
}

#[test]
fn test_rpc_info_decodes_storage_when_local_code_hash_matches() {
    let project = ProjectBuilder::new("rpc-info-storage-decode")
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .build()
        .contract("counter")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();

    let artifact_path = project.path().join("build/counter.json");
    let artifact = fs::read_to_string(&artifact_path).expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");

    let (mock_url, mock_handle, _) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        )]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_decodes_storage.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_info_decodes_storage_for_boc_contract_with_types() {
    let source_project = ProjectBuilder::new("rpc-info-boc-types-source")
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .build();
    source_project
        .acton()
        .build()
        .contract("counter")
        .run()
        .success();

    let artifact_path = source_project.path().join("build/counter.json");
    let artifact = fs::read_to_string(&artifact_path).expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");
    let code = Boc::decode_base64(code_boc64).expect("code BoC base64 must decode");

    let project = ProjectBuilder::new("rpc-info-boc-types")
        .contract_from_boc_with_types(
            "precompiled",
            Boc::encode(code),
            "contracts/precompiled.types.tolk",
        )
        .raw_file(
            "contracts/precompiled.types.tolk",
            PRECOMPILED_COUNTER_TYPES,
        )
        .build();
    let log_dir = prepare_log_dir(project.path());

    let (mock_url, mock_handle, _) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        )]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_decodes_storage_for_boc_contract_with_types.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_info_skips_broken_contract_candidates_and_matches_later_contract() {
    let project = ProjectBuilder::new("rpc-info-skips-broken-candidate")
        .contract_from_boc("a_bad", vec![0x01, 0x02, 0x03])
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .build()
        .contract("counter")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();

    let artifact_path = project.path().join("build/counter.json");
    let artifact = fs::read_to_string(&artifact_path).expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");

    let (mock_url, mock_handle, _) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        )]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_decodes_storage.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_info_surfaces_malformed_manifest_errors() {
    let project = ProjectBuilder::new("rpc-info-malformed-manifest").build();
    let log_dir = prepare_log_dir(project.path());
    fs::write(
        project.path().join("Acton.toml"),
        "[package\nname = \"broken\"\n",
    )
    .expect("failed to write malformed Acton.toml");

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(RAW_INFO_ADDRESS)
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_contains("Failed to load Acton config")
        .assert_stderr_contains("TOML parse error");
}

#[test]
fn test_rpc_info_rejects_invalid_address() {
    let project = ProjectBuilder::new("rpc-info-invalid-address").build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg("not-an-address")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_info_invalid_address.stderr.txt",
        );
}

#[test]
fn test_rpc_info_reads_wallet_account_from_localnet() {
    let project = ProjectBuilder::new("rpc-info-localnet-wallet")
        .script_file("print_deployer_address", PRINT_DEPLOYER_ADDRESS_SCRIPT)
        .build();
    write_deployer_wallets(project.path());

    let node = start_localnet_with_localnet(&project);
    let log_dir = prepare_log_dir(project.path());

    let script_output = project
        .acton()
        .script("scripts/print_deployer_address.tolk")
        .verify_network("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();
    let script_stdout = stdout(&script_output);
    script_output.success();

    let deployer_address = extract_marker_value(&script_stdout, "DEPLOYER_ADDRESS=");

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(&deployer_address)
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();
    assert_localnet_rpc_snapshot(
        &output,
        "integration/snapshots/rpc/test_rpc_info_localnet_wallet.stdout.txt",
    );

    node.stop();
}

#[test]
fn test_rpc_info_decodes_storage_from_localnet() {
    let project = ProjectBuilder::new("rpc-info-localnet-storage")
        .file_from_path(
            "contracts/types",
            "src/commands/new/templates/counter/contracts/types.tolk",
        )
        .contract_from_path(
            "counter",
            "src/commands/new/templates/counter/contracts/Counter.tolk",
        )
        .script_file("deploy_counter", DEPLOY_COUNTER_SCRIPT)
        .build();
    write_deployer_wallets(project.path());

    let node = project
        .localnet()
        .before_start(ActonCommand::build)
        .args(["--accounts", "deployer"])
        .start();
    append_localnet_network(project.path(), &format!("{}/api/v2", node.base_url()));
    let log_dir = prepare_log_dir(project.path());

    let deploy_output = project
        .acton()
        .script("scripts/deploy_counter.tolk")
        .verify_network("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();
    let deploy_stdout = stdout(&deploy_output);
    deploy_output.success();

    let counter_address = extract_marker_value(&deploy_stdout, "COUNTER_ADDRESS=");
    node.wait_until_address_state_active(&counter_address, Duration::from_secs(12));

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("info")
        .arg(&counter_address)
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();
    assert_localnet_rpc_snapshot(
        &output,
        "integration/snapshots/rpc/test_rpc_info_localnet_storage.stdout.txt",
    );

    node.stop();
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_block_prints_full_toncenter_masterchain_info() {
    let project = ProjectBuilder::new("rpc-block-custom-network").build();
    let log_dir = prepare_log_dir(project.path());
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_masterchain_info_ok_response(123_456)]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("block")
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_block_custom_network.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_eq!(captured[0].method, "GET");
    assert_eq!(
        captured[0].path, "/api/v2/getMasterchainInfo",
        "unexpected request path"
    );
    assert_eq!(
        header_value(&captured[0].headers, "X-API-Key"),
        Some("custom-mock-api-key"),
        "rpc block should send TonCenter API keys for custom networks from MOCK_API_KEY",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_block_number_uses_custom_network_and_api_key() {
    let project = ProjectBuilder::new("rpc-block-number-custom-network").build();
    let log_dir = prepare_log_dir(project.path());
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v2_masterchain_info_ok_response(123_456)]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("block-number")
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_block_number_custom_network.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_eq!(captured[0].method, "GET");
    assert_eq!(
        captured[0].path, "/api/v2/getMasterchainInfo",
        "unexpected request path"
    );
    assert_eq!(
        header_value(&captured[0].headers, "X-API-Key"),
        Some("custom-mock-api-key"),
        "rpc block-number should send TonCenter API keys for custom networks from MOCK_API_KEY",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_trace_uses_v3_traces_and_formatter_context() {
    let project = ProjectBuilder::new("rpc-trace-v3-tree")
        .file("contracts/types", TRACE_COUNTER_TYPES)
        .contract("counter", TRACE_COUNTER_CONTRACT)
        .build();
    let log_dir = prepare_log_dir(project.path());

    project
        .acton()
        .build()
        .contract("counter")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success();

    let artifact_path = project.path().join("build/counter.json");
    let artifact = fs::read_to_string(&artifact_path).expect("build artifact must exist");
    let artifact: JsonValue =
        serde_json::from_str(&artifact).expect("build artifact must be valid json");
    let code_boc64 = artifact["code_boc64"]
        .as_str()
        .expect("build artifact must contain code_boc64");

    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock(vec![
        toncenter_v3_trace_ok_response(
            MATCHED_INFO_ADDRESS,
            MATCHED_INFO_OWNER_ADDRESS,
            &counter_increase_body_boc64(5),
        ),
        toncenter_v3_account_states_ok_response(
            MATCHED_INFO_ADDRESS,
            MATCHED_INFO_OWNER_ADDRESS,
            code_boc64,
        ),
        toncenter_v3_trace_ok_response(
            MATCHED_INFO_ADDRESS,
            MATCHED_INFO_OWNER_ADDRESS,
            &counter_increase_body_boc64(5),
        ),
        toncenter_v3_account_states_ok_response(
            MATCHED_INFO_ADDRESS,
            MATCHED_INFO_OWNER_ADDRESS,
            code_boc64,
        ),
    ]);
    append_custom_network_with_urls(
        project.path(),
        "mock",
        &format!("{mock_url}/api/v2"),
        &format!("{mock_url}/api/v3"),
    );

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("--color")
        .arg("never")
        .arg("rpc")
        .arg("trace")
        .arg(TRACE_ROOT_HASH)
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();

    output
        .success()
        .assert_snapshot_matches("integration/snapshots/rpc/test_rpc_trace_v3_tree.stdout.txt");

    let output = project
        .acton()
        .current_dir(project.path())
        .arg("--color")
        .arg("never")
        .arg("rpc")
        .arg("trace")
        .arg(TRACE_ROOT_HASH)
        .arg("--net")
        .arg("custom:mock")
        .arg("--show-bodies")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run();

    output.success().assert_snapshot_matches(
        "integration/snapshots/rpc/test_rpc_trace_v3_tree_show_bodies.stdout.txt",
    );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(
        captured.len(),
        4,
        "expected exactly four TonCenter requests"
    );
    for request_idx in [0, 2] {
        assert_eq!(captured[request_idx].method, "GET");
        assert_eq!(
            captured[request_idx].path,
            format!("/api/v3/traces?tx_hash={TRACE_ROOT_HASH}&limit=1"),
            "unexpected trace request path"
        );
        assert_eq!(
            header_value(&captured[request_idx].headers, "X-API-Key"),
            Some("custom-mock-api-key"),
            "rpc trace should send TonCenter API keys for custom networks from MOCK_API_KEY",
        );
    }
    for request_idx in [1, 3] {
        assert_eq!(captured[request_idx].method, "GET");
        assert!(
            captured[request_idx]
                .path
                .starts_with("/api/v3/accountStates?"),
            "unexpected accountStates request path: {}",
            captured[request_idx].path
        );
    }
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_trace_formats_v3_trace_without_in_msg() {
    let project = ProjectBuilder::new("rpc-trace-v3-missing-in-msg").build();
    let log_dir = prepare_log_dir(project.path());
    let (mock_url, mock_handle, captured) =
        spawn_toncenter_v2_mock(vec![toncenter_v3_trace_without_in_msg_response(
            MATCHED_INFO_ADDRESS,
        )]);
    append_custom_network_with_urls(
        project.path(),
        "mock",
        &format!("{mock_url}/api/v2"),
        &format!("{mock_url}/api/v3"),
    );

    project
        .acton()
        .current_dir(project.path())
        .arg("--color")
        .arg("never")
        .arg("rpc")
        .arg("trace")
        .arg(TRACE_ROOT_HASH)
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_trace_without_in_msg.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 1, "expected exactly one TonCenter request");
    assert_eq!(
        captured[0].path,
        format!("/api/v3/traces?tx_hash={TRACE_ROOT_HASH}&limit=1"),
        "unexpected trace request path"
    );
}

fn toncenter_v3_trace_ok_response(
    counter_address: &str,
    owner_address: &str,
    body_boc64: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "traces": [{
                "trace_id": TRACE_ROOT_HASH,
                "transactions_order": [TRACE_ROOT_HASH, TRACE_CHILD_HASH, TRACE_RETURN_HASH],
                "transactions": {
                    TRACE_ROOT_HASH: {
                        "account": counter_address,
                        "hash": TRACE_ROOT_HASH,
                        "lt": "100",
                        "now": 1_700_000_000_u32,
                        "mc_block_seqno": 10_000_u32,
                        "orig_status": "active",
                        "end_status": "active",
                        "total_fees": "1200",
                        "description": successful_v3_description(1),
                        "in_msg": {
                            "hash": "root-in-msg",
                            "source": owner_address,
                            "destination": counter_address,
                            "value": "100000000",
                            "bounce": true,
                            "bounced": false,
                            "message_content": {
                                "body": body_boc64
                            }
                        },
                        "out_msgs": [{
                            "hash": "child-msg",
                            "source": counter_address,
                            "destination": owner_address,
                            "value": "1",
                            "bounce": false,
                            "bounced": false
                        }, {
                            "hash": "return-msg",
                            "source": counter_address,
                            "destination": counter_address,
                            "value": "2",
                            "bounce": false,
                            "bounced": false,
                            "message_content": {
                                "body": counter_return_excesses_body_boc64(7)
                            }
                        }]
                    },
                    TRACE_CHILD_HASH: {
                        "account": owner_address,
                        "hash": TRACE_CHILD_HASH,
                        "lt": "101",
                        "now": 1_700_000_001_u32,
                        "mc_block_seqno": 10_001_u32,
                        "orig_status": "active",
                        "end_status": "active",
                        "total_fees": "100",
                        "description": successful_v3_description(0),
                        "in_msg": {
                            "hash": "child-msg",
                            "source": counter_address,
                            "destination": owner_address,
                            "value": "1",
                            "bounce": false,
                            "bounced": false
                        },
                        "out_msgs": []
                    },
                    TRACE_RETURN_HASH: {
                        "account": counter_address,
                        "hash": TRACE_RETURN_HASH,
                        "lt": "102",
                        "now": 1_700_000_002_u32,
                        "mc_block_seqno": 10_002_u32,
                        "orig_status": "active",
                        "end_status": "active",
                        "total_fees": "100",
                        "description": successful_v3_description(0),
                        "in_msg": {
                            "hash": "return-msg",
                            "source": counter_address,
                            "destination": counter_address,
                            "value": "2",
                            "bounce": false,
                            "bounced": false,
                            "message_content": {
                                "body": counter_return_excesses_body_boc64(7)
                            }
                        },
                        "out_msgs": []
                    }
                },
                "is_incomplete": false
            }]
        })
        .to_string(),
    }
}

fn successful_v3_description(messages_created: u16) -> serde_json::Value {
    serde_json::json!({
        "type": "ord",
        "aborted": false,
        "destroyed": false,
        "credit_first": false,
        "compute_ph": {
            "skipped": false,
            "success": true,
            "gas_fees": "1000",
            "gas_used": "123",
            "gas_limit": "1000",
            "exit_code": 0,
            "vm_steps": 5
        },
        "action": {
            "success": true,
            "valid": true,
            "no_funds": false,
            "result_code": 0,
            "tot_actions": messages_created,
            "msgs_created": messages_created
        }
    })
}

fn toncenter_v3_account_states_ok_response(
    counter_address: &str,
    owner_address: &str,
    counter_code_boc64: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "accounts": [
                {
                    "address": counter_address,
                    "balance": "1000000000",
                    "code_boc": counter_code_boc64,
                    "status": "active"
                },
                {
                    "address": owner_address,
                    "balance": "0",
                    "code_boc": null,
                    "status": "uninit"
                }
            ]
        })
        .to_string(),
    }
}

fn toncenter_v3_trace_without_in_msg_response(account: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "traces": [{
                "trace_id": TRACE_ROOT_HASH,
                "transactions_order": [TRACE_ROOT_HASH],
                "transactions": {
                    TRACE_ROOT_HASH: {
                        "account": account,
                        "hash": TRACE_ROOT_HASH,
                        "lt": "100",
                        "now": 1_700_000_000_u32,
                        "mc_block_seqno": 10_000_u32,
                        "orig_status": "active",
                        "end_status": "active",
                        "total_fees": "1200",
                        "description": successful_v3_description(0),
                        "out_msgs": []
                    }
                },
                "is_incomplete": false
            }]
        })
        .to_string(),
    }
}

fn counter_increase_body_boc64(increase_by: u32) -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(0x7e87_64ef)
        .expect("must store IncreaseCounter opcode");
    builder
        .store_u32(increase_by)
        .expect("must store IncreaseCounter payload");
    let cell = builder.build().expect("must build body cell");
    Boc::encode_base64(&cell)
}

fn counter_return_excesses_body_boc64(query_id: u64) -> String {
    let mut builder = CellBuilder::new();
    builder
        .store_u32(0xd532_76db)
        .expect("must store ReturnExcessesBack opcode");
    builder
        .store_u64(query_id)
        .expect("must store ReturnExcessesBack query_id");
    let cell = builder.build().expect("must build body cell");
    Boc::encode_base64(&cell)
}

fn counter_storage_boc64(id: u32, owner_address: &str, counter: u32) -> String {
    let owner = owner_address
        .parse::<IntAddr>()
        .expect("owner address must parse");
    let mut builder = CellBuilder::new();
    builder.store_u32(id).expect("must store id");
    owner
        .store_into(&mut builder, Cell::empty_context())
        .expect("must store owner");
    builder.store_u32(counter).expect("must store counter");
    let cell = builder.build().expect("must build storage cell");
    Boc::encode_base64(&cell)
}

fn test_cell_boc64(value: u32) -> String {
    let mut builder = CellBuilder::new();
    builder.store_u32(value).expect("must store u32");
    let cell = builder.build().expect("must build cell");
    Boc::encode_base64(&cell)
}

fn write_deployer_wallets(project_root: &Path) {
    fs::write(project_root.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
}

fn start_localnet_with_localnet(project: &Project) -> crate::support::localnet::LocalnetHandle {
    let node = project.localnet().args(["--accounts", "deployer"]).start();
    append_localnet_network(project.path(), &format!("{}/api/v2", node.base_url()));
    node
}

fn stdout(output: &crate::support::assertions::TestOutput) -> String {
    String::from_utf8(output.output.get_output().stdout.clone())
        .expect("command stdout must be utf-8")
}

fn extract_marker_value(output: &str, marker: &str) -> String {
    let cleaned = strip_ansi(output);
    cleaned
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(marker).map(ToOwned::to_owned))
        .unwrap_or_else(|| panic!("Marker `{marker}` not found in output:\n{cleaned}"))
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn assert_request_snapshot(request: &CapturedToncenterRequest, snapshot_path: &str) {
    let mut normalized = format!("{} {}\n", request.method, request.path);
    if !request.body.is_empty() {
        let body = std::str::from_utf8(&request.body).expect("request body must be utf-8");
        normalized.push_str(body);
        normalized.push('\n');
    }

    let expected_path = Path::new("tests").join(snapshot_path);
    let expected = fs::read_to_string(&expected_path).unwrap_or_else(|err| {
        panic!(
            "request snapshot {} must exist: {err}\n\nactual:\n{normalized}",
            expected_path.display()
        )
    });
    assertion().eq(normalized, expected);
}

fn prepare_log_dir(project_root: &Path) -> String {
    let log_dir = project_root.join(".acton-logs");
    fs::create_dir_all(&log_dir).expect("must create log dir");
    log_dir.to_string_lossy().into_owned()
}

fn assert_localnet_rpc_snapshot(
    output: &crate::support::assertions::TestSuccess,
    snapshot_path: &str,
) {
    let normalized = normalize_localnet_rpc_stdout(&output.get_normalized_stdout());
    let expected_path = Path::new("tests").join(snapshot_path);
    let expected =
        fs::read_to_string(&expected_path).expect("localnet rpc snapshot file must exist");
    assertion().eq(normalized, expected);
}

fn normalize_localnet_rpc_stdout(stdout: &str) -> String {
    let mut normalized_lines = Vec::new();
    for line in stdout.lines() {
        if let Some((prefix, _)) = line.split_once("Last Tx Hash:") {
            normalized_lines.push(format!("{prefix}Last Tx Hash:      [TX_HASH]"));
        } else {
            normalized_lines.push(line.to_owned());
        }
    }
    let mut normalized = normalized_lines.join("\n");
    if stdout.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}
