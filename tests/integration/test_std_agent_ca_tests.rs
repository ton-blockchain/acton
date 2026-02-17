//! Reserved integration test module for subagent CA.
//!
//! Ownership boundary for agent CA:
//! - tests/integration/test_std_agent_ca_tests.rs
//! - tests/integration/snapshots/test_std_agent_ca/**
//! - tests/integration/testdata/test_std_agent_ca/**
//! - tests/support/test_std_agent_ca/** (optional)
//!
//! Required test name prefix:
//! - ca_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

fn run_expect_tuple_membership_compile_failure(
    project_name: &str,
    test_body: &str,
    snapshot_path: &str,
) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("expect_tuple_membership", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_stderr_contains("type arguments not expected here")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ca_stdlib_expect_tuple_to_contain_existing_value_reports_compile_diagnostic_bug() {
    run_expect_tuple_membership_compile_failure(
        "ca-stdlib-expect-tuple-to-contain-existing-value-bug",
        r#"
get fun `test-ca-stdlib-to-contain-existing-value-bug`() {
    var values = createEmptyTuple();
    values.push(1);
    values.push(2);

    // BUG: Expectation<tuple>.toContain() should compile and validate tuple membership at runtime; expected pass, got compiler error "type arguments not expected here".
    expect(values).toContain(2);
}
"#,
        "integration/snapshots/test_std_agent_ca/ca_stdlib_expect_tuple_to_contain_existing_value_reports_compile_diagnostic_bug.stdout.txt",
    );
}

#[test]
fn ca_stdlib_expect_tuple_to_contain_missing_value_runtime_diagnostic_is_unreachable_bug() {
    run_expect_tuple_membership_compile_failure(
        "ca-stdlib-expect-tuple-to-contain-missing-value-diagnostic-bug",
        r#"
get fun `test-ca-stdlib-to-contain-missing-value-diagnostic-bug`() {
    var values = createEmptyTuple();
    values.push(10);
    values.push(20);

    // BUG: Expectation<tuple>.toContain() should fail at runtime with "Tuple doesn't contain the value"; got compiler error "type arguments not expected here".
    expect(values).toContain(30);
}
"#,
        "integration/snapshots/test_std_agent_ca/ca_stdlib_expect_tuple_to_contain_missing_value_runtime_diagnostic_is_unreachable_bug.stdout.txt",
    );
}

#[test]
fn ca_stdlib_expect_tuple_to_not_contain_present_value_runtime_diagnostic_is_unreachable_bug() {
    run_expect_tuple_membership_compile_failure(
        "ca-stdlib-expect-tuple-to-not-contain-present-value-diagnostic-bug",
        r#"
get fun `test-ca-stdlib-to-not-contain-present-value-diagnostic-bug`() {
    var values = createEmptyTuple();
    values.push(7);
    values.push(8);

    // BUG: Expectation<tuple>.toNotContain() should fail at runtime with "Tuple contains the value but it should not"; got compiler error "type arguments not expected here".
    expect(values).toNotContain(8);
}
"#,
        "integration/snapshots/test_std_agent_ca/ca_stdlib_expect_tuple_to_not_contain_present_value_runtime_diagnostic_is_unreachable_bug.stdout.txt",
    );
}
