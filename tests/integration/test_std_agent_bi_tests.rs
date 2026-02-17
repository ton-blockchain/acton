//! Reserved integration test module for subagent BI.
//!
//! Ownership boundary for agent BI:
//! - tests/integration/test_std_agent_bi_tests.rs
//! - tests/integration/snapshots/test_std_agent_bi/**
//! - tests/integration/testdata/test_std_agent_bi/**
//! - tests/support/test_std_agent_bi/** (optional)
//!
//! Required test name prefix:
//! - bi_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
"#;

const NOOP_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn bi_stdlib_wait_for_transaction_missing_hash_before_send_returns_false_bug() {
    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test-bi-stdlib-wait-for-transaction-missing-hash-before-send`() {{
    val sender = net.treasury("bi_wait_sender");
    val missingHashSlice = beginCell().storeUint(0xB1, 8).storeUint(0, 248).toSlice();

    // BUG: net.waitForTransaction should return false in emulation mode for a missing hash before send, got compute-phase stack underflow.
    net.waitForTransaction(sender.address, missingHashSlice, true, 1, 1);
}}
"#
    );

    ProjectBuilder::new("bi-stdlib-wait-for-transaction-missing-hash-before-send")
        .contract("noop", NOOP_CONTRACT)
        .test_file("wait_for_transaction_missing_hash_before_send", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("stack underflow")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_bi/bi_stdlib_wait_for_transaction_missing_hash_before_send_returns_false_bug.stdout.txt",
        );
}
