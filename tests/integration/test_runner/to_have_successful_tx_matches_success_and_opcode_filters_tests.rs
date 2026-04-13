use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const AE_MESSAGES: &str = r"
struct (0xAE110001) Ping {
    queryId: uint64
}

struct (0xAE110002) BounceNotice {
    queryId: uint64
}

struct (0xAE110003) ExternalNotice {
    queryId: uint64
}
";

const AE_CONTRACT: &str = r#"
import "@stdlib/gas-payments"
import "messages"

const ERR_FAIL = 701;
const ERR_BOUNCE = 777;

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy Ping.fromSlice(in.body);

    if (msg.queryId == 10) {
        throw ERR_FAIL;
    }

    if (msg.queryId == 20) {
        reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
        return;
    }

    if (msg.queryId == 30) {
        createExternalLogMessage({
            dest: createAddressNone(),
            body: ExternalNotice { queryId: msg.queryId },
        }).send(SEND_MODE_REGULAR);
        return;
    }

    if (msg.queryId == 40) {
        createMessage({
            bounce: false,
            value: ton("0.1"),
            dest: in.senderAddress,
            body: BounceNotice { queryId: msg.queryId },
        }).send(SEND_MODE_PAY_FEES_SEPARATELY);
        return;
    }
}

fun onBouncedMessage(_: InMessageBounced) {
    throw ERR_BOUNCE;
}
"#;

const TEST_PRELUDE: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/message"
import "../contracts/messages"

const ERR_FAIL = 701;
const ERR_BOUNCE = 777;
const ACTION_FAIL_EXIT = 37;

struct Harness {
    address: address
    init: ContractState
}

fun Harness.create() {
    val init = ContractState {
        code: build("harness"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Harness { address, init };
}

fun deployHarness() {
    val sender = net.treasury("sender");
    val harness = Harness.create();

    val deployMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: harness.init,
        },
    });
    val deployRes = net.send(sender.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({ to: harness.address });

    return (sender, harness, deployRes);
}

fun sendPing(sender: Treasury, harness: Harness, queryId: uint64): SendResultList {
    val msg = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: harness.address,
        body: Ping { queryId },
    });
    return net.send(sender.address, msg);
}
"#;

fn with_prelude(test_body: &str) -> String {
    format!("{TEST_PRELUDE}\n{test_body}\n")
}

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .file("contracts/messages", AE_MESSAGES)
        .contract("harness", AE_CONTRACT)
        .test_file("tx_expect", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_failure_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .file("contracts/messages", AE_MESSAGES)
        .contract("harness", AE_CONTRACT)
        .test_file("tx_expect", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn to_have_successful_tx_matches_success_and_opcode_filters() {
    run_success_case(
        "ae-stdlib-successful-tx-search-filters",
        r"
get fun `test ae successful tx search filters`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 1);

    expect(res).toHaveSuccessfulTx<Ping>({
        from: sender.address,
        to: harness.address,
    });
    expect(res).toHaveTx<Ping>({
        from: sender.address,
        to: harness.address,
        success: true,
        bounce: false,
        exitCode: 0,
    });
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_have_successful_tx_matches_success_and_opcode_filters.stdout.txt",
    );
}

#[test]
fn to_have_failed_tx_matches_compute_exit_code_filter() {
    run_success_case(
        "ae-stdlib-failed-tx-compute-exit-filter",
        r"
get fun `test ae failed tx compute exit filter`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 10);

    expect(res).toHaveFailedTx<Ping>({
        from: sender.address,
        to: harness.address,
        exitCode: ERR_FAIL,
    });
    expect(res).toHaveTx<Ping>({
        from: sender.address,
        to: harness.address,
        success: false,
        exitCode: ERR_FAIL,
    });
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_have_failed_tx_matches_compute_exit_code_filter.stdout.txt",
    );
}

#[test]
fn to_have_tx_matches_action_exit_code_filter() {
    run_success_case(
        "ae-stdlib-action-exit-code-filter",
        r"
get fun `test-ae-action-exit-code-filter`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 20);

    expect(res).toHaveTx<Ping>({
        from: sender.address,
        to: harness.address,
        exitCode: 0,
        actionExitCode: ACTION_FAIL_EXIT,
        success: false,
    });
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_have_tx_matches_action_exit_code_filter.stdout.txt",
    );
}

#[test]
fn to_have_bounced_tx_matches_bounced_opcode_and_exit_code_filters() {
    run_success_case(
        "ae-stdlib-bounced-tx-opcode-filter",
        r#"
get fun `test ae bounced tx opcode filter`() {
    val (sender, harness, _) = deployHarness();

    val trigger = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: harness.address,
        body: Ping { queryId: 40 },
    });
    val sendSingleRes = net.sendSingle(sender.address, trigger);
    expect(sendSingleRes.outMessages.size()).toEqual(1);

    val noticeBody = sendSingleRes.outMessages.at<BounceNotice>(0).loadBody().toCell();
    val bouncedBody = beginCell()
        .storeUint(0xFFFFFFFF, 32)
        .storeSlice(noticeBody.beginParse())
        .endCell();

    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.3"),
        dest: harness.address,
        body: bouncedBody,
    }).bounced();

    val bouncedRes = net.send(sender.address, bouncedMsg);
    expect(bouncedRes).toHaveBouncedTx<BounceNotice>({
        from: sender.address,
        to: harness.address,
    });
    expect(bouncedRes).toHaveFailedTx<BounceNotice>({
        from: sender.address,
        to: harness.address,
        bounced: true,
        exitCode: ERR_BOUNCE,
    });
}
"#,
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_have_bounced_tx_matches_bounced_opcode_and_exit_code_filters.stdout.txt",
    );
}

#[test]
fn to_emit_external_message_matches_emitted_type() {
    run_success_case(
        "ae-stdlib-emit-external-message-positive",
        r"
get fun `test ae emit external message positive`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 30);

    expect(res).toEmitExternalMessage<ExternalNotice>();
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_emit_external_message_matches_emitted_type.stdout.txt",
    );
}

#[test]
fn to_emit_external_message_fails_for_missing_external_output() {
    run_failure_case(
        "ae-stdlib-emit-external-message-missing",
        r"
get fun `test ae emit external message missing`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 1);

    expect(res).toEmitExternalMessage<ExternalNotice>();
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_emit_external_message_fails_for_missing_external_output.stdout.txt",
    );
}

#[test]
fn to_have_failed_tx_requires_non_null_exit_code_param() {
    run_failure_case(
        "ae-stdlib-failed-tx-missing-exit-code",
        r"
get fun `test-ae-failed-tx-missing-exit-code`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 10);

    expect(res).toHaveFailedTx<Ping>({
        from: sender.address,
        to: harness.address,
    });
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_have_failed_tx_requires_non_null_exit_code_param.stdout.txt",
    );
}

#[test]
fn deploy_filter_distinguishes_deploy_and_non_deploy_transactions() {
    run_success_case(
        "ae-stdlib-deploy-filter",
        r"
get fun `test ae deploy filter`() {
    val (sender, harness, deployRes) = deployHarness();

    expect(deployRes).toHaveTx({
        to: harness.address,
        deploy: true,
    });
    expect(deployRes).toNotHaveTx({
        to: harness.address,
        deploy: false,
    });

    val callRes = sendPing(sender, harness, 1);
    expect(callRes).toHaveTx<Ping>({
        from: sender.address,
        to: harness.address,
        deploy: false,
    });
    expect(callRes).toNotHaveTx<Ping>({
        from: sender.address,
        to: harness.address,
        deploy: true,
    });
}
",
        "integration/snapshots/test-runner/to_have_successful_tx_matches_success_and_opcode_filters/to_not_have_tx_with_deploy_false_is_ignored_bug.stdout.txt",
    );
}
