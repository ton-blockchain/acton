//! Reserved integration test module for subagent AF.
//!
//! Ownership boundary for agent AF:
//! - tests/integration/test_std_agent_af_tests.rs
//! - tests/integration/snapshots/test_std_agent_af/**
//! - tests/integration/testdata/test_std_agent_af/**
//! - tests/support/test_std_agent_af/** (optional)
//!
//! Required test name prefix:
//! - af_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const OUTLIST_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/outlist_expect"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"

struct (0x7e8764ef) IncreaseCounter {
    queryId: uint64
    increaseBy: uint32
}

struct (0x51fd716d) DecreaseCounter {
    queryId: uint64
    decreaseBy: uint32
}
"#;

fn run_outlist_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{OUTLIST_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("outlist_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

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
fn af_stdlib_outlist_to_be_non_empty_passes_for_single_send_action() {
    run_outlist_success(
        "af-stdlib-outlist-to-be-non-empty",
        r#"
get fun `test-af-outlist-to-be-non-empty`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: IncreaseCounter { queryId: 1, increaseBy: 10 },
    });

    msg.send(SEND_MODE_REGULAR);

    val out_actions = vm.outActions();
    expect(out_actions).toBeNonEmpty();
    expect(out_actions.size()).toEqual(1);
}
"#,
        "integration/snapshots/test_std_agent_af/af_stdlib_outlist_to_be_non_empty_passes_for_single_send_action.stdout.txt",
    );
}

#[test]
fn af_stdlib_outlist_to_be_empty_passes_without_actions() {
    run_outlist_success(
        "af-stdlib-outlist-to-be-empty",
        r#"
get fun `test-af-outlist-to-be-empty`() {
    val out_actions = createEmptyTuple();
    expect(out_actions).toBeEmpty();
    expect(out_actions.size()).toEqual(0);
}
"#,
        "integration/snapshots/test_std_agent_af/af_stdlib_outlist_to_be_empty_passes_without_actions.stdout.txt",
    );
}

#[test]
fn af_stdlib_outlist_to_be_send_message_at_extracts_typed_body() {
    run_outlist_success(
        "af-stdlib-outlist-typed-send-message",
        r#"
get fun `test-af-outlist-send-message-typed-body`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: IncreaseCounter { queryId: 7, increaseBy: 77 },
    });
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);

    val out_actions = vm.outActions();
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);

    val action = out_actions.at(0);
    if (action is OutActionSendMessage) {
        val body = action.loadBody<IncreaseCounter>();
        expect(body.queryId).toEqual(7);
        expect(body.increaseBy).toEqual(77);
    }
}
"#,
        "integration/snapshots/test_std_agent_af/af_stdlib_outlist_to_be_send_message_at_extracts_typed_body.stdout.txt",
    );
}

#[test]
fn af_stdlib_outlist_to_be_send_message_at_opcode_mismatch_reports_known_type() {
    run_outlist_failure(
        "af-stdlib-outlist-opcode-mismatch-known-type",
        r#"
get fun `test-af-outlist-opcode-mismatch-known-type`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: IncreaseCounter { queryId: 11, increaseBy: 22 },
    });
    msg.send(SEND_MODE_REGULAR);

    val out_actions = vm.outActions();
    expect(out_actions).toBeSendMessageAt<DecreaseCounter>(0);
}
"#,
        "integration/snapshots/test_std_agent_af/af_stdlib_outlist_to_be_send_message_at_opcode_mismatch_reports_known_type.stdout.txt",
    );
}

#[test]
fn af_stdlib_outlist_to_be_send_message_at_opcode_mismatch_without_known_type_name() {
    run_outlist_success(
        "af-stdlib-outlist-opcode-mismatch-unknown-type",
        r#"
get fun `test-af-outlist-opcode-mismatch-unknown-type`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: beginCell().storeUint(0x3f6a9bcd, 32).storeUint(1, 32).endCell().beginParse(),
    });
    msg.send(SEND_MODE_REGULAR);

    val out_actions = vm.outActions();
    expectToEndWithExitCode(567);
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);
}
"#,
        "integration/snapshots/test_std_agent_af/af_stdlib_outlist_to_be_send_message_at_opcode_mismatch_without_known_type_name.stdout.txt",
    );
}

#[test]
fn af_stdlib_outlist_to_be_send_message_at_bounced_prefix_opcode_mismatch_is_reported() {
    run_outlist_success(
        "af-stdlib-outlist-opcode-mismatch-bounced-prefix",
        r#"
get fun `test-af-outlist-opcode-mismatch-bounced-prefix`() {
    val dest = net.randomAddress("counter");
    val bounced_like_body = beginCell()
        .storeUint(0xFFFFFFFF, 32)
        .storeSlice(IncreaseCounter { queryId: 33, increaseBy: 44 }.toCell().beginParse())
        .endCell()
        .beginParse();
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: bounced_like_body,
    }).bounced();
    msg.send(SEND_MODE_REGULAR);

    val out_actions = vm.outActions();
    expectToEndWithExitCode(567);
    expect(out_actions).toBeSendMessageAt<IncreaseCounter>(0);
}
"#,
        "integration/snapshots/test_std_agent_af/af_stdlib_outlist_to_be_send_message_at_bounced_prefix_opcode_mismatch_is_reported.stdout.txt",
    );
}
