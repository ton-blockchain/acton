use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CK_MESSAGES: &str = r"
struct (0xCC01CC01) CkPing {
    queryId: uint64
    amount: uint32
    target: address
}

struct (0xCC01CC02) CkNotify {
    queryId: uint64
    amount: uint32
}
";

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
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/transaction"
import "../../lib/tlb/maybe"
import "../contracts/messages"

fun deployCkHarness() {
    val sender = testing.treasury("sender");
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
    val target = randomAddress("ck_tick_tock_target");
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

fun defaultStoragePhase(): TlbTrStoragePhase {
    return TlbTrStoragePhase {
        storageFeesCollected: 0,
        storageFeesDue: TlbNone{},
        statusChange: TlbAccUnchanged{},
    };
}

fun withDescription(tx: TlbTransaction, descr: TlbTransactionDescr): TlbTransaction {
    val descrCell: Cell<TlbTransactionDescr> = descr.toCell();
    return TlbTransaction {
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

fun asTickTock(tx: TlbTransaction, isTock: bool, withoutAction: bool): TlbTransaction {
    val descr = tx.description.load();
    if (descr is TlbTransOrd) {
        var storagePh = defaultStoragePhase();
        if (descr.storagePh !is TlbNone) {
            storagePh = descr.storagePh.unwrap();
        }

        var action: TlbMaybe<Cell<TlbTrActionPhase>> = descr.action;
        if (withoutAction) {
            action = TlbNone{};
        }

        val tickTock = TlbTransTickTock {
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

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
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
    run_success_case(
        "ck-stdlib-transaction-get-action-fee-tick-and-tock",
        r"
get fun `test ck transaction get action fee tick and tock`() {
    val (sender, workerAddress) = deployCkHarness();
    val txs = sendCkPing(sender, workerAddress, 201, 4);
    val baseTx = txs.findTransaction<CkPing>({
        from: sender.address,
        to: workerAddress,
    })!;

    val descr = baseTx.description.load();
    if (descr is TlbTransOrd) {
        expect(descr.action is TlbNone).toEqual(false);
    }

    val expected = baseTx.getActionFee();
    val tickTx = asTickTock(baseTx, false, false);
    val tockTx = asTickTock(baseTx, true, false);

    expect(tickTx.getActionFee()).toEqual(expected);
    expect(tockTx.getActionFee()).toEqual(expected);
}
",
        "integration/snapshots/test-runner/transaction_get_action_fee_tick_and_tock_match_ord_action_fee/transaction_get_action_fee_tick_and_tock_match_ord_action_fee.stdout.txt",
    );
}

#[test]
fn transaction_get_action_fee_tick_tock_without_action_returns_none() {
    run_success_case(
        "ck-stdlib-transaction-get-action-fee-tick-tock-none",
        r"
get fun `test ck transaction get action fee tick tock none`() {
    val (sender, workerAddress) = deployCkHarness();
    val txs = sendCkPing(sender, workerAddress, 202, 0);
    val baseTx = txs.findTransaction<CkPing>({
        from: sender.address,
        to: workerAddress,
    })!;

    val tickNoAction = asTickTock(baseTx, false, true);
    val tockNoAction = asTickTock(baseTx, true, true);

    expect(tickNoAction.getActionFee()).toBeNull();
    expect(tockNoAction.getActionFee()).toBeNull();
}
",
        "integration/snapshots/test-runner/transaction_get_action_fee_tick_and_tock_match_ord_action_fee/transaction_get_action_fee_tick_tock_without_action_returns_none.stdout.txt",
    );
}
