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
fn wait_for_trace_returns_send_result_list_in_emulation_mode() {
    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test bi stdlib wait for trace returns self in emulation`() {{
    val sender = testing.treasury("bi_trace_sender");
    val receiver = testing.treasury("bi_trace_receiver");
    val txs = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.2"),
            dest: receiver.address,
        }}),
    );

    expect(txs.waitForTrace(true, 1, 1)).toBeNotNull();
}}
"#
    );

    ProjectBuilder::new("bi-stdlib-wait-for-trace-emulation-noop")
        .contract("noop", NOOP_CONTRACT)
        .test_file("wait_for_trace_in_emulation", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/wait_for_trace_returns_send_result_list_in_emulation_mode/wait_for_trace_returns_send_result_list_in_emulation_mode.stdout.txt",
        );
}
