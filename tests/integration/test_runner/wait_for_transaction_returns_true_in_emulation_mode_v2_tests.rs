use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

const NOOP_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn wait_for_transaction_returns_true_in_emulation_mode() {
    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test bi stdlib wait for transaction missing hash before send`() {{
    val sender = testing.treasury("bi_wait_sender");
    val receiver = testing.treasury("bi_wait_receiver");
    val txs = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.2"),
            dest: receiver.address,
        }}),
    );

    expect(txs.waitForFirstTransaction(true, 1, 1)).toBeNotNull();
}}
"#
    );

    ProjectBuilder::new("bi-stdlib-wait-for-transaction-emulation-noop")
        .contract("noop", NOOP_CONTRACT)
        .test_file("wait_for_transaction_missing_hash_before_send", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/wait_for_transaction_returns_true_in_emulation_mode_v2/wait_for_transaction_returns_true_in_emulation_mode.stdout.txt",
        );
}
