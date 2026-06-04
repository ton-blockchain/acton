use crate::common::{assertion, strip_ansi};
use crate::support::TestOutputExt;
use crate::support::project::{ActonCommand, Project, ProjectBuilder};
use crate::support::toncenter::{
    CapturedToncenterRequest, append_custom_network, append_localnet_network,
    spawn_toncenter_v2_mock, spawn_toncenter_v2_mock_with_capture,
    toncenter_v2_account_info_with_code_ok_response, toncenter_v2_run_get_method_ok_response,
};
use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;
use std::time::Duration;
use tvm_ffi::stack::TupleItem;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::models::message::IntAddr;

const RAW_INFO_ADDRESS: &str = "0:1111111111111111111111111111111111111111111111111111111111111111";
const MATCHED_INFO_ADDRESS: &str =
    "0:2222222222222222222222222222222222222222222222222222222222222222";
const MATCHED_INFO_OWNER_ADDRESS: &str =
    "0:3333333333333333333333333333333333333333333333333333333333333333";

const RPC_CALL_STORAGE_TYPES: &str = r"
struct Storage {
    id: uint32
    owner: address
    counter: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
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

const RPC_CALL_TYPES_CONTRACT: &str = r#"
import "types"
import "@stdlib/lisp-lists"

struct BasicReply {
    ok: bool
    title: string
    owner: address
    maybeOwner: address?
}

struct ScalarReply {
    signed: int
    unsigned: uint32
    amount: coins
    ok: bool
    title: string
    owner: address
    anyOwner: any_address
    maybeOwner: address?
    missingOwner: address?
    optionalInt: int?
    cellValue: cell
    sliceValue: slice
    bitsValue: bits32
}

struct PluginListEntry {
    workchain: int32
    address: uint256
}

struct ContainerReply {
    numbers: array<int32>
    balances: map<int32, int32>
}

contract TypesContract {
    storage: Storage
}

fun onInternalMessage(_in: InMessage) {}

get fun acceptBasic(
    i: int,
    u: uint32,
    flag: bool,
    title: string,
    owner: address,
    maybeOwner: address?,
    optionalInt: int?,
    cellArg: cell,
    sliceArg: slice,
    bits: bits32,
): int {
    return i + u;
}

get fun basicReply(): BasicReply {
    val storage = lazy Storage.load();

    return BasicReply {
        ok: true,
        title: "hello",
        owner: storage.owner,
        maybeOwner: null,
    };
}

get fun scalarReply(): ScalarReply {
    val storage = lazy Storage.load();

    return ScalarReply {
        signed: -123,
        unsigned: 123,
        amount: 1500000000,
        ok: true,
        title: "hello",
        owner: storage.owner,
        anyOwner: storage.owner as any_address,
        maybeOwner: storage.owner,
        missingOwner: null,
        optionalInt: null,
        cellValue: beginCell().storeUint(42, 8).endCell(),
        sliceValue: beginCell().storeAddress(storage.owner).endCell().beginParse(),
        bitsValue: beginCell().storeUint(0xaabbccdd, 32).endCell().beginParse() as bits32,
    };
}

get fun anyAddressReply(): any_address {
    val storage = lazy Storage.load();

    return storage.owner as any_address;
}

get fun pluginList(): lisp_list<PluginListEntry> {
    return [
        PluginListEntry { workchain: 0, address: 4369 },
        PluginListEntry { workchain: -1, address: 8738 },
    ] as lisp_list<PluginListEntry>;
}

get fun emptyPluginList(): lisp_list<PluginListEntry> {
    return [];
}

get fun numberArray(): array<int32> {
    return [1, 2, 3];
}

get fun pair(): [int32, bool] {
    return [7, true];
}

get fun balances(): map<int32, int32> {
    var value = map<int32, int32> [];
    value.set(1, 10);
    value.set(2, 20);

    return value;
}

get fun containerReply(): ContainerReply {
    var value = map<int32, int32> [];
    value.set(7, 70);
    value.set(8, 80);

    return ContainerReply {
        numbers: [4, 5],
        balances: value,
    };
}

get fun nothing(): void {}

get fun unsupportedStruct(arg: Storage): int {
    return arg.id;
}

get fun acceptAnyAddress(arg: any_address): int {
    if (arg.isNone()) {
        return 1;
    }
    if (arg.isInternal()) {
        return 2;
    }
    return 3;
}
"#;

const DEPLOYER_WALLET_CONFIG: &str = r#"[wallets.deployer]
kind = "v4r2"
workchain = 0
keys = { mnemonic = "cupboard match uphold miracle fog balance unknown region share hand trophy million toy narrow ability exchange first toast fresh maid report cram strong later" }
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

#[test]
fn test_rpc_call_runs_counter_methods_from_localnet() {
    let (project, node, log_dir, counter_address) =
        deploy_counter_to_localnet("rpc-call-localnet-counter-methods");

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&counter_address)
        .arg("currentCounter")
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_zero_arg_local_abi.stdout.txt",
        );

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&counter_address)
        .arg("double")
        .arg("--net")
        .arg("localnet")
        .arg("21")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_localnet_argument.stdout.txt",
        );

    let double_method_id = get_method_id(project.path(), "double").to_string();
    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&counter_address)
        .arg(&double_method_id)
        .arg("--net")
        .arg("localnet")
        .arg("21")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_numeric_method_id_localnet.stdout.txt",
        );

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&counter_address)
        .arg("double")
        .arg("--net")
        .arg("localnet")
        .arg("--json")
        .arg("7")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/rpc/test_rpc_call_args_json.stdout.txt");

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&counter_address)
        .arg("missing")
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_unknown_get_method.stderr.txt",
        );

    node.stop();
}

#[test]
fn test_rpc_call_decodes_abi_return_types_from_localnet() {
    let (project, node, log_dir, contract_address) =
        deploy_types_contract_to_localnet("rpc-call-localnet-return-types");

    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "basicReply",
        &[],
        "integration/snapshots/rpc/test_rpc_call_complex_result.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "basicReply",
        &["--json"],
        "integration/snapshots/rpc/test_rpc_call_complex_result_json.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "scalarReply",
        &[],
        "integration/snapshots/rpc/test_rpc_call_scalar_return_types.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "anyAddressReply",
        &[],
        "integration/snapshots/rpc/test_rpc_call_any_address_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "pluginList",
        &[],
        "integration/snapshots/rpc/test_rpc_call_lisp_list_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "emptyPluginList",
        &[],
        "integration/snapshots/rpc/test_rpc_call_empty_lisp_list_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "numberArray",
        &[],
        "integration/snapshots/rpc/test_rpc_call_array_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "pair",
        &[],
        "integration/snapshots/rpc/test_rpc_call_tuple_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "balances",
        &[],
        "integration/snapshots/rpc/test_rpc_call_map_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "containerReply",
        &[],
        "integration/snapshots/rpc/test_rpc_call_struct_container_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "nothing",
        &[],
        "integration/snapshots/rpc/test_rpc_call_void_return.stdout.txt",
    );
    assert_localnet_call_snapshot(
        &project,
        &log_dir,
        &contract_address,
        "basicReply",
        &["--raw"],
        "integration/snapshots/rpc/test_rpc_call_raw_known_abi.stdout.txt",
    );

    node.stop();
}

#[test]
fn test_rpc_call_parses_and_rejects_abi_arguments_from_localnet() {
    let (project, node, log_dir, contract_address) =
        deploy_types_contract_to_localnet("rpc-call-localnet-abi-errors");

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&contract_address)
        .arg("acceptBasic")
        .arg("--net")
        .arg("localnet")
        .arg("1")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_wrong_argument_count.stderr.txt",
        );

    let cell_arg = test_cell_boc_hex(0x0102_0304);
    let slice_arg = Boc::encode_hex(address_cell(MATCHED_INFO_OWNER_ADDRESS));
    let bits_arg = test_cell_boc_hex(0xaabb_ccdd);
    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&contract_address)
        .arg("acceptBasic")
        .arg("--net")
        .arg("localnet")
        .arg("-5")
        .arg("not-a-number")
        .arg("true")
        .arg("hello")
        .arg(MATCHED_INFO_OWNER_ADDRESS)
        .arg("null")
        .arg("null")
        .arg(&cell_arg)
        .arg(&slice_arg)
        .arg(&bits_arg)
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_invalid_argument_value.stderr.txt",
        );

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&contract_address)
        .arg("unsupportedStruct")
        .arg("--net")
        .arg("localnet")
        .arg("1")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_unsupported_argument_type.stderr.txt",
        );

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&contract_address)
        .arg("acceptAnyAddress")
        .arg("--net")
        .arg("localnet")
        .arg(MATCHED_INFO_OWNER_ADDRESS)
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_any_address_argument_internal.stdout.txt",
        );

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(&contract_address)
        .arg("acceptAnyAddress")
        .arg("--net")
        .arg("localnet")
        .arg("addr_none")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_any_address_argument_none.stdout.txt",
        );

    node.stop();
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_call_parses_basic_abi_argument_types_and_sends_stack() {
    let (project, log_dir, code_boc64) =
        build_rpc_call_project("rpc-call-basic-argument-types", RPC_CALL_TYPES_CONTRACT);
    let cell_arg = test_cell_boc_hex(0x0102_0304);
    let slice_arg = Boc::encode_hex(address_cell(MATCHED_INFO_OWNER_ADDRESS));
    let bits_arg = test_cell_boc_hex(0xaabb_ccdd);
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock_with_capture(vec![
        toncenter_v2_account_info_with_code_ok_response(
            1_234_000_000,
            &code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(37.into())], 0),
    ]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("acceptBasic")
        .arg("--net")
        .arg("custom:mock")
        .arg("-5")
        .arg("0x2a")
        .arg("true")
        .arg("\"hello rpc\"")
        .arg(MATCHED_INFO_OWNER_ADDRESS)
        .arg("null")
        .arg("null")
        .arg(&cell_arg)
        .arg(&slice_arg)
        .arg(&bits_arg)
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_basic_argument_types.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 2, "expected account info and runGetMethod");
    assert_json_request_body_snapshot(
        &captured[1],
        "integration/snapshots/rpc/test_rpc_call_basic_argument_types.request.json",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_call_custom_network_sends_api_key() {
    let (project, log_dir, code_boc64) =
        build_rpc_call_project("rpc-call-custom-network-api-key", RPC_CALL_COUNTER_CONTRACT);
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock_with_capture(vec![
        toncenter_v2_account_info_with_code_ok_response(
            1_234_000_000,
            &code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(42.into())], 0),
    ]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

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
        header_value(&captured[1], "X-API-Key"),
        Some("custom-mock-api-key"),
        "rpc call should send TonCenter API keys for custom networks from MOCK_API_KEY",
    );
}

#[test]
fn test_rpc_call_decodes_address_result_from_cell_stack_item() {
    let (project, log_dir, code_boc64) =
        build_rpc_call_project("rpc-call-address-cell-result", RPC_CALL_ADDRESS_CONTRACT);
    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_with_code_ok_response(
            1_234_000_000,
            &code_boc64,
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
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

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
    let (project, log_dir, code_boc64) =
        build_rpc_call_project("rpc-call-nonzero-exit-code", RPC_CALL_COUNTER_CONTRACT);
    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_with_code_ok_response(
            1_234_000_000,
            &code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(42.into())], 11),
    ]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

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
fn test_rpc_call_falls_back_to_raw_stack_when_abi_result_width_mismatches() {
    let (project, log_dir, code_boc64) =
        build_rpc_call_project("rpc-call-result-width-mismatch", RPC_CALL_COUNTER_CONTRACT);
    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_with_code_ok_response(
            1_234_000_000,
            &code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(
            vec![TupleItem::Int(42.into()), TupleItem::Int(43.into())],
            0,
        ),
    ]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

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
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_result_width_mismatch.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");
}

#[test]
fn test_rpc_call_without_abi_allows_zero_arg_raw_call() {
    let project = ProjectBuilder::new("rpc-call-raw-no-abi").build();
    let log_dir = prepare_log_dir(project.path());
    let raw_cell =
        Boc::decode_base64(test_cell_boc64(0x1234_5678)).expect("raw stack cell BoC must decode");
    let (mock_url, mock_handle) = spawn_toncenter_v2_mock(vec![
        toncenter_v2_account_info_with_code_ok_response(
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
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

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

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_call_without_abi_parses_raw_arguments() {
    let project = ProjectBuilder::new("rpc-call-args-no-abi").build();
    let log_dir = prepare_log_dir(project.path());
    let cell_arg = test_cell_boc_hex(0x0102_0304);
    let slice_arg = test_cell_boc_hex(0x1122_3344);
    let slice_arg = format!("slice:{slice_arg}");
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock_with_capture(vec![
        toncenter_v2_account_info_with_code_ok_response(
            777_000_000,
            &test_cell_boc64(0xdead_beef),
            &test_cell_boc64(0x1234_5678),
            "active",
            "",
            "17",
            "deadbeef",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(123.into())], 0),
    ]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(RAW_INFO_ADDRESS)
        .arg("seqno")
        .arg("--net")
        .arg("custom:mock")
        .arg("1")
        .arg("0x2a")
        .arg("true")
        .arg("null")
        .arg(MATCHED_INFO_OWNER_ADDRESS)
        .arg("addr_none")
        .arg(&cell_arg)
        .arg(&slice_arg)
        .arg("string:hello")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_args_without_abi.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 2, "expected account info and runGetMethod");
    assert_json_request_body_snapshot(
        &captured[1],
        "integration/snapshots/rpc/test_rpc_call_args_without_abi.request.json",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn test_rpc_call_unknown_numeric_method_id_with_abi_uses_raw_arguments() {
    let (project, log_dir, code_boc64) = build_rpc_call_project(
        "rpc-call-numeric-id-raw-with-abi",
        RPC_CALL_COUNTER_CONTRACT,
    );
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock_with_capture(vec![
        toncenter_v2_account_info_with_code_ok_response(
            1_234_000_000,
            &code_boc64,
            &counter_storage_boc64(7, MATCHED_INFO_OWNER_ADDRESS, 42),
            "active",
            "",
            "999",
            "c0ffee",
        ),
        toncenter_v2_run_get_method_ok_response(vec![TupleItem::Int(321.into())], 0),
    ]);
    append_custom_network(project.path(), "mock", &format!("{mock_url}/api/v2"));

    project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(MATCHED_INFO_ADDRESS)
        .arg("123456")
        .arg("--net")
        .arg("custom:mock")
        .arg("1")
        .env("ACTON_LOG_DIR", &log_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/rpc/test_rpc_call_unknown_numeric_method_id_with_abi.stdout.txt",
        );

    mock_handle.join().expect("mock server thread must finish");

    let captured = captured
        .lock()
        .expect("captured requests mutex should not be poisoned");
    assert_eq!(captured.len(), 2, "expected account info and runGetMethod");
    assert_json_request_body_snapshot(
        &captured[1],
        "integration/snapshots/rpc/test_rpc_call_unknown_numeric_method_id_with_abi.request.json",
    );
}

fn assert_localnet_call_snapshot(
    project: &Project,
    log_dir: &str,
    contract_address: &str,
    method: &str,
    extra_args: &[&str],
    snapshot_path: &str,
) {
    let mut cmd = project
        .acton()
        .current_dir(project.path())
        .arg("rpc")
        .arg("call")
        .arg(contract_address)
        .arg(method)
        .arg("--net")
        .arg("localnet")
        .env("ACTON_LOG_DIR", log_dir);
    for arg in extra_args {
        cmd = cmd.arg(arg);
    }
    cmd.run().success().assert_snapshot_matches(snapshot_path);
}

fn deploy_counter_to_localnet(
    name: &str,
) -> (
    Project,
    crate::support::localnet::LocalnetHandle,
    String,
    String,
) {
    deploy_contract_to_localnet(name, RPC_CALL_COUNTER_CONTRACT, "COUNTER_ADDRESS=")
}

fn deploy_types_contract_to_localnet(
    name: &str,
) -> (
    Project,
    crate::support::localnet::LocalnetHandle,
    String,
    String,
) {
    deploy_contract_to_localnet(name, RPC_CALL_TYPES_CONTRACT, "COUNTER_ADDRESS=")
}

fn deploy_contract_to_localnet(
    name: &str,
    contract: &str,
    address_marker: &str,
) -> (
    Project,
    crate::support::localnet::LocalnetHandle,
    String,
    String,
) {
    let project = ProjectBuilder::new(name)
        .file("contracts/types", RPC_CALL_STORAGE_TYPES)
        .contract("counter", contract)
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

    let contract_address = extract_marker_value(&deploy_stdout, address_marker);
    node.wait_until_address_state_active(&contract_address, Duration::from_secs(12));

    (project, node, log_dir, contract_address)
}

fn build_rpc_call_project(name: &str, contract: &str) -> (Project, String, String) {
    let project = ProjectBuilder::new(name)
        .file("contracts/types", RPC_CALL_STORAGE_TYPES)
        .contract("counter", contract)
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
        .expect("build artifact must contain code_boc64")
        .to_owned();

    (project, log_dir, code_boc64)
}

fn get_method_id(project_root: &Path, method_name: &str) -> i64 {
    let abi_path = project_root.join("build/abi/counter.json");
    let abi = fs::read_to_string(&abi_path).expect("ABI artifact must exist");
    let abi: JsonValue = serde_json::from_str(&abi).expect("ABI artifact must be valid json");
    abi["get_methods"]
        .as_array()
        .expect("ABI artifact must contain get_methods")
        .iter()
        .find(|method| method["name"].as_str() == Some(method_name))
        .and_then(|method| method["tvm_method_id"].as_i64())
        .unwrap_or_else(|| panic!("get-method `{method_name}` must exist in ABI"))
}

fn test_cell_boc64(value: u32) -> String {
    Boc::encode_base64(test_cell(u64::from(value), 32))
}

fn test_cell_boc_hex(value: u32) -> String {
    Boc::encode_hex(test_cell(u64::from(value), 32))
}

fn test_cell(value: u64, bits: u16) -> Cell {
    let mut builder = CellBuilder::new();
    builder.store_uint(value, bits).expect("must store uint");
    builder.build().expect("must build cell")
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

fn assert_json_request_body_snapshot(request: &CapturedToncenterRequest, snapshot_path: &str) {
    let body = std::str::from_utf8(&request.body).expect("request body must be utf-8");
    let json: JsonValue = serde_json::from_str(body).expect("request body must be valid json");
    let mut normalized = serde_json::to_string_pretty(&json).expect("json must format");
    normalized.push('\n');

    let expected_path = Path::new("tests").join(snapshot_path);
    let expected = fs::read_to_string(&expected_path).unwrap_or_else(|err| {
        panic!(
            "request body snapshot {} must exist: {err}\n\nactual:\n{normalized}",
            expected_path.display()
        )
    });
    assertion().eq(normalized, expected);
}

fn header_value<'a>(request: &'a CapturedToncenterRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

fn write_deployer_wallets(project_root: &Path) {
    fs::write(project_root.join("wallets.toml"), DEPLOYER_WALLET_CONFIG)
        .expect("failed to write wallets.toml");
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

fn prepare_log_dir(project_root: &Path) -> String {
    let log_dir = project_root.join(".acton-logs");
    fs::create_dir_all(&log_dir).expect("must create log dir");
    log_dir.to_string_lossy().into_owned()
}
