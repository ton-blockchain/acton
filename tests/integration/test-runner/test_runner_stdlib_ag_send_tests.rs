//! Reserved integration test module for subagent AG.
//!
//! Ownership boundary for agent AG:
//! - tests/integration/test-runner/test_runner_stdlib_ag_send_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/**
//! - tests/integration/testdata/test_std_agent_ag/**
//! - tests/support/test_std_agent_ag/** (optional)
//!
//! Required test name prefix:
//! - ag_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_MESSAGES: &str = r#"
struct (0x91000001) TriggerForward {
    queryId: uint64
    target: address
}

struct (0x91000002) Notify {
    queryId: uint64
}

struct (0x91000003) TriggerEcho {
    queryId: uint64
}

struct (0x91000004) EchoNotice {
    queryId: uint64
}

struct (0x91000005) TriggerExternal {
    queryId: uint64
}

struct (0x91000006) ExternalNotice {
    count: uint32
}
"#;

const FORWARDER_CONTRACT: &str = r#"
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

const RECEIVER_CONTRACT: &str = r#"
import "messages"

struct Storage {
    received: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { received: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy Notify.fromSlice(in.body);
    var storage = loadStorage();
    storage.received = storage.received + 1;
    saveStorage(storage);
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun received(): int {
    return loadStorage().received;
}
"#;

const EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"
import "messages"

struct Storage {
    externalCount: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { externalCount: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onExternalMessage() {
    acceptExternalMessage();

    var storage = loadStorage();
    storage.externalCount = storage.externalCount + 1;
    saveStorage(storage);

    createExternalLogMessage({
        dest: createAddressNone(),
        body: ExternalNotice {
            count: storage.externalCount,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun externalCount(): int {
    return loadStorage().externalCount;
}
"#;

const ECHO_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerEcho.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: in.senderAddress,
        body: EchoNotice {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const NETWORK_TEST_PRELUDE: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/tlb/maybe"
import "../contracts/messages"

fun deployForwardHarness() {
    val sender = net.treasury("sender");

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

    val deployForwarder = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: forwarderInit,
        },
    });
    expect(net.send(sender.address, deployForwarder)).toHaveSuccessfulDeploy({ to: forwarderAddress });

    val deployReceiver = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: receiverInit,
        },
    });
    expect(net.send(sender.address, deployReceiver)).toHaveSuccessfulDeploy({ to: receiverAddress });

    return (sender, forwarderAddress, receiverAddress);
}

fun deployExternalHarness() {
    val sender = net.treasury("sender");

    val externalInit = ContractState {
        code: build("external"),
        data: createEmptyCell(),
    };
    val externalAddress = AutoDeployAddress { stateInit: externalInit }.calculateAddress();

    val deployExternal = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: externalInit,
        },
    });
    expect(net.send(sender.address, deployExternal)).toHaveSuccessfulDeploy({ to: externalAddress });

    return (sender, externalAddress);
}

fun deployEchoHarness() {
    val sender = net.treasury("sender");

    val echoInit = ContractState {
        code: build("echo"),
        data: createEmptyCell(),
    };
    val echoAddress = AutoDeployAddress { stateInit: echoInit }.calculateAddress();

    val deployEcho = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: echoInit,
        },
    });
    expect(net.send(sender.address, deployEcho)).toHaveSuccessfulDeploy({ to: echoAddress });

    return (sender, echoAddress);
}

fun receiverCount(addr: address): int {
    return net.runGetMethod<int>(addr, "received");
}

fun externalCount(addr: address): int {
    return net.runGetMethod<int>(addr, "externalCount");
}
"#;

fn with_network_test_source(test_body: &str) -> String {
    format!("{NETWORK_TEST_PRELUDE}\n{test_body}\n")
}

fn network_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = with_network_test_source(test_body);
    ProjectBuilder::new(project_name)
        .file("contracts/messages", NETWORK_MESSAGES)
        .contract("forwarder", FORWARDER_CONTRACT)
        .contract("receiver", RECEIVER_CONTRACT)
        .contract("external", EXTERNAL_CONTRACT)
        .contract("echo", ECHO_CONTRACT)
        .test_file("network_paths", &source)
}

fn run_network_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    network_project(project_name, test_body)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn send_processes_children_and_find_transaction_by_participants_and_opcode() {
    run_network_success(
        "ag-stdlib-send-processes-children",
        r#"
get fun `test-ag-send-processes-children-and-find-transaction`() {
    val (sender, forwarderAddress, receiverAddress) = deployForwardHarness();

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.5"),
            dest: forwarderAddress,
            body: TriggerForward {
                queryId: 11,
                target: receiverAddress,
            },
        }),
    );

    expect(txs).toHaveLength(2);
    expect(txs.findTransaction<TriggerForward>({
        from: sender.address,
        to: forwarderAddress,
        success: true,
    })).toBeDefined();
    expect(txs.findTransaction<Notify>({
        from: forwarderAddress,
        to: receiverAddress,
        success: true,
    })).toBeDefined();
    expect(receiverCount(receiverAddress)).toEqual(1);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_send_processes_children_and_find_transaction_by_participants_and_opcode.stdout.txt",
    );
}

#[test]
fn send_single_keeps_child_list_empty_and_preserves_out_message() {
    run_network_success(
        "ag-stdlib-send-single-keeps-out-message",
        r#"
get fun `test-ag-send-single-keeps-child-list-empty`() {
    val (sender, forwarderAddress, receiverAddress) = deployForwardHarness();

    val sendSingleRes = net.sendSingle(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.5"),
            dest: forwarderAddress,
            body: TriggerForward {
                queryId: 22,
                target: receiverAddress,
            },
        }),
    );

    expect(sendSingleRes.childTxs.size()).toEqual(0);
    expect(sendSingleRes.outMessages.size()).toEqual(1);

    val notice = sendSingleRes.outMessages.at<Notify>(0).loadBody();
    expect(notice.queryId).toEqual(22);

    expect(receiverCount(receiverAddress)).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_send_single_keeps_child_list_empty_and_preserves_out_message.stdout.txt",
    );
}

#[test]
fn send_external_runs_handler_and_collects_external_out_message() {
    run_network_success(
        "ag-stdlib-send-external-runs-handler",
        r#"
get fun `test-ag-send-external-runs-handler`() {
    val (_, externalAddress) = deployExternalHarness();

    val txs = net.sendExternal(
        createExternalMessage(externalAddress, TriggerExternal { queryId: 1 }),
    );

    expect(txs).toHaveLength(1);
    expect(txs).toHaveSuccessfulTx();
    expect(txs.at(0).externals).toHaveLength(1);

    val externalLog = txs.at(0).externals.at<ExternalNotice>(0).loadBody();
    expect(externalLog.count).toEqual(1);
    expect(externalCount(externalAddress)).toEqual(1);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_send_external_runs_handler_and_collects_external_out_message.stdout.txt",
    );
}

#[test]
fn send_external_is_repeatable_and_keeps_incrementing_state() {
    run_network_success(
        "ag-stdlib-send-external-repeatable",
        r#"
get fun `test-ag-send-external-repeatable`() {
    val (_, externalAddress) = deployExternalHarness();

    val first = net.sendExternal(
        createExternalMessage(externalAddress, TriggerExternal { queryId: 2 }),
    );
    val second = net.sendExternal(
        createExternalMessage(externalAddress, TriggerExternal { queryId: 3 }),
    );

    expect(first).toHaveLength(1);
    expect(second).toHaveLength(1);

    val firstLog = first.at(0).externals.at<ExternalNotice>(0).loadBody();
    val secondLog = second.at(0).externals.at<ExternalNotice>(0).loadBody();
    expect(firstLog.count).toEqual(1);
    expect(secondLog.count).toEqual(2);
    expect(externalCount(externalAddress)).toEqual(2);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_send_external_is_repeatable_and_keeps_incrementing_state.stdout.txt",
    );
}

#[test]
fn find_transaction_matches_body_hash_and_returns_null_for_mismatch() {
    run_network_success(
        "ag-stdlib-find-transaction-body-hash",
        r#"
get fun `test-ag-find-transaction-body-hash`() {
    val (sender, forwarderAddress, receiverAddress) = deployForwardHarness();

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.5"),
            dest: forwarderAddress,
            body: TriggerForward {
                queryId: 33,
                target: receiverAddress,
            },
        }),
    );

    val expectedBody = TriggerForward {
        queryId: 33,
        target: receiverAddress,
    }.toCell();
    val wrongBody = TriggerForward {
        queryId: 34,
        target: receiverAddress,
    }.toCell();

    expect(txs.findTransaction<TriggerForward>({
        to: forwarderAddress,
        body: expectedBody,
    })).toBeDefined();
    val notFound = txs.findTransaction<TriggerForward>({
        to: forwarderAddress,
        body: wrongBody,
    });
    expect(notFound is None).toEqual(true);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_find_transaction_matches_body_hash_and_returns_null_for_mismatch.stdout.txt",
    );
}

#[test]
fn find_transaction_matches_bounced_opcode_after_prefix() {
    run_network_success(
        "ag-stdlib-find-transaction-bounced-opcode",
        r#"
get fun `test-ag-find-transaction-bounced-opcode`() {
    val (sender, echoAddress) = deployEchoHarness();

    val initial = net.sendSingle(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.4"),
            dest: echoAddress,
            body: TriggerEcho {
                queryId: 44,
            },
        }),
    );

    val noticeBody = initial.outMessages.at<EchoNotice>(0).loadBody().toCell();
    val bouncedBody = beginCell()
        .storeUint(0xFFFFFFFF, 32)
        .storeSlice(noticeBody.beginParse())
        .endCell();

    val bouncedRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: echoAddress,
            body: bouncedBody,
        }).bounced(),
    );

    expect(bouncedRes.findTransaction<EchoNotice>({
        from: sender.address,
        to: echoAddress,
        bounced: true,
    })).toBeDefined();
    val notBouncedMatch = bouncedRes.findTransaction<EchoNotice>({
        from: sender.address,
        to: echoAddress,
        bounced: false,
    });
    expect(notBouncedMatch is None).toEqual(true);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_find_transaction_matches_bounced_opcode_after_prefix.stdout.txt",
    );
}

#[test]
fn wait_returns_true_in_emulation_mode_for_non_empty_and_empty_results() {
    run_network_success(
        "ag-stdlib-wait-in-emulation-mode",
        r#"
get fun `test-ag-wait-in-emulation-mode`() {
    val (sender, forwarderAddress, receiverAddress) = deployForwardHarness();

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.3"),
            dest: forwarderAddress,
            body: TriggerForward {
                queryId: 55,
                target: receiverAddress,
            },
        }),
    );

    expect(txs.wait()).toEqual(true);
    expect(txs.wait(true, 1, 1)).toEqual(true);

    val empty: SendResultList = createEmptyTuple();
    expect(empty.wait()).toEqual(true);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_wait_returns_true_in_emulation_mode_for_non_empty_and_empty_results.stdout.txt",
    );
}

#[test]
fn wait_returns_false_for_empty_list_in_broadcast_mode() {
    run_network_success(
        "ag-stdlib-wait-empty-broadcast-false",
        r#"
get fun `test-ag-wait-empty-list-in-broadcast-mode`() {
    net.enableBroadcast();
    val empty: SendResultList = createEmptyTuple();
    expect(empty.wait()).toEqual(false);
    net.disableBroadcast();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_wait_returns_false_for_empty_list_in_broadcast_mode.stdout.txt",
    );
}

#[test]
fn wait_rejects_zero_attempts_in_broadcast_mode() {
    network_project(
        "ag-stdlib-wait-zero-attempts-rejected",
        r#"
get fun `test-ag-wait-zero-attempts-rejected`() {
    val (sender, forwarderAddress, receiverAddress) = deployForwardHarness();

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.3"),
            dest: forwarderAddress,
            body: TriggerForward {
                queryId: 66,
                target: receiverAddress,
            },
        }),
    );

    net.enableBroadcast();
    txs.wait(true, 0, 1);
}
"#,
    )
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_contains("Attempt number must be positive")
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/test_runner_stdlib_ag_send_tests/ag_stdlib_wait_rejects_zero_attempts_in_broadcast_mode.stdout.txt",
    );
}
