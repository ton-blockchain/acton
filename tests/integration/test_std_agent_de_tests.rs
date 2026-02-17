//! Reserved for agent-de.
//! Prefix: de_stdlib_
//! Ownership: this file and tests/integration/snapshots/test_std_agent_de/**
//! Agent-owned tests for SendResultList.wait missing-hash behavior.

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
"#;

const NOOP_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const PANIC_NULL_OBJECT: &str = "Attempted to create a NULL object.";
const PANIC_EVENT_LOOP: &str = "event loop thread panicked";
const PANIC_ABORT: &str = "thread caused non-unwinding panic. aborting.";
const WAITING_LOG: &str = "Awaiting transaction... [Attempt 1/1]";

fn run_wait_missing_hash_case(
    project_name: &str,
    get_method_name: &str,
    quiet: bool,
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
    // BUG: SendResultList.wait should return false without failing the test when tx hash is missing.
    txs.wait({quiet_literal}, 1, 1);
    net.disableBroadcast();
}}
"#
    );

    let output = ProjectBuilder::new(project_name)
        .contract("noop", NOOP_CONTRACT)
        .test_file("send_result_wait_missing_hash", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure();

    // The wait path currently aborts while creating reqwest client in broadcast mode,
    // before quiet/non-quiet polling logs are emitted.
    output
        .assert_contains(PANIC_NULL_OBJECT)
        .assert_contains(PANIC_EVENT_LOOP)
        .assert_contains(PANIC_ABORT)
        .assert_not_contains(WAITING_LOG);
}

#[test]
fn de_stdlib_wait_missing_tx_hash_non_quiet_panics_before_wait_logging_bug() {
    run_wait_missing_hash_case(
        "de-stdlib-wait-missing-hash-non-quiet",
        "test-de-stdlib-wait-missing-hash-non-quiet",
        false,
    );
}

#[test]
fn de_stdlib_wait_missing_tx_hash_quiet_panics_before_wait_logging_bug() {
    run_wait_missing_hash_case(
        "de-stdlib-wait-missing-hash-quiet",
        "test-de-stdlib-wait-missing-hash-quiet",
        true,
    );
}
