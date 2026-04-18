use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const AM_MESSAGES: &str = r"
struct (0xA0C0A001) Ping {
    queryId: uint64
    amount: uint32
    target: address
}

struct (0xA0C0A002) Notify {
    queryId: uint64
    amount: uint32
}

struct (0xA0C0A003) Other {
    queryId: uint64
}
";

const AM_WORKER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy Ping.fromSlice(in.body);
    if (msg.amount == 0) {
        return;
    }

    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.target,
        body: Notify {
            queryId: msg.queryId,
            amount: msg.amount + 1,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const AM_RECEIVER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy Notify.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const AM_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/message"
import "../../lib/types/transaction"
import "../../lib/tlb/maybe"
import "../contracts/messages"

fun deployAmHarness() {
    val sender = testing.treasury("sender");

    val workerInit = ContractState {
        code: build("worker"),
        data: createEmptyCell(),
    };
    val workerAddress = AutoDeployAddress { stateInit: workerInit }.calculateAddress();

    val receiverInit = ContractState {
        code: build("receiver"),
        data: createEmptyCell(),
    };
    val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

    val deployWorker = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: workerInit,
        },
    });
    expect(net.send(sender.address, deployWorker)).toHaveSuccessfulDeploy({ to: workerAddress });

    val deployReceiver = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: receiverInit,
        },
    });
    expect(net.send(sender.address, deployReceiver)).toHaveSuccessfulDeploy({ to: receiverAddress });

    return (sender, workerAddress, receiverAddress);
}

fun sendPing(sender: Treasury, worker: address, receiver: address, queryId: uint64, amount: uint32): SendResultList {
    return net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.5"),
            dest: worker,
            body: Ping {
                queryId,
                amount,
                target: receiver,
            },
        }),
    );
}
"#;

fn with_source(test_body: &str) -> String {
    format!("{AM_IMPORTS}\n{test_body}\n")
}

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = with_source(test_body);
    ProjectBuilder::new(project_name)
        .file("contracts/messages", AM_MESSAGES)
        .contract("worker", AM_WORKER_CONTRACT)
        .contract("receiver", AM_RECEIVER_CONTRACT)
        .test_file("transaction_helpers", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints() {
    run_success_case(
        "am-stdlib-transaction-load-body-and-load-in-msg",
        r"
get fun `test am transaction load body and load in msg`() {
    val (sender, workerAddress, receiverAddress) = deployAmHarness();
    val txs = sendPing(sender, workerAddress, receiverAddress, 11, 7);

    val tx = txs.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;

    val body = tx.loadBody<Ping>();
    expect(body.queryId).toEqual(11);
    expect(body.amount).toEqual(7);
    expect(body.target).toEqual(receiverAddress);

    val inMsg = tx.loadInMsg<Ping>();
    val inBody = inMsg.loadBody();
    expect(inBody).toEqual(body);
    expect(inMsg.info.src).toEqual(sender.address as any_address);
    expect(inMsg.info.dest).toEqual(workerAddress);
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints.stdout.txt",
    );
}

#[test]
fn transaction_load_body_reports_exit_code63_for_mismatched_message_type() {
    run_success_case(
        "am-stdlib-transaction-load-body-mismatch",
        r"
get fun `test am transaction load body mismatch`() {
    val (sender, workerAddress, receiverAddress) = deployAmHarness();
    val txs = sendPing(sender, workerAddress, receiverAddress, 21, 9);
    val tx = txs.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;

    expectToEndWithExitCode(63);
    tx.loadBody<Other>();
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_load_body_reports_exit_code63_for_mismatched_message_type.stdout.txt",
    );
}

#[test]
fn transaction_load_in_msg_reports_exit_code63_for_mismatched_message_type() {
    run_success_case(
        "am-stdlib-transaction-load-in-msg-mismatch",
        r"
get fun `test am transaction load in msg mismatch`() {
    val (sender, workerAddress, receiverAddress) = deployAmHarness();
    val txs = sendPing(sender, workerAddress, receiverAddress, 31, 5);
    val tx = txs.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;

    expectToEndWithExitCode(63);
    tx.loadInMsg<Other>().loadBody();
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_load_in_msg_reports_exit_code63_for_mismatched_message_type.stdout.txt",
    );
}

#[test]
fn transaction_get_used_gas_matches_send_result_for_root_and_child_transactions() {
    run_success_case(
        "am-stdlib-transaction-get-used-gas-for-root-and-child",
        r"
get fun `test am transaction get used gas for root and child`() {
    val (sender, workerAddress, receiverAddress) = deployAmHarness();
    val txs = sendPing(sender, workerAddress, receiverAddress, 41, 10);

    expect(txs).toHaveLength(2);

    val rootTx = txs.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;
    val childTx = txs.findTransaction<Notify>({
        from: workerAddress,
        to: receiverAddress,
        success: true,
    })!;

    expect(rootTx.getUsedGas()).toEqual(txs.at(0).gasUsed);
    expect(childTx.getUsedGas()).toEqual(txs.at(1).gasUsed);
    expect(rootTx.getUsedGas()).toBeGreater(255);
    expect(childTx.getUsedGas()).toBeGreater(255);
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_get_used_gas_matches_send_result_for_root_and_child_transactions.stdout.txt",
    );
}

#[test]
fn transaction_get_used_gas_reports_skipped_compute_phase_for_undeployed_destination() {
    run_success_case(
        "am-stdlib-transaction-get-used-gas-skipped-compute",
        r#"
get fun `test am transaction get used gas skipped compute`() {
    val sender = testing.treasury("sender");
    val undeployed = randomAddress("am_skip_compute_target");

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: undeployed,
            body: Ping {
                queryId: 51,
                amount: 1,
                target: undeployed,
            },
        }),
    );

    val tx = txs.findTransaction<Ping>({
        from: sender.address,
        to: undeployed,
    })!;

    expectToEndWithExitCode(567);
    tx.getUsedGas();
}
"#,
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_get_used_gas_reports_skipped_compute_phase_for_undeployed_destination.stdout.txt",
    );
}

#[test]
fn transaction_get_action_fee_matches_transaction_description_and_none_branch() {
    run_success_case(
        "am-stdlib-transaction-get-action-fee-match-and-none",
        r"
fun expectedActionFee(tx: TlbTransaction): coins? {
    val descr = tx.description.load();
    if (descr is TlbTransOrd) {
        if (descr.action is TlbNone) {
            return null;
        }
        return descr.action.unwrap().load().totalActionFees.unwrapOr(null);
    }
    if (descr is TlbTransTickTock) {
        if (descr.action is TlbNone) {
            return null;
        }
        return descr.action.unwrap().load().totalActionFees.unwrapOr(null);
    }
    return null;
}

get fun `test am transaction get action fee match and none`() {
    val (sender, workerAddress, receiverAddress) = deployAmHarness();

    val withAction = sendPing(sender, workerAddress, receiverAddress, 61, 4);
    val withActionTx = withAction.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;
    expect(withActionTx.getActionFee()).toEqual(expectedActionFee(withActionTx));

    val noAction = sendPing(sender, workerAddress, receiverAddress, 62, 0);
    val noActionTx = noAction.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;
    expect(noActionTx.getActionFee()).toEqual(expectedActionFee(noActionTx));
    expect(noActionTx.getActionFee()).toBeNull();
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_get_action_fee_matches_transaction_description_and_none_branch.stdout.txt",
    );
}

#[test]
fn transaction_get_account_address_defaults_to_basechain_and_supports_masterchain_override() {
    run_success_case(
        "am-stdlib-transaction-get-account-address-workchain-override",
        r"
get fun `test am transaction get account address workchain override`() {
    val (sender, workerAddress, receiverAddress) = deployAmHarness();
    val txs = sendPing(sender, workerAddress, receiverAddress, 71, 3);

    val tx = txs.findTransaction<Ping>({
        from: sender.address,
        to: workerAddress,
    })!;

    expect(tx.getAccountAddress()).toEqual(workerAddress);
    expect(tx.getAccountAddress(BASECHAIN)).toEqual(workerAddress);
    expect(tx.getAccountAddress(MASTERCHAIN)).toNotEqual(workerAddress);
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_get_account_address_defaults_to_basechain_and_supports_masterchain_override.stdout.txt",
    );
}

#[test]
fn transaction_varuint7_roundtrip_for_storage_used_large_values_bug() {
    run_success_case(
        "am-stdlib-transaction-varuint7-roundtrip-bug",
        r"
get fun `test am transaction varuint7 roundtrip bug`() {
    val original = TlbStorageUsed {
        cells: 1024,
        bits: 511,
    };
    val decoded = TlbStorageUsed.fromCell(original.toCell());

    expect(decoded.cells).toEqual(original.cells);
    expect(decoded.bits).toEqual(original.bits);
}
",
        "integration/snapshots/test-runner/transaction_load_body_and_load_in_msg_extract_typed_payload_and_endpoints/transaction_varuint7_roundtrip_for_storage_used_large_values_bug.stdout.txt",
    );
}
