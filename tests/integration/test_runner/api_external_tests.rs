use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{append_custom_network, spawn_toncenter_v2_mock_with_capture};

const EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"

struct (0x70000001) TriggerExternal {
    id: uint32
}

struct (0x70000002) ExternalAlpha {
    value: uint32
}

struct (0x70000003) ExternalBeta {
    value: uint32
}

fun externalDest() {
    return any_address.fromCell(
        beginCell()
            .storeUint(0b01, 2)
            .storeUint(16, 9)
            .storeUint(0xBEEF, 16)
            .endCell(),
    );
}

fun onExternalMessage() {
    acceptExternalMessage();

    createExternalLogMessage({
        dest: createAddressNone(),
        body: ExternalAlpha { value: 111 },
    }).send(SEND_MODE_REGULAR);

    createExternalLogMessage({
        dest: externalDest(),
        body: ExternalBeta { value: 222 },
    }).send(SEND_MODE_REGULAR);
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const REJECTING_EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"

fun onExternalMessage() {
    throw 10;
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ABI_REJECTING_EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"

const ERR_EXTERNAL_REJECTED = 701;

fun onExternalMessage() {
    throw ERR_EXTERNAL_REJECTED;
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const EXTERNAL_API_TEST_PRELUDE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/types/message"
import "../../lib/types/transaction"

struct (0x70000001) TriggerExternal {
    id: uint32
}

struct (0x70000002) ExternalAlpha {
    value: uint32
}

struct (0x70000003) ExternalBeta {
    value: uint32
}

struct ExternalHarness {
    address: address
    init: ContractState
}

fun ExternalHarness.create() {
    val init = ContractState {
        code: build("external"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return ExternalHarness { address, init };
}

fun deployHarness() {
    val harness = ExternalHarness.create();
    val deployer = testing.treasury("deployer");
    val deployRes = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: {
                stateInit: harness.init,
            },
        }),
    );
    expect(deployRes).toHaveSuccessfulDeploy({ to: harness.address });
    return (harness, deployer);
}

fun externalDest() {
    return any_address.fromCell(
        beginCell()
            .storeUint(0b01, 2)
            .storeUint(16, 9)
            .storeUint(0xBEEF, 16)
            .endCell(),
    );
}
"#;

fn with_prelude(test_body: &str) -> String {
    format!("{EXTERNAL_API_TEST_PRELUDE}\n{test_body}")
}

fn run_success_case(project_name: &str, test_body: &str, test_name: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(test_name);
}

fn run_failure_case(project_name: &str, test_body: &str, test_name: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains(test_name);
}

fn run_failure_snapshot_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_snapshot_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn send_external_collects_external_messages_with_deterministic_order() {
    run_success_case(
        "o-lib-api-send-external-collects-externals",
        r"
get fun `test send external collects externals`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 1 }),
    ).unwrap();

    expect(txs).toHaveLength(1);
    val tx = txs.at(0);
    expect(tx.externals).toHaveLength(2);

    val alpha = tx.externals.at<ExternalAlpha>(0);
    expect(alpha.info.src).toEqual(harness.address);
    expect(alpha.info.dest).toEqual(createAddressNone());
    expect(alpha.loadBody()).toEqual(ExternalAlpha { value: 111 });

    val beta = tx.externals.at<ExternalBeta>(1);
    expect(beta.info.src).toEqual(harness.address);
    expect(beta.info.dest).toEqual(externalDest());
    expect(beta.loadBody()).toEqual(ExternalBeta { value: 222 });
}
",
        "send external collects externals",
    );
}

#[test]
fn send_external_stays_local_when_broadcast_flag_enabled_in_test_runner() {
    run_snapshot_case(
        "o-lib-api-send-external-broadcast-flag-local",
        r"
get fun `test send external stays local with broadcast flag in tests`() {
    val (harness, _) = deployHarness();

    net.enableBroadcast();
    expect(net.isBroadcasting()).toBeTrue();

    val txs = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 9 }),
    ).unwrap();

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();
    expect(tx.loadBody<TriggerExternal>()).toEqual(TriggerExternal { id: 9 });
    expect(txs.at(0).externals).toHaveLength(2);
}
",
        "integration/snapshots/test-runner/api_external/send_external_stays_local_when_broadcast_flag_enabled_in_test_runner.stdout.txt",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn broadcast_wait_helpers_stay_local_in_test_runner() {
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock_with_capture(vec![]);
    let source = with_prelude(
        r"
get fun `test broadcast wait helpers stay local in tests`() {
    val (harness, _) = deployHarness();

    net.enableBroadcast();
    val txs = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 10 }),
    ).unwrap();

    expect(txs.waitForFirstTransaction(true, 1, 1)).toBeNull();
    expect(txs.waitForTrace(true, 1, 1)).toBeNull();
}
",
    );

    let project = ProjectBuilder::new("o-lib-api-broadcast-wait-helpers-local")
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build();
    append_custom_network(project.path(), "mock-wait-unused", &mock_url);

    project
        .acton()
        .test()
        .arg("--fork-net")
        .arg("custom:mock-wait-unused")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/broadcast_wait_helpers_stay_local_in_test_runner.stdout.txt",
        );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(captured.len(), 0, "acton test must not poll toncenter");
}

#[test]
fn transaction_load_body_decodes_external_inbound_body() {
    run_snapshot_case(
        "o-lib-api-transaction-load-body-external-in",
        r"
get fun `test transaction load body decodes external inbound body`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 7 }),
    ).unwrap();

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();
    val body = tx.loadBody<TriggerExternal>();
    expect(body).toEqual(TriggerExternal { id: 7 });
}
",
        "integration/snapshots/test-runner/api_external/transaction_load_body_decodes_external_inbound_body.stdout.txt",
    );
}

#[test]
fn transaction_load_in_msg_decodes_external_inbound_message() {
    run_snapshot_case(
        "o-lib-api-transaction-load-in-msg-external-in",
        r"
get fun `test transaction load in msg decodes external inbound message`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 8 }),
    ).unwrap();

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();
    val inMsg = tx.loadInMsg<TriggerExternal>();
    expect(inMsg.loadBody()).toEqual(TriggerExternal { id: 8 });
    expect(inMsg.info is TlbExternalInMessageInfo).toBeTrue();
    if (inMsg.info is TlbExternalInMessageInfo) {
        expect(inMsg.info.dest).toEqual(harness.address);
        expect(inMsg.info.importFee).toBeGreater(0);
    }
}
",
        "integration/snapshots/test-runner/api_external/transaction_load_in_msg_decodes_external_inbound_message.stdout.txt",
    );
}

#[test]
fn create_external_message_accepts_explicit_external_src() {
    run_success_case(
        "o-lib-api-create-external-explicit-src",
        r"
get fun `test create external message with external src`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        net.createExternalMessage(
            harness.address,
            TriggerExternal { id: 2 },
            null,
            externalDest(),
        ),
    ).unwrap();

    expect(txs).toHaveLength(1);
    expect(txs.at(0).externals).toHaveLength(2);

    val first = txs.at(0).externals.at<ExternalAlpha>(0);
    expect(first.loadBody()).toEqual(ExternalAlpha { value: 111 });
}
",
        "create external message with external src",
    );
}

#[test]
fn send_external_is_repeatable_for_same_contract() {
    run_success_case(
        "o-lib-api-send-external-repeatable",
        r"
get fun `test send external repeatable`() {
    val (harness, _) = deployHarness();

    val first = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 3 }),
    ).unwrap();
    val second = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 4 }),
    ).unwrap();

    expect(first).toHaveLength(1);
    expect(second).toHaveLength(1);
    expect(first.at(0).externals).toHaveLength(2);
    expect(second.at(0).externals).toHaveLength(2);

    val firstAlpha = first.at(0).externals.at<ExternalAlpha>(0).loadBody();
    val secondAlpha = second.at(0).externals.at<ExternalAlpha>(0).loadBody();
    expect(firstAlpha).toEqual(ExternalAlpha { value: 111 });
    expect(secondAlpha).toEqual(ExternalAlpha { value: 111 });
}
",
        "send external repeatable",
    );
}

#[test]
fn send_external_returns_not_accepted_result_when_deployed_contract_has_too_low_balance() {
    run_success_case(
        "o-lib-api-send-external-low-balance-rejected",
        r#"
get fun `test send external low balance rejected`() {
    val (harness, _) = deployHarness();

    val tinyBalanceSource = randomAddress("o_external_tiny_balance_source");
    testing.topUp(tinyBalanceSource, 1);

    val harnessShard = testing.getShardAccount(harness.address);
    val tinyBalanceShard = testing.getShardAccount(tinyBalanceSource);

    expect(harnessShard).toBeNotNull();
    expect(tinyBalanceShard).toBeNotNull();

    val harnessAcc = harnessShard!.account.load();
    val tinyBalanceAcc = tinyBalanceShard!.account.load();

    expect(harnessAcc is TlbAccountInfo).toBeTrue();
    expect(tinyBalanceAcc is TlbAccountInfo).toBeTrue();

    if (harnessAcc is TlbAccountInfo && tinyBalanceAcc is TlbAccountInfo) {
        val lowBalanceAcc = TlbAccountInfo {
            addr: harness.address,
            storageStat: harnessAcc.storageStat,
            storage: {
                lastTransLt: harnessAcc.storage.lastTransLt,
                balance: tinyBalanceAcc.storage.balance,
                state: harnessAcc.storage.state,
            },
        };
        var lowBalanceShard = harnessShard!;
        lowBalanceShard.account = (lowBalanceAcc as TlbAccount).toCell();
        testing.setShardAccount(harness.address, lowBalanceShard);
    }

    expect(testing.getAccountBalance(harness.address)).toEqual(1);

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 6 }),
    );
    expect(result).toBeNotAccepted();
    expect(result.transactions).toBeNull();
    expect(result.error).toBeNotNull();
    expect(result.error!.externalNotAccepted).toBeTrue();
    expect(result.error!.message).toNotEqual("");
    expect(result.error!.missingLibraries).toHaveLength(0);
}
"#,
        "send external low balance rejected",
    );
}

#[test]
fn external_send_result_helpers_cover_accepted_trace() {
    run_snapshot_case(
        "o-lib-api-external-send-result-helpers-accepted",
        r#"
get fun `test external send result helpers cover accepted trace`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 11 }),
    );

    expect(result).toBeAccepted();
    expect(result.isAccepted()).toBeTrue();
    expect(result.error).toBeNull();
    expect(result.transactions).toBeNotNull();

    result.giveName("external-helper-trace");

    val txs = result.unwrap();
    expect(txs).toHaveLength(1);
    expect(result.at(0).lt).toEqual(txs.at(0).lt);

    val first = result.waitForFirstTransaction(true, 1, 1);
    expect(first).toBeNotNull();
    expect(first!.lt).toEqual(result.at(0).lt);

    val trace = result.waitForTrace(true, 1, 1);
    expect(trace).toBeNotNull();
    expect(trace!.at(0).lt).toEqual(result.at(0).lt);

    val found = result.findExternalOutMessage<ExternalAlpha>({
        from: harness.address,
        to: createAddressNone(),
    });
    expect(found).toBeNotNull();
    expect(found!.loadBody()).toEqual(ExternalAlpha { value: 111 });
}
"#,
        "integration/snapshots/test-runner/api_external/external_send_result_helpers_cover_accepted_trace.stdout.txt",
    );
}

#[test]
fn external_send_result_helpers_cover_rejected_trace() {
    let source = with_prelude(
        r"
get fun `test external send result helpers cover rejected trace`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, createEmptyCell()),
    );

    expect(result).toBeNotAccepted();
    expect(result).toEndWithExitCode(10);
    expect(result.isAccepted()).toBeFalse();
    expect(result.transactions).toBeNull();
    expect(result.error).toBeNotNull();
    expect(result.error!.externalNotAccepted).toBeTrue();
    expect(result.waitForFirstTransaction(true, 1, 1)).toBeNull();
    expect(result.waitForTrace(true, 1, 1)).toBeNull();
    expect(result.findExternalOutMessage<ExternalAlpha>({})).toBeNull();
}
",
    );

    ProjectBuilder::new("o-lib-api-external-send-result-helpers-rejected")
        .contract("external", REJECTING_EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/external_send_result_helpers_cover_rejected_trace.stdout.txt",
        );
}

#[test]
fn create_external_message_rejects_internal_src() {
    run_failure_case(
        "o-lib-api-create-external-rejects-internal-src",
        r"
get fun `test create external message rejects internal src`() {
    val (harness, deployer) = deployHarness();

    net.createExternalMessage(
        harness.address,
        TriggerExternal { id: 5 },
        null,
        deployer.address,
    );
}
",
        "create external message rejects internal src",
    );
}

#[test]
fn external_send_result_unwrap_reports_external_vm_exit_code() {
    let source = with_prelude(
        r"
get fun `test external send result unwrap reports external vm exit code`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, createEmptyCell()),
    );

    result.unwrap();
}
",
    );

    ProjectBuilder::new("o-lib-api-external-send-result-unwrap-exit-code")
        .contract("external", REJECTING_EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/external_send_result_unwrap_reports_external_vm_exit_code.stdout.txt",
        );
}

#[test]
fn to_be_accepted_reports_external_vm_exit_code() {
    let source = with_prelude(
        r"
get fun `test toBeAccepted reports external vm exit code`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, createEmptyCell()),
    );

    expect(result).toBeAccepted();
}
",
    );

    ProjectBuilder::new("o-lib-api-send-external-to-be-accepted-exit-code")
        .contract("external", REJECTING_EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_be_accepted_reports_external_vm_exit_code.stdout.txt",
        );
}

#[test]
fn to_be_not_accepted_reports_accepted_external_result() {
    run_failure_snapshot_case(
        "o-lib-api-send-external-to-be-not-accepted-accepted",
        r"
get fun `test toBeNotAccepted reports accepted external result`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 12 }),
    );

    expect(result).toBeNotAccepted();
}
",
        "integration/snapshots/test-runner/api_external/to_be_not_accepted_reports_accepted_external_result.stdout.txt",
    );
}

#[test]
fn to_have_external_vm_exit_code_reports_mismatch() {
    let source = with_prelude(
        r"
get fun `test toEndWithExitCode reports mismatch`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, createEmptyCell()),
    );

    expect(result).toEndWithExitCode(11);
}
",
    );

    ProjectBuilder::new("o-lib-api-send-external-vm-exit-code-mismatch")
        .contract("external", REJECTING_EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_have_external_vm_exit_code_reports_mismatch.stdout.txt",
        );
}

#[test]
fn to_have_external_vm_exit_code_reports_accepted_external_result() {
    run_failure_snapshot_case(
        "o-lib-api-send-external-vm-exit-code-accepted",
        r"
get fun `test toEndWithExitCode reports accepted external result`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 13 }),
    );

    expect(result).toEndWithExitCode(10);
}
",
        "integration/snapshots/test-runner/api_external/to_have_external_vm_exit_code_reports_accepted_external_result.stdout.txt",
    );
}

#[test]
fn to_be_accepted_without_backtrace_suggests_backtrace_full() {
    let source = with_prelude(
        r"
get fun `test toBeAccepted suggests backtrace full`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, createEmptyCell()),
    );

    expect(result).toBeAccepted();
}
",
    );

    ProjectBuilder::new("o-lib-api-send-external-to-be-accepted-backtrace-hint")
        .contract("external", REJECTING_EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_be_accepted_without_backtrace_suggests_backtrace_full.stdout.txt",
        );
}

#[test]
fn to_be_accepted_reports_external_abi_exit_code_name() {
    let source = with_prelude(
        r"
get fun `test toBeAccepted reports external abi exit code name`() {
    val (harness, _) = deployHarness();

    val result = net.sendExternal(
        net.createExternalMessage(harness.address, createEmptyCell()),
    );

    expect(result).toBeAccepted();
}
",
    );

    ProjectBuilder::new("o-lib-api-send-external-to-be-accepted-abi-exit-code")
        .contract("external", ABI_REJECTING_EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_be_accepted_reports_external_abi_exit_code_name.stdout.txt",
        );
}

#[test]
fn to_be_accepted_reports_external_error_without_vm_exit_code() {
    let source = with_prelude(
        r#"
get fun `test toBeAccepted reports external error without vm exit code`() {
    val (harness, _) = deployHarness();
    val missingLibraries: array<string> = [];
    val result = ExternalSendResult {
        transactions: null,
        destination: harness.address,
        error: ExternalSendError {
            message: "custom external failure without vm code",
            externalNotAccepted: true,
            vmExitCode: null,
            elapsedTimeNs: null,
            missingLibraries,
            diagnosticId: null,
        },
    };

    expect(result).toBeAccepted();
}
"#,
    );

    ProjectBuilder::new("o-lib-api-send-external-to-be-accepted-no-vm-code")
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_be_accepted_reports_external_error_without_vm_exit_code.stdout.txt",
        );
}

#[test]
fn to_be_accepted_reports_external_send_failure_status() {
    let source = with_prelude(
        r#"
get fun `test toBeAccepted reports external send failure status`() {
    val (harness, _) = deployHarness();
    val missingLibraries: array<string> = [];
    val result = ExternalSendResult {
        transactions: null,
        destination: harness.address,
        error: ExternalSendError {
            message: "external send failed before contract acceptance",
            externalNotAccepted: false,
            vmExitCode: null,
            elapsedTimeNs: null,
            missingLibraries,
            diagnosticId: null,
        },
    };

    expect(result).toBeAccepted();
}
"#,
    );

    ProjectBuilder::new("o-lib-api-send-external-to-be-accepted-send-failure-status")
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_be_accepted_reports_external_send_failure_status.stdout.txt",
        );
}

#[test]
fn to_be_accepted_reports_external_missing_libraries() {
    let source = with_prelude(
        r#"
get fun `test toBeAccepted reports external missing libraries`() {
    val (harness, _) = deployHarness();
    val missingLibraries: array<string> = ["lib-alpha", "lib-beta"];
    val result = ExternalSendResult {
        transactions: null,
        destination: harness.address,
        error: ExternalSendError {
            message: "external failed because libraries are unavailable",
            externalNotAccepted: true,
            vmExitCode: 41,
            elapsedTimeNs: null,
            missingLibraries,
            diagnosticId: null,
        },
    };

    expect(result).toBeAccepted();
}
"#,
    );

    ProjectBuilder::new("o-lib-api-send-external-to-be-accepted-missing-libraries")
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_external/to_be_accepted_reports_external_missing_libraries.stdout.txt",
        );
}

#[test]
fn find_external_out_message_has_generic_compilation_bug() {
    run_success_case(
        "o-lib-api-find-external-out-generic-bug",
        r"
get fun `test find external out message bug`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        net.createExternalMessage(harness.address, TriggerExternal { id: 5 }),
    ).unwrap();

    val found = txs.findExternalOutMessage<ExternalAlpha>({
        from: harness.address,
        to: createAddressNone(),
    });

    expect(found).toBeNotNull();
}
",
        "find external out message bug",
    );
}
