use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

const NOOP_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const WAITING_LOG: &str = "Awaiting transaction... [Attempt 1/1]";

fn run_wait_missing_hash_case(
    project_name: &str,
    get_method_name: &str,
    quiet: bool,
    snapshot_path: &str,
) {
    let quiet_literal = if quiet { "true" } else { "false" };
    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `{get_method_name}`() {{
    val sender = net.treasury("de_wait_sender");
    val receiver = net.treasury("de_wait_receiver");
    val txs = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.2"),
            dest: receiver.address,
        }}),
    );

    net.enableBroadcast();
    expect(txs.wait({quiet_literal}, 1, 1)).toEqual(false);
    net.disableBroadcast();
}}
"#
    );

    let output = ProjectBuilder::new(project_name)
        .contract("noop", NOOP_CONTRACT)
        .test_file("send_result_wait_missing_hash", &source)
        .build()
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .test()
        .run()
        .success();

    output.assert_passed(1);

    if quiet {
        output.assert_not_contains(WAITING_LOG);
    } else {
        output.assert_contains(WAITING_LOG);
    }
    output.assert_snapshot_matches(snapshot_path);
}

#[test]
fn wait_missing_tx_hash_non_quiet_returns_false() {
    run_wait_missing_hash_case(
        "de-stdlib-wait-missing-hash-non-quiet",
        "test-de-stdlib-wait-missing-hash-non-quiet",
        false,
        "integration/snapshots/test-runner/wait_missing_tx_hash_non_quiet_returns_false/wait_missing_tx_hash_non_quiet_returns_false.stdout.txt",
    );
}

#[test]
fn wait_missing_tx_hash_quiet_returns_false_without_wait_log() {
    run_wait_missing_hash_case(
        "de-stdlib-wait-missing-hash-quiet",
        "test-de-stdlib-wait-missing-hash-quiet",
        true,
        "integration/snapshots/test-runner/wait_missing_tx_hash_non_quiet_returns_false/wait_missing_tx_hash_quiet_returns_false_without_wait_log.stdout.txt",
    );
}
