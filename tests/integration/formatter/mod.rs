use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton::formatter::FormatterContext;
use tycho_types::models::{ReserveCurrencyFlags, SendMsgFlags};

const LINEAR_MESSAGES: &str = r#"
struct (0xF1000001) FmRoute {
    queryId: uint64
    mid: address
    sink: address
}

struct (0xF1000002) FmRelay {
    queryId: uint64
    sink: address
}

struct (0xF1000003) FmDelivered {
    queryId: uint64
    hop: uint8
}
"#;

const LINEAR_ROOT_CONTRACT: &str = r#"
import "fm_linear_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmRoute.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.mid,
        body: FmRelay {
            queryId: msg.queryId,
            sink: msg.sink,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const LINEAR_MID_CONTRACT: &str = r#"
import "fm_linear_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmRelay.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.sink,
        body: FmDelivered {
            queryId: msg.queryId,
            hop: 2,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const LINEAR_ROOT_OPCODE_MISMATCH_CONTRACT: &str = r#"
import "fm_linear_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmRoute.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.mid,
        body: FmDelivered {
            queryId: msg.queryId,
            hop: 1,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const LINEAR_SINK_CONTRACT: &str = r#"
import "fm_linear_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy FmDelivered.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const LINEAR_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_linear_messages"

fun deployFmLinearHarness() {
    val sender = net.treasury("sender");

    val rootInit = ContractState {
        code: build("fm_linear_root"),
        data: createEmptyCell(),
    };
    val rootAddress = AutoDeployAddress { stateInit: rootInit }.calculateAddress();

    val midInit = ContractState {
        code: build("fm_linear_mid"),
        data: createEmptyCell(),
    };
    val midAddress = AutoDeployAddress { stateInit: midInit }.calculateAddress();

    val sinkInit = ContractState {
        code: build("fm_linear_sink"),
        data: createEmptyCell(),
    };
    val sinkAddress = AutoDeployAddress { stateInit: sinkInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: rootInit,
        },
    }))).toHaveSuccessfulDeploy({ to: rootAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: midInit,
        },
    }))).toHaveSuccessfulDeploy({ to: midAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: sinkInit,
        },
    }))).toHaveSuccessfulDeploy({ to: sinkAddress });

    return (sender, rootAddress, midAddress, sinkAddress);
}

fun sendFmLinear(sender: Treasury, rootAddress: address, midAddress: address, sinkAddress: address, queryId: uint64): SendResultList {
    return net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.6"),
            dest: rootAddress,
            body: FmRoute {
                queryId,
                mid: midAddress,
                sink: sinkAddress,
            },
        }),
    );
}
"#;

const FANOUT_MESSAGES: &str = r#"
struct (0xF2000001) FmFanKick {
    queryId: uint64
    left: address
    right: address
}

struct (0xF2000002) FmLeftNotice {
    queryId: uint64
}

struct (0xF2000003) FmRightNotice {
    queryId: uint64
}
"#;

const FANOUT_ROOT_CONTRACT: &str = r#"
import "fm_fanout_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmFanKick.fromSlice(in.body);

    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.left,
        body: FmLeftNotice {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_REGULAR);

    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.right,
        body: FmRightNotice {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const FANOUT_LEFT_CONTRACT: &str = r#"
import "fm_fanout_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy FmLeftNotice.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const FANOUT_RIGHT_CONTRACT: &str = r#"
import "fm_fanout_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmRightNotice.fromSlice(in.body);
    if (msg.queryId == 0) {
        throw 1000;
    }
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const FANOUT_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_fanout_messages"

fun deployFmFanoutHarness() {
    val sender = net.treasury("sender");

    val rootInit = ContractState {
        code: build("fm_fanout_root"),
        data: createEmptyCell(),
    };
    val rootAddress = AutoDeployAddress { stateInit: rootInit }.calculateAddress();

    val leftInit = ContractState {
        code: build("fm_fanout_left"),
        data: createEmptyCell(),
    };
    val leftAddress = AutoDeployAddress { stateInit: leftInit }.calculateAddress();

    val rightInit = ContractState {
        code: build("fm_fanout_right"),
        data: createEmptyCell(),
    };
    val rightAddress = AutoDeployAddress { stateInit: rightInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: rootInit,
        },
    }))).toHaveSuccessfulTx({ to: rootAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: leftInit,
        },
    }))).toHaveSuccessfulTx({ to: leftAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: rightInit,
        },
    }))).toHaveSuccessfulTx({ to: rightAddress });

    return (sender, rootAddress, leftAddress, rightAddress);
}

fun sendFmFanout(sender: Treasury, rootAddress: address, leftAddress: address, rightAddress: address, queryId: uint64): SendResultList {
    return net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.5"),
            dest: rootAddress,
            body: FmFanKick {
                queryId,
                left: leftAddress,
                right: rightAddress,
            },
        }),
    );
}
"#;

const EXTERNAL_MESSAGES: &str = r#"
struct (0xF3000001) FmExternalTrigger {
    queryId: uint64
}

struct (0xF3000002) FmExternalNoneDest {
    queryId: uint64
}

struct (0xF3000003) FmExternalAddressDest {
    queryId: uint64
}
"#;

const EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"
import "fm_external_messages"

fun fmExternalAddress(tag: uint32): any_address {
    return beginCell()
        .storeUint(0b01, 2)
        .storeUint(32, 9)
        .storeUint(tag, 32)
        .endCell()
        .beginParse()
        .loadAddressAny();
}

fun onExternalMessage() {
    acceptExternalMessage();

    createExternalLogMessage({
        dest: createAddressNone(),
        body: FmExternalNoneDest {
            queryId: 1,
        },
    }).send(SEND_MODE_REGULAR);

    createExternalLogMessage({
        dest: fmExternalAddress(0xA1B2C3D4),
        body: FmExternalAddressDest {
            queryId: 2,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const EXTERNAL_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_external_messages"

fun fmExternalAddress(tag: uint32): any_address {
    return beginCell()
        .storeUint(0b01, 2)
        .storeUint(32, 9)
        .storeUint(tag, 32)
        .endCell()
        .beginParse()
        .loadAddressAny();
}

fun deployFmExternalHarness() {
    val sender = net.treasury("sender");

    val extInit = ContractState {
        code: build("fm_external_root"),
        data: createEmptyCell(),
    };
    val extAddress = AutoDeployAddress { stateInit: extInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: extInit,
        },
    }))).toHaveSuccessfulDeploy({ to: extAddress });

    return extAddress;
}
"#;

const EXTERNAL_THROW_CONTRACT: &str = r#"
import "@stdlib/gas-payments"

fun onExternalMessage() {
    acceptExternalMessage();
    throw 10;
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const EXTERNAL_THROW_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_external_messages"

fun fmExternalAddress(tag: uint32): any_address {
    return beginCell()
        .storeUint(0b01, 2)
        .storeUint(32, 9)
        .storeUint(tag, 32)
        .endCell()
        .beginParse()
        .loadAddressAny();
}

fun deployFmExternalThrowHarness() {
    val sender = net.treasury("sender");

    val extInit = ContractState {
        code: build("fm_external_throw"),
        data: createEmptyCell(),
    };
    val extAddress = AutoDeployAddress { stateInit: extInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: extInit,
        },
    }))).toHaveSuccessfulDeploy({ to: extAddress });

    return extAddress;
}
"#;

const BOUNCE_MESSAGES: &str = r#"
struct (0xF4000001) FmBouncePing {
    queryId: uint64
}

struct (0xF4000002) FmBounceAck {
    queryId: uint64
}
"#;

const BOUNCE_CONTRACT: &str = r#"
import "fm_bounce_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmBouncePing.fromSlice(in.body);

    createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: in.senderAddress,
        body: FmBounceAck {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const BOUNCE_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_bounce_messages"

fun deployFmBounceHarness() {
    val sender = net.treasury("sender");
    val init = ContractState {
        code: build("fm_bounce_echo"),
        data: createEmptyCell(),
    };
    val echoAddress = AutoDeployAddress { stateInit: init }.calculateAddress();
    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }))).toHaveSuccessfulDeploy({ to: echoAddress });

    return (sender, echoAddress);
}
"#;

const ACTION_CHILD_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ACTION_FAIL_CONTRACT: &str = r#"
import "../gen/fm_action_child_code.tolk"

fun onInternalMessage(_: InMessage) {
    val addr = AutoDeployAddress {
        stateInit: ContractState {
            code: fmActionChildCompiledCode(),
            data: createEmptyCell(),
        },
    }.calculateAddress();

    reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

    val outMsg = createMessage({
        dest: addr,
        bounce: false,
        value: ton("0.5"),
    });
    outMsg.send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ACTION_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"

fun fmActionInit() {
    val init = ContractState {
        code: build("fm_action_fail"),
        data: createEmptyCell(),
    };
    val actionAddress = AutoDeployAddress { stateInit: init }.calculateAddress();
    return (init, actionAddress);
}
"#;

const FLAGS_MESSAGES: &str = r#"
struct (0xF5000001) FmFlagsOk {
    queryId: uint64
}

struct (0xF5000002) FmFlagsThrow {
    queryId: uint64
}

struct (0xF5000003) FmFlagsActionFail {
    queryId: uint64
}
"#;

const FLAGS_CONTRACT: &str = r#"
import "fm_flags_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val opcode = in.body.preloadUint(32);
    if (opcode == 0xF5000001) {
        val _ok = lazy FmFlagsOk.fromSlice(in.body);
        return;
    }
    if (opcode == 0xF5000002) {
        val _throwMsg = lazy FmFlagsThrow.fromSlice(in.body);
        throw 10;
    }
    if (opcode == 0xF5000003) {
        val _failMsg = lazy FmFlagsActionFail.fromSlice(in.body);
        reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
        return;
    }
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const FLAGS_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_flags_messages"

fun deployFmFlagsHarness() {
    val sender = net.treasury("sender");

    val init = ContractState {
        code: build("fm_flags"),
        data: createEmptyCell(),
    };
    val flagsAddress = AutoDeployAddress { stateInit: init }.calculateAddress();
    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }))).toHaveSuccessfulDeploy({ to: flagsAddress });

    return (sender, flagsAddress);
}

fun unknownFmFlagsAddress() {
    return address("0:0000000000000000000000000000000000000000000000000000000000000BAD");
}
"#;

const DEBUG_MESSAGES: &str = r#"
struct (0xF6000001) FmDebugPing {
    queryId: uint64
}
"#;

const DEBUG_CONTRACT: &str = r#"
import "fm_debug_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy FmDebugPing.fromSlice(in.body);
    debug.printString("fmt-debug-marker");
    debug.print(msg.queryId);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DEBUG_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_debug_messages"

fun deployFmDebugHarness() {
    val sender = net.treasury("sender");
    val init = ContractState {
        code: build("fm_debug"),
        data: createEmptyCell(),
    };
    val debugAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }))).toHaveSuccessfulDeploy({ to: debugAddress });

    return (sender, debugAddress);
}
"#;

const DESTROY_MESSAGES: &str = r#"
struct (0xF7000001) FmDestroyNow {
    queryId: uint64
}
"#;

const DESTROY_CONTRACT: &str = r#"
import "fm_destroy_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy FmDestroyNow.fromSlice(in.body);

    createMessage({
        bounce: false,
        value: ton("0"),
        dest: in.senderAddress,
    }).send(SEND_MODE_DESTROY | SEND_MODE_CARRY_ALL_BALANCE);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DESTROY_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/fm_destroy_messages"

fun deployFmDestroyHarness() {
    val sender = net.treasury("sender");
    val init = ContractState {
        code: build("fm_destroy"),
        data: createEmptyCell(),
    };
    val destroyAddress = AutoDeployAddress { stateInit: init }.calculateAddress();
    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }))).toHaveSuccessfulDeploy({ to: destroyAddress });

    return (sender, destroyAddress);
}
"#;

const LETTER_ROLLOVER_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
"#;

fn run_success_case(project: ProjectBuilder, snapshot_path: &str) {
    project
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn linear_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{LINEAR_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_linear_messages", LINEAR_MESSAGES)
        .contract("fm_linear_root", LINEAR_ROOT_CONTRACT)
        .contract("fm_linear_mid", LINEAR_MID_CONTRACT)
        .contract("fm_linear_sink", LINEAR_SINK_CONTRACT)
        .test_file("formatter_linear", &source)
}

fn linear_mismatch_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{LINEAR_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_linear_messages", LINEAR_MESSAGES)
        .contract("fm_linear_root", LINEAR_ROOT_OPCODE_MISMATCH_CONTRACT)
        .contract("fm_linear_mid", LINEAR_MID_CONTRACT)
        .contract("fm_linear_sink", LINEAR_SINK_CONTRACT)
        .test_file("formatter_linear", &source)
}

fn fanout_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{FANOUT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_fanout_messages", FANOUT_MESSAGES)
        .contract("fm_fanout_root", FANOUT_ROOT_CONTRACT)
        .contract("fm_fanout_left", FANOUT_LEFT_CONTRACT)
        .contract("fm_fanout_right", FANOUT_RIGHT_CONTRACT)
        .test_file("formatter_fanout", &source)
}

fn external_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{EXTERNAL_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_external_messages", EXTERNAL_MESSAGES)
        .contract("fm_external_root", EXTERNAL_CONTRACT)
        .test_file("formatter_external", &source)
}

fn external_throw_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{EXTERNAL_THROW_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_external_messages", EXTERNAL_MESSAGES)
        .contract("fm_external_throw", EXTERNAL_THROW_CONTRACT)
        .test_file("formatter_external_throw", &source)
}

fn bounce_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{BOUNCE_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_bounce_messages", BOUNCE_MESSAGES)
        .contract("fm_bounce_echo", BOUNCE_CONTRACT)
        .test_file("formatter_bounce", &source)
}

fn action_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{ACTION_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("fm_action_child", ACTION_CHILD_CONTRACT)
        .contract_with_deps(
            "fm_action_fail",
            ACTION_FAIL_CONTRACT,
            vec!["fm_action_child"],
        )
        .test_file("formatter_action", &source)
}

fn flags_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{FLAGS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_flags_messages", FLAGS_MESSAGES)
        .contract("fm_flags", FLAGS_CONTRACT)
        .test_file("formatter_flags", &source)
}

fn debug_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{DEBUG_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_debug_messages", DEBUG_MESSAGES)
        .contract("fm_debug", DEBUG_CONTRACT)
        .test_file("formatter_debug", &source)
}

fn destroy_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{DESTROY_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/fm_destroy_messages", DESTROY_MESSAGES)
        .contract("fm_destroy", DESTROY_CONTRACT)
        .test_file("formatter_destroy", &source)
}

fn letter_rollover_formatter_project(project_name: &str, test_body: &str) -> ProjectBuilder {
    let source = format!("{LETTER_ROLLOVER_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name).test_file("formatter_letter_rollover", &source)
}

#[test]
fn formatter_linear_chain_println_renders_nested_tree() {
    run_success_case(
        linear_formatter_project(
            "formatter-linear-chain-println",
            r#"
get fun `test-formatter-linear-chain-println`() {
    val (sender, rootAddress, midAddress, sinkAddress) = deployFmLinearHarness();
    val txs = sendFmLinear(sender, rootAddress, midAddress, sinkAddress, 101);

    expect(txs).toHaveLength(3);
    println(txs);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_linear_chain_println_nested_tree.stdout.txt",
    );
}

#[test]
fn formatter_linear_chain_println_renders_exit_code63_for_opcode_mismatch() {
    run_success_case(
        linear_mismatch_formatter_project(
            "formatter-linear-chain-println-exit-code63-opcode-mismatch",
            r#"
get fun `test-formatter-linear-chain-println-exit-code63-opcode-mismatch`() {
    val (sender, rootAddress, midAddress, sinkAddress) = deployFmLinearHarness();
    val txs = sendFmLinear(sender, rootAddress, midAddress, sinkAddress, 102);

    expect(txs).toHaveLength(2);
    println(txs);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_linear_chain_println_exit_code63_opcode_mismatch.stdout.txt",
    );
}

#[test]
fn formatter_exit_code63_from_cell_mismatch_is_reported_in_test_body() {
    linear_formatter_project(
        "formatter-exit-code63-from-cell-mismatch-in-test-body",
        r#"
get fun `test-formatter-exit-code63-from-cell-mismatch-in-test-body`() {
    val mid = net.randomAddress("fm_mismatch_mid");
    val sink = net.randomAddress("fm_mismatch_sink");
    val wrongCell = FmRoute {
        queryId: 999,
        mid,
        sink,
    }.toCell();

    FmRelay.fromCell(wrongCell);
}
"#,
    )
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_contains("exit_code=63")
    .assert_snapshot_matches(
        "integration/snapshots/formatter/formatter_exit_code63_from_cell_mismatch_in_test_body.stdout.txt",
    );
}

#[test]
fn formatter_exit_code63_from_cell_mismatch_in_test_body_with_backtrace_full() {
    linear_formatter_project(
        "formatter-exit-code63-from-cell-mismatch-in-test-body-with-backtrace-full",
        r#"
get fun `test-formatter-exit-code63-from-cell-mismatch-in-test-body-with-backtrace-full`() {
    val mid = net.randomAddress("fm_mismatch_mid");
    val sink = net.randomAddress("fm_mismatch_sink");
    val wrongCell = FmRoute {
        queryId: 1001,
        mid,
        sink,
    }.toCell();

    FmRelay.fromCell(wrongCell);
}
"#,
    )
    .build()
    .acton()
    .test()
    .with_backtrace("full")
    .run()
    .failure()
    .assert_failed(1)
    .assert_contains("exit_code=63")
    .assert_snapshot_matches(
        "integration/snapshots/formatter/formatter_exit_code63_from_cell_mismatch_in_test_body_with_backtrace_full.stdout.txt",
    );
}

#[test]
fn formatter_fanout_chain_println_renders_sibling_branches() {
    run_success_case(
        fanout_formatter_project(
            "formatter-fanout-println",
            r#"
get fun `test-formatter-fanout-chain-println`() {
    val (sender, rootAddress, leftAddress, rightAddress) = deployFmFanoutHarness();
    val txs = sendFmFanout(sender, rootAddress, leftAddress, rightAddress, 202);

    expect(txs).toHaveLength(3);
    println(txs);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_fanout_chain_println_sibling_branches.stdout.txt",
    );
}

#[test]
fn formatter_external_out_println_renders_none_and_external_destinations() {
    run_success_case(
        external_formatter_project(
            "formatter-external-out-println-destinations",
            r#"
get fun `test-formatter-external-out-println-destinations`() {
    val extAddress = deployFmExternalHarness();
    val txs = net.sendExternal(
        createExternalMessage(
            extAddress,
            FmExternalTrigger { queryId: 303 },
            null,
            fmExternalAddress(0x0A0B0C0D),
        ),
    );

    expect(txs!).toHaveLength(1);
    println(txs);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_external_out_println_destinations.stdout.txt",
    );
}

#[test]
fn formatter_ext_in_exit_code_with_backtrace_full_println_renders_backtrace() {
    external_throw_formatter_project(
        "formatter-ext-in-exit-code-with-backtrace-full",
        r#"
get fun `test-formatter-ext-in-exit-code-with-backtrace-full`() {
    val extAddress = deployFmExternalThrowHarness();
    val txs = net.sendExternal(
        createExternalMessage(
            extAddress,
            FmExternalTrigger { queryId: 404 },
            null,
            fmExternalAddress(0x0F0E0D0C),
        ),
    );

    expect(txs!).toHaveLength(1);
    println(txs);
}
"#,
    )
    .build()
    .acton()
    .test()
    .with_backtrace("full")
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/formatter/formatter_ext_in_exit_code_with_backtrace_full.stdout.txt",
    );
}

#[test]
fn formatter_println_renders_bounced_and_compute_skipped_transactions() {
    run_success_case(
        bounce_formatter_project(
            "formatter-println-bounced-compute-skipped",
            r#"
get fun `test-formatter-println-bounced-and-compute-skipped`() {
    val (sender, echoAddress) = deployFmBounceHarness();

    val bounced = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: echoAddress,
            body: FmBouncePing {
                queryId: 401,
            },
        }).bounced(),
    );

    val skipped = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: address("0:0000000000000000000000000000000000000000000000000000000000000BAD"),
            body: FmBouncePing {
                queryId: 402,
            },
        }),
    );

    expect(bounced).toHaveLength(1);
    expect(skipped).toHaveLength(1);
    println(bounced);
    println(skipped);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_multi_root_println_bounced_compute_skipped.stdout.txt",
    );
}

#[test]
fn formatter_multi_root_println_renders_independent_internal_chains() {
    run_success_case(
        linear_formatter_project(
            "formatter-multi-root-println-independent-internal-chains",
            r#"
get fun `test-formatter-multi-root-println-independent-internal-chains`() {
    val (sender, rootAddress, midAddress, sinkAddress) = deployFmLinearHarness();
    val first = sendFmLinear(sender, rootAddress, midAddress, sinkAddress, 505);
    val second = sendFmLinear(sender, rootAddress, midAddress, sinkAddress, 506);

    var merged: SendResultList = SendResultList.createEmpty();
    var i = 0;
    while (i < first.size()) {
        merged.push(first.get(i));
        i += 1;
    }
    i = 0;
    while (i < second.size()) {
        merged.push(second.get(i));
        i += 1;
    }

    expect(merged.size()).toEqual(first.size() + second.size());
    println(merged);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_multi_root_println_independent_internal_chains.stdout.txt",
    );
}

#[test]
fn formatter_action_phase_failure_println_renders_retrace_and_backtrace_hint() {
    run_success_case(
        action_formatter_project(
            "formatter-action-failure-println-with-hint",
            r#"
get fun `test-formatter-action-phase-failure-println-with-hint`() {
    val sender = net.treasury("sender");
    val (init, actionAddress) = fmActionInit();
    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: {
                stateInit: init,
            },
        }),
    );

    expect(txs).toHaveLength(1);
    println(txs);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_action_phase_failure_println_with_hint.stdout.txt",
    );
}

#[test]
fn formatter_action_phase_failure_println_with_backtrace_full_renders_action_locations() {
    let source = format!(
        "{}\n{}\n",
        ACTION_IMPORTS,
        r#"
get fun `test-formatter-action-phase-failure-println-with-backtrace-full`() {
    val sender = net.treasury("sender");
    val (init, actionAddress) = fmActionInit();
    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: {
                stateInit: init,
            },
        }),
    );

    expect(txs).toHaveLength(1);
    println(txs);
}
"#
    );

    ProjectBuilder::new("formatter-action-failure-println-with-backtrace-full")
        .contract("fm_action_child", ACTION_CHILD_CONTRACT)
        .contract_with_deps(
            "fm_action_fail",
            ACTION_FAIL_CONTRACT,
            vec!["fm_action_child"],
        )
        .test_file("formatter_action", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/formatter/formatter_action_phase_failure_println_with_backtrace_full.stdout.txt",
        );
}

#[test]
fn formatter_flags_after_gas_println_renders_multiple_flag_variants() {
    run_success_case(
        flags_formatter_project(
            "formatter-flags-after-gas-variants",
            r#"
get fun `test-formatter-flags-after-gas-variants`() {
    val (sender, flagsAddress) = deployFmFlagsHarness();

    val okRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: flagsAddress,
            body: FmFlagsOk { queryId: 1 },
        }),
    );

    val throwRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: flagsAddress,
            body: FmFlagsThrow { queryId: 2 },
        }),
    );

    val actionFailRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: flagsAddress,
            body: FmFlagsActionFail { queryId: 3 },
        }),
    );

    val skippedRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: unknownFmFlagsAddress(),
            body: FmFlagsOk { queryId: 4 },
        }),
    );

    expect(okRes).toHaveLength(1);
    expect(throwRes).toHaveLength(1);
    expect(actionFailRes).toHaveLength(1);
    expect(skippedRes).toHaveLength(1);

    println(okRes);
    println(throwRes);
    println(actionFailRes);
    println(skippedRes);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_flags_after_gas_println_variants.stdout.txt",
    );
}

#[test]
fn formatter_exit_code_println_without_backtrace_full() {
    run_success_case(
        flags_formatter_project(
            "formatter-exit-code-without-backtrace-full",
            r#"
get fun `test-formatter-exit-code-without-backtrace-full`() {
    val (sender, flagsAddress) = deployFmFlagsHarness();

    val throwRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: flagsAddress,
            body: FmFlagsThrow { queryId: 55 },
        }),
    );

    expect(throwRes).toHaveLength(1);
    println(throwRes);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_exit_code_without_backtrace_full.stdout.txt",
    );
}

#[test]
fn formatter_exit_code_println_with_backtrace_full() {
    let source = format!(
        "{}\n{}\n",
        FLAGS_IMPORTS,
        r#"
get fun `test-formatter-exit-code-with-backtrace-full`() {
    val (sender, flagsAddress) = deployFmFlagsHarness();

    val throwRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: flagsAddress,
            body: FmFlagsThrow { queryId: 77 },
        }),
    );

    expect(throwRes).toHaveLength(1);
    println(throwRes);
}
"#
    );

    ProjectBuilder::new("formatter-exit-code-with-backtrace-full")
        .file("contracts/fm_flags_messages", FLAGS_MESSAGES)
        .contract("fm_flags", FLAGS_CONTRACT)
        .test_file("formatter_flags", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/formatter/formatter_exit_code_with_backtrace_full.stdout.txt",
        );
}

#[test]
fn formatter_orphan_chain_println_treats_missing_parent_as_root() {
    run_success_case(
        linear_formatter_project(
            "formatter-orphan-chain-println-missing-parent",
            r#"
get fun `test-formatter-orphan-chain-println-missing-parent`() {
    val (sender, rootAddress, midAddress, sinkAddress) = deployFmLinearHarness();
    val txs = sendFmLinear(sender, rootAddress, midAddress, sinkAddress, 909);
    expect(txs).toHaveLength(3);

    var orphaned: SendResultList = SendResultList.createEmpty();
    orphaned.push(txs.get(1));
    orphaned.push(txs.get(2));
    println(orphaned);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_orphan_chain_println_missing_parent.stdout.txt",
    );
}

#[test]
fn formatter_debug_logs_println_renders_debug_logs_block() {
    debug_formatter_project(
        "formatter-debug-logs-println",
        r#"
get fun `test-formatter-debug-logs-println`() {
    val (sender, debugAddress) = deployFmDebugHarness();

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: debugAddress,
            body: FmDebugPing { queryId: 1 },
        }),
    );

    expect(txs).toHaveLength(1);
    println(txs);
}
"#,
    )
    .build()
    .acton()
    .test()
    .with_backtrace("full")
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/formatter/formatter_debug_logs_println_block.stdout.txt",
    );
}

#[test]
fn formatter_account_destroyed_println_renders_destroyed_marker() {
    run_success_case(
        destroy_formatter_project(
            "formatter-account-destroyed-println",
            r#"
get fun `test-formatter-account-destroyed-println`() {
    val (sender, destroyAddress) = deployFmDestroyHarness();

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: destroyAddress,
            body: FmDestroyNow { queryId: 1 },
        }),
    );

    println(txs);
}
"#,
        ),
        "integration/snapshots/formatter/formatter_account_destroyed_println_marker.stdout.txt",
    );
}

#[test]
#[allow(clippy::format_in_format_args)]
fn formatter_contract_letters_rollover_after_z_println_uses_a1_and_b1() {
    let mut sends = String::new();
    for index in 1..=27 {
        sends.push_str(
            format!(
                r#"
    val tx{index} = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.2"),
            dest: address("0:{address_hex}"),
            body: beginCell().storeUint({index}, 32).endCell(),
        }}),
    );
    expect(tx{index}).toHaveLength(1);
    merged.push(tx{index}.get(0));
"#,
                address_hex = format!("{index:064x}"),
            )
            .as_str(),
        );
    }

    let body = format!(
        r#"
get fun `test-formatter-contract-letters-rollover-a1-b1`() {{
    val sender = net.treasury("sender");
    var merged: SendResultList = SendResultList.createEmpty();
{sends}
    expect(merged).toHaveLength(27);
    println(merged);
}}
"#,
    );

    letter_rollover_formatter_project("formatter-contract-letters-rollover-a1-b1", &body)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" A1")
        .assert_contains(" B1")
        .assert_not_contains(" A2")
        .assert_not_contains(" B2");
}

#[test]
fn formatter_format_send_msg_flags_covers_all_bits_and_regular() {
    assert_eq!(
        FormatterContext::format_send_msg_flags(SendMsgFlags::empty()),
        "REGULAR"
    );

    let all_flags = SendMsgFlags::PAY_FEE_SEPARATELY
        | SendMsgFlags::IGNORE_ERROR
        | SendMsgFlags::BOUNCE_ON_ERROR
        | SendMsgFlags::DELETE_IF_EMPTY
        | SendMsgFlags::WITH_REMAINING_BALANCE
        | SendMsgFlags::ALL_BALANCE;

    assert_eq!(
        FormatterContext::format_send_msg_flags(all_flags),
        "PAY_FEES_SEPARATELY | IGNORE_ERRORS | BOUNCE_ON_ACTION_FAIL | DESTROY | CARRY_ALL_REMAINING_MESSAGE_VALUE | CARRY_ALL_BALANCE"
    );
}

#[test]
fn formatter_format_reserve_currency_flags_covers_all_bits_and_exact_amount() {
    assert_eq!(
        FormatterContext::format_reserve_currency_flags(ReserveCurrencyFlags::empty()),
        "EXACT_AMOUNT"
    );

    let all_flags = ReserveCurrencyFlags::ALL_BUT
        | ReserveCurrencyFlags::IGNORE_ERROR
        | ReserveCurrencyFlags::WITH_ORIGINAL_BALANCE
        | ReserveCurrencyFlags::REVERSE
        | ReserveCurrencyFlags::BOUNCE_ON_ERROR;

    assert_eq!(
        FormatterContext::format_reserve_currency_flags(all_flags),
        "ALL_BUT_AMOUNT | AT_MOST | INCREASE_BY_ORIGINAL_BALANCE | NEGATE_AMOUNT | BOUNCE_ON_ACTION_FAIL"
    );
}
