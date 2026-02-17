//! Reserved integration test module for subagent EN.
//!
//! Ownership boundary for agent EN:
//! - tests/integration/test_std_agent_en_tests.rs
//! - tests/integration/snapshots/test_std_agent_en/**
//! - tests/integration/testdata/test_std_agent_en/**
//! - tests/support/test_std_agent_en/** (optional)
//!
//! Required test name prefix:
//! - en_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EN_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

struct (0xEE100001) EnExternalNotice {
    queryId: uint64
}
"#;

const EN_NOOP_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

fn run_find_external_out_message_compile_failure(
    project_name: &str,
    test_body: &str,
    expected_generic: &str,
    snapshot_path: &str,
) {
    let source = format!("{EN_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("noop", EN_NOOP_CONTRACT)
        .test_file("find_external_out_message_compile_failure", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("type arguments not expected here")
        .assert_contains("lib/emulation/network.tolk:326:21: error: type arguments not expected here")
        .assert_contains(expected_generic)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn en_stdlib_find_external_out_message_typed_branch_reports_tuple_get_type_argument_compile_error() {
    run_find_external_out_message_compile_failure(
        "en-stdlib-find-external-out-message-typed-branch-bug",
        r#"
get fun `test-en-find-external-out-message-typed-branch-bug`() {
    val txs: SendResultList = createEmptyTuple();

    // BUG: SendResultList.findExternalOutMessage typed branch should compile and return Maybe.none() for an empty list, but it fails because network.tolk calls tuple.get with type arguments.
    val found = txs.findExternalOutMessage<EnExternalNotice>({});
    expect(found).toBeNone();
}
"#,
        "SendResultList.findExternalOutMessage<EnExternalNotice>",
        "integration/snapshots/test_std_agent_en/en_stdlib_find_external_out_message_typed_branch_reports_tuple_get_type_argument_compile_error.stdout.txt",
    );
}

#[test]
fn en_stdlib_find_external_out_message_default_branch_reports_tuple_get_type_argument_compile_error() {
    run_find_external_out_message_compile_failure(
        "en-stdlib-find-external-out-message-default-branch-bug",
        r#"
get fun `test-en-find-external-out-message-default-branch-bug`() {
    val txs: SendResultList = createEmptyTuple();

    // BUG: SendResultList.findExternalOutMessage default branch should compile and return Maybe.none() for an empty list, but it fails because network.tolk calls tuple.get with type arguments.
    val found = txs.findExternalOutMessage({});
    expect(found).toBeNone();
}
"#,
        "SendResultList.findExternalOutMessage<never>",
        "integration/snapshots/test_std_agent_en/en_stdlib_find_external_out_message_default_branch_reports_tuple_get_type_argument_compile_error.stdout.txt",
    );
}
