use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/tlb/maybe"
"#;

const RECEIVER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn wait_for_transaction_returns_true_in_emulation_mode() {
    let source = format!(
        r#"
{NETWORK_IMPORTS}

fun deployReceiver() {{
    val sender = net.treasury("sender");

    val stateInit = ContractState {{
        code: build("receiver"),
        data: createEmptyCell(),
    }};
    val receiverAddress = AutoDeployAddress {{ stateInit }}.calculateAddress();

    val deployMsg = createMessage({{
        bounce: false,
        value: ton("1"),
        dest: {{
            stateInit,
        }},
    }});
    expect(net.send(sender.address, deployMsg)).toHaveSuccessfulDeploy({{ to: receiverAddress }});

    return (sender, receiverAddress);
}}

get fun `test bh stdlib wait for transaction positive known body hash`() {{
    val (sender, receiverAddress) = deployReceiver();

    val payload = beginCell().storeUint(0xBEEF, 16).storeUint(77, 8).endCell();
    val txs = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.2"),
            dest: receiverAddress,
            body: payload,
        }}),
    );
    expect(txs).toHaveTx({{
        from: sender.address,
        to: receiverAddress,
        success: true,
    }});

    val tx = txs.at(0).tx.load();
    val inMsgCell = tx.messages.load().inMsg.unwrap();
    val inMsg = inMsgCell.load();

    var body = inMsg.body;
    body.skipBits(1); // skip Either bit in Message body
    val bodyHash = body.hash();
    val bodyHashSlice = beginCell().storeUint(bodyHash, 256).toSlice();

    expect(net.waitForTransaction(inMsg.info.dest, bodyHashSlice, true, 1, 1)).toEqual(true);
}}
"#
    );

    ProjectBuilder::new("bh-stdlib-wait-for-transaction-emulation-noop")
        .contract("receiver", RECEIVER_CONTRACT)
        .test_file("wait_for_transaction_known_hash", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/wait_for_transaction_returns_true_in_emulation_mode/wait_for_transaction_returns_true_in_emulation_mode.stdout.txt",
        );
}
