use crate::common::{assertion, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::{ActonCommand, Project, ProjectBuilder};
use serde_json::Value as JsonValue;
use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{thread, vec};
use tvm_ffi::json_stack::legacy_stack_to_json;
use tvm_ffi::stack::{Tuple, TupleItem};
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
const RPC_CALL_COUNTER_CONTRACT: &str = r#"
import "types"

contract Counter {
    storage: Storage
}

fun onInternalMessage(_in: InMessage) {}

get fun currentCounter(): int {
    val storage = lazy Storage.load();

    return storage.counter;
}

get fun double(value: uint32): int {
    return value * 2;
}
"#;
const RPC_CALL_ADDRESS_CONTRACT: &str = r#"
import "types"

struct OwnerReply {
    owner: address
}

contract Counter {
    storage: Storage
}

fun onInternalMessage(_in: InMessage) {}

get fun ownerReply(): OwnerReply {
    val storage = lazy Storage.load();

    return OwnerReply { owner: storage.owner };
}
"#;

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
    write_custom_network_config(project.path(), "mock", &mock_url);

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
    write_custom_network_config(project.path(), "mock", &mock_url);

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
    write_custom_network_config(project.path(), "mock", &mock_url);

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
    write_custom_network_config(project.path(), "mock", &mock_url);

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

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_call_runs_zero_arg_get_method_with_local_abi() {
    let project = ProjectBuilder::new("rpc-call-local-abi")
        .file("contracts/types", TRACE_COUNTER_TYPES)
        .contract("counter", RPC_CALL_COUNTER_CONTRACT)
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
        toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(42.into())], 0),
    ]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("currentCounter")
        .arg("--net")
        .arg("custom:mock")
        .env("MOCK_API_KEY", "custom-mock-api-key")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_zero_arg_local_abi.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 2, "expected account info and runGetMethod");
    assert_eq!(captured[0].method, "GET");
    assert_eq!(captured[1].method, "POST");
    assert_eq!(captured[1].path, "/api/v2/jsonRPC");
    assert_eq!(
        header_value(&captured[1].headers, "X-API-Key"),
        Some("custom-mock-api-key"),
        "rpc call should send TonCenter API keys for custom networks from MOCK_API_KEY",
    );
}

#[test]
fn test_rpc_call_parses_abi_arguments_and_prints_json() {
    let project = ProjectBuilder::new("rpc-call-args-json")
        .file("contracts/types", TRACE_COUNTER_TYPES)
        .contract("counter", RPC_CALL_COUNTER_CONTRACT)
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

    let (mock_url, mock_handle, _) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(14.into())], 0),
    ]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("double")
        .arg("--net")
        .arg("custom:mock")
        .arg("--json")
        .arg("7")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/rpc/test_rpc_call_args_json.stdout.txt");

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_call_decodes_address_result_from_cell_stack_item() {
    let project = ProjectBuilder::new("rpc-call-address-cell-result")
        .file("contracts/types", TRACE_COUNTER_TYPES)
        .contract("counter", RPC_CALL_ADDRESS_CONTRACT)
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

    let (mock_url, mock_handle, _) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(
            vec![TupleItem::Cell(address_cell(MATCHED_INFO_OWNER_ADDRESS))],
            0,
        ),
    ]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("ownerReply")
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_address_cell_result.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_call_prints_nonzero_exit_code_after_result() {
    let project = ProjectBuilder::new("rpc-call-nonzero-exit-code")
        .file("contracts/types", TRACE_COUNTER_TYPES)
        .contract("counter", RPC_CALL_COUNTER_CONTRACT)
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

    let (mock_url, mock_handle, _) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_ok_response(
            1_234_000_000,
            code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(42.into())], 11),
    ]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("currentCounter")
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_nonzero_exit_code.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_call_without_abi_allows_zero_arg_raw_call() {
    let project = ProjectBuilder::new("rpc-call-raw-no-abi").build();
    let log_dir = prepare_log_dir(project.path());
    let raw_cell =
        Boc::decode_base64(test_cell_boc64(0x1234_5678)).expect("raw stack cell BoC must decode");
    let (mock_url, mock_handle, _) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_ok_response(
            777_000_000,
            &test_cell_boc64(0xdead_beef),
            &test_cell_boc64(0x1234_5678),
            "active",
            "",
            "17",
            "deadbeef",
        ),
        toncenter_v2_run_get_method_ok_response(
            vec![
                TupleItem::Int(11.into()),
                TupleItem::Cell(address_cell(MATCHED_INFO_OWNER_ADDRESS)),
                TupleItem::Slice(addr_none_cell()),
                TupleItem::Cell(raw_cell),
                TupleItem::Int(20.into()),
                TupleItem::Int(10.into()),
                TupleItem::Int(0.into()),
                TupleItem::Int(1.into()),
                TupleItem::Int(2.into()),
                TupleItem::Int(123.into()),
            ],
            0,
        ),
    ]);
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(RAW_INFO_ADDRESS)
        .arg("seqno")
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_raw_without_abi.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_call_rejects_unknown_get_method_when_abi_is_known() {
    let project = ProjectBuilder::new("rpc-call-unknown-method")
        .file("contracts/types", TRACE_COUNTER_TYPES)
        .contract("counter", RPC_CALL_COUNTER_CONTRACT)
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
    write_custom_network_config(project.path(), "mock", &mock_url);

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("missing")
        .arg("--net")
        .arg("custom:mock")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_unknown_get_method.stderr.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
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
    append_localnet_network(project.path(), &node.base_url());
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
    wait_until_address_state_active(&node, &counter_address, Duration::from_secs(12));

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
    write_custom_network_config(project.path(), "mock", &mock_url);

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
    write_custom_network_config(project.path(), "mock", &mock_url);

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
    write_custom_network_config_with_v3(project.path(), "mock", &mock_url);

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
    write_custom_network_config_with_v3(project.path(), "mock", &mock_url);

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

#[derive(Debug, Clone)]
struct ToncenterV2MockResponse {
    status: u16,
    body: String,
}

#[derive(Debug, Clone)]
struct CapturedToncenterV2Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}

fn spawn_toncenter_v2_mock(
    responses: Vec<ToncenterV2MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterV2Request>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind TonCenter mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set TonCenter mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get TonCenter mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedToncenterV2Request>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for response in responses {
            let wait_until = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for TonCenter request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("TonCenter mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set TonCenter mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone TonCenter mock stream"),
            );

            let request_line = read_request_line(&mut reader);
            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_owned();
            let path = parts.next().unwrap_or_default().to_owned();
            let headers = read_headers(&mut reader);

            captured_requests_thread
                .lock()
                .expect("captured requests mutex should not be poisoned")
                .push(CapturedToncenterV2Request {
                    method,
                    path,
                    headers,
                });

            let raw_response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response.status,
                status_text(response.status),
                response.body.len(),
                response.body
            );
            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write TonCenter response");
            stream.flush().expect("failed to flush TonCenter response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

fn read_request_line(reader: &mut BufReader<std::net::TcpStream>) -> String {
    let mut request_line = String::new();
    let read_deadline = Instant::now() + Duration::from_secs(2);
    loop {
        request_line.clear();
        match reader.read_line(&mut request_line) {
            Ok(0) => {
                assert!(
                    Instant::now() <= read_deadline,
                    "timed out waiting for TonCenter request line"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Ok(_) => return request_line,
            Err(err) if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                assert!(
                    Instant::now() <= read_deadline,
                    "timed out waiting for TonCenter request line"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => panic!("failed to read TonCenter request line: {err}"),
        }
    }
}

fn read_headers(reader: &mut BufReader<std::net::TcpStream>) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    loop {
        let mut header_line = String::new();
        let read = reader
            .read_line(&mut header_line)
            .expect("failed to read TonCenter header line");
        if read == 0 || header_line == "\r\n" {
            return headers;
        }

        if let Some((name, value)) = header_line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

fn toncenter_v2_account_info_ok_response(
    balance: i64,
    code_boc64: &str,
    data_boc64: &str,
    state: &str,
    frozen_hash: &str,
    lt: &str,
    hash: &str,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "balance": balance.to_string(),
                "code": code_boc64,
                "data": data_boc64,
                "state": state,
                "frozen_hash": frozen_hash,
                "last_transaction_id": {
                    "lt": lt,
                    "hash": hash,
                }
            }
        })
        .to_string(),
    }
}

fn toncenter_v2_run_get_method_ok_response(
    stack: Vec<TupleItem>,
    exit_code: i32,
) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "stack": legacy_stack_to_json(&Tuple(stack)).expect("stack must serialize to legacy json"),
                "exit_code": exit_code
            }
        })
        .to_string(),
    }
}

fn toncenter_v2_masterchain_info_ok_response(seqno: u64) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "last": {
                    "@type": "ton.blockIdExt",
                    "workchain": -1,
                    "shard": "-9223372036854775808",
                    "seqno": seqno,
                    "root_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                    "file_hash": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
                }
            }
        })
        .to_string(),
    }
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

fn test_cell_boc64(value: u32) -> String {
    let mut builder = CellBuilder::new();
    builder.store_u32(value).expect("must store u32");
    let cell = builder.build().expect("must build cell");
    Boc::encode_base64(&cell)
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

fn address_cell(address: &str) -> Cell {
    let address = address.parse::<IntAddr>().expect("address must parse");
    let mut builder = CellBuilder::new();
    address
        .store_into(&mut builder, Cell::empty_context())
        .expect("must store address");
    builder.build().expect("must build address cell")
}

fn addr_none_cell() -> Cell {
    let mut builder = CellBuilder::new();
    builder.store_uint(0, 2).expect("must store addr_none");
    builder.build().expect("must build addr_none cell")
}

fn write_custom_network_config(project_root: &Path, name: &str, url: &str) {
    use std::fmt::Write as _;

    let config_path = project_root.join("Acton.toml");
    let mut config = fs::read_to_string(&config_path).expect("Acton.toml must exist");
    let _ = write!(
        config,
        "\n[networks.{name}]\napi = {{ v2 = \"{url}/api/v2\" }}\n"
    );
    fs::write(config_path, config).expect("failed to update Acton.toml");
}

fn write_custom_network_config_with_v3(project_root: &Path, name: &str, url: &str) {
    use std::fmt::Write as _;

    let config_path = project_root.join("Acton.toml");
    let mut config = fs::read_to_string(&config_path).expect("Acton.toml must exist");
    let _ = write!(
        config,
        "\n[networks.{name}]\napi = {{ v2 = \"{url}/api/v2\", v3 = \"{url}/api/v3\" }}\n"
    );
    fs::write(config_path, config).expect("failed to update Acton.toml");
}

fn write_deployer_wallets(project_root: &Path) {
    fs::write(project_root.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
}

fn start_localnet_with_localnet(project: &Project) -> crate::support::localnet::LocalnetHandle {
    let node = project.localnet().args(["--accounts", "deployer"]).start();
    append_localnet_network(project.path(), &node.base_url());
    node
}

fn append_localnet_network(project_path: &Path, base_url: &str) {
    use std::fmt::Write as _;

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
    fs::write(&acton_toml_path, acton_toml).expect("failed to write Acton.toml with localnet");
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
