//! Reserved integration test module for subagent CB.
//!
//! Ownership boundary for agent CB:
//! - tests/integration/test_std_agent_cb_tests.rs
//! - tests/integration/snapshots/test_std_agent_cb/**
//! - tests/integration/testdata/test_std_agent_cb/**
//! - tests/support/test_std_agent_cb/** (optional)
//!
//! Required test name prefix:
//! - cb_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const OUTLIST_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/outlist_expect"
import "../../lib/vm/vm"

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}
"#;

fn run_outlist_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{OUTLIST_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("outlist_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cb_stdlib_outlist_to_be_send_message_at_incompatible_typed_payload_reports_exit_code() {
    run_outlist_failure(
        "cb-stdlib-outlist-incompatible-typed-payload",
        r#"
get fun `test-cb-outlist-incompatible-typed-payload`() {
    val dest = net.randomAddress("counter");
    val incompatible_payload = beginCell()
        .storeUint(0x7e8764ef, 32)
        .storeUint(42, 64)
        .endCell()
        .beginParse();
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: incompatible_payload,
    });
    msg.send(SEND_MODE_REGULAR);

    val out_actions = vm.outActions();
    expectToEndWithExitCode(567);
    // BUG: toBeSendMessageAt should fail for malformed payload matching opcode prefix; expected ASSERTION_FAILED (567), got success (0).
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);
}
"#,
        "integration/snapshots/test_std_agent_cb/cb_stdlib_outlist_to_be_send_message_at_incompatible_typed_payload_reports_exit_code.stdout.txt",
    );
}
