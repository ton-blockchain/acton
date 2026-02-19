//! Reserved integration test module for subagent CK.
//!
//! Ownership boundary for agent CK:
//! - tests/integration/test-runner/test_runner_stdlib_ck_transaction_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ck_transaction_tests/**
//! - tests/integration/testdata/test_std_agent_ck/**
//! - tests/support/test_std_agent_ck/** (optional)
//!
//! Required test name prefix:
//! - ck_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CK_MESSAGES: &str = r#"
struct (0xCC01CC01) CkPing {
    queryId: uint64
    amount: uint32
    target: address
}

struct (0xCC01CC02) CkNotify {
    queryId: uint64
    amount: uint32
}
"#;

const CK_WORKER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy CkPing.fromSlice(in.body);
    if (msg.amount == 0) {
        return;
    }

    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.target,
        body: CkNotify {
            queryId: msg.queryId,
            amount: msg.amount + 1,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CK_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/transaction"
import "../../lib/tlb/maybe"
import "../contracts/messages"

fun deployCkHarness() {
    val sender = net.treasury("sender");
    val workerInit = ContractState {
        code: build("worker"),
        data: createEmptyCell(),
    };
    val workerAddress = AutoDeployAddress {
        stateInit: workerInit,
    }.calculateAddress();

    val deployWorker = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: workerInit,
        },
    });
    expect(net.send(sender.address, deployWorker)).toHaveSuccessfulDeploy({ to: workerAddress });

    return (sender, workerAddress);
}

fun sendCkPing(sender: Treasury, workerAddress: address, queryId: uint64, amount: uint32): SendResultList {
    val target = net.randomAddress("ck_tick_tock_target");
    return net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.5"),
            dest: workerAddress,
            body: CkPing {
                queryId,
                amount,
                target,
            },
        }),
    );
}

fun defaultStoragePhase(): TrStoragePhase {
    return TrStoragePhase {
        storageFeesCollected: 0,
        storageFeesDue: None{},
        statusChange: AccUnchanged{},
    };
}

fun withDescription(tx: Transaction, descr: TransactionDescr): Transaction {
    val descrCell: Cell<TransactionDescr> = descr.toCell();
    return Transaction {
        accountAddr: tx.accountAddr,
        lt: tx.lt,
        prevTransHash: tx.prevTransHash,
        prevTransLt: tx.prevTransLt,
        now: tx.now,
        outmsgCnt: tx.outmsgCnt,
        origStatus: tx.origStatus,
        endStatus: tx.endStatus,
        messages: tx.messages,
        totalFees: tx.totalFees,
        stateUpdate: tx.stateUpdate,
        description: descrCell,
    };
}

fun asTickTock(tx: Transaction, isTock: bool, withoutAction: bool): Transaction {
    val descr = tx.description.load();
    if (descr is TransOrd) {
        var storagePh = defaultStoragePhase();
        if (descr.storagePh is None) {
            storagePh = defaultStoragePhase();
        } else {
            storagePh = descr.storagePh.value;
        }

        var action: Maybe<Cell<TrActionPhase>> = descr.action;
        if (withoutAction) {
            action = None{};
        }

        val tickTock = TransTickTock {
            isTock,
            storagePh,
            computePh: descr.computePh,
            action,
            aborted: descr.aborted,
            destroyed: descr.destroyed,
        };
        return withDescription(tx, tickTock);
    }

    return tx;
}
"#;

fn run_ck_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/messages", CK_MESSAGES)
        .contract("worker", CK_WORKER_CONTRACT)
        .test_file("transaction_tick_tock", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn transaction_get_action_fee_tick_and_tock_match_ord_action_fee() {
    run_ck_success_case(
        "ck-stdlib-transaction-get-action-fee-tick-and-tock",
        r#"
get fun `test-ck-transaction-get-action-fee-tick-and-tock`() {
    val (sender, workerAddress) = deployCkHarness();
    val txs = sendCkPing(sender, workerAddress, 201, 4);
    val baseTx = txs.findTransaction<CkPing>({
        from: sender.address,
        to: workerAddress,
    }).unwrap();

    val descr = baseTx.description.load();
    if (descr is TransOrd) {
        expect(descr.action is None).toEqual(false);
    }

    val expected = baseTx.getActionFee();
    val tickTx = asTickTock(baseTx, false, false);
    val tockTx = asTickTock(baseTx, true, false);

    expect(tickTx.getActionFee()).toEqual(expected);
    expect(tockTx.getActionFee()).toEqual(expected);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ck_transaction_tests/ck_stdlib_transaction_get_action_fee_tick_and_tock_match_ord_action_fee.stdout.txt",
    );
}

#[test]
fn transaction_get_action_fee_tick_tock_without_action_returns_none() {
    run_ck_success_case(
        "ck-stdlib-transaction-get-action-fee-tick-tock-none",
        r#"
get fun `test-ck-transaction-get-action-fee-tick-tock-none`() {
    val (sender, workerAddress) = deployCkHarness();
    val txs = sendCkPing(sender, workerAddress, 202, 0);
    val baseTx = txs.findTransaction<CkPing>({
        from: sender.address,
        to: workerAddress,
    }).unwrap();

    val tickNoAction = asTickTock(baseTx, false, true);
    val tockNoAction = asTickTock(baseTx, true, true);

    expect(tickNoAction.getActionFee()).toBeNone();
    expect(tockNoAction.getActionFee()).toBeNone();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ck_transaction_tests/ck_stdlib_transaction_get_action_fee_tick_tock_without_action_returns_none.stdout.txt",
    );
}
