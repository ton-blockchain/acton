//! Reserved integration test module for subagent CA.
//!
//! Ownership boundary for agent CA:
//! - tests/integration/test-runner/test_runner_stdlib_ca_expect_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ca_expect_tests/**
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

fn run_expect_tuple_membership_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("expect_tuple_membership", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_expect_tuple_membership_failure(
    project_name: &str,
    test_body: &str,
    snapshot_path: &str,
    contains: &[&str],
) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    let output = ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("expect_tuple_membership", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure();

    output.assert_failed(1);
    for needle in contains {
        output.assert_contains(needle);
    }
    output.assert_snapshot_matches(snapshot_path);
}

#[test]
fn expect_tuple_to_contain_existing_value_reports_compile_diagnostic_bug() {
    run_expect_tuple_membership_success(
        "ca-stdlib-expect-tuple-to-contain-existing-value-bug",
        r#"
get fun `test-ca-stdlib-to-contain-existing-value-bug`() {
    var values = createEmptyTuple();
    values.push(1);
    values.push(2);

    expect(values).toContain(2);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ca_expect_tests/ca_stdlib_expect_tuple_to_contain_existing_value_reports_compile_diagnostic_bug.stdout.txt",
    );
}

#[test]
fn expect_tuple_to_contain_missing_value_runtime_diagnostic_is_unreachable_bug() {
    run_expect_tuple_membership_failure(
        "ca-stdlib-expect-tuple-to-contain-missing-value-diagnostic-bug",
        r#"
get fun `test-ca-stdlib-to-contain-missing-value-diagnostic-bug`() {
    var values = createEmptyTuple();
    values.push(10);
    values.push(20);

    expect(values).toContain(30);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ca_expect_tests/ca_stdlib_expect_tuple_to_contain_missing_value_runtime_diagnostic_is_unreachable_bug.stdout.txt",
        &["Tuple doesn't contain the value"],
    );
}

#[test]
fn expect_tuple_to_not_contain_present_value_runtime_diagnostic_is_unreachable_bug() {
    run_expect_tuple_membership_failure(
        "ca-stdlib-expect-tuple-to-not-contain-present-value-diagnostic-bug",
        r#"
get fun `test-ca-stdlib-to-not-contain-present-value-diagnostic-bug`() {
    var values = createEmptyTuple();
    values.push(7);
    values.push(8);

    expect(values).toNotContain(8);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ca_expect_tests/ca_stdlib_expect_tuple_to_not_contain_present_value_runtime_diagnostic_is_unreachable_bug.stdout.txt",
        &["Tuple contains the value but it should not"],
    );
}

#[test]
fn expect_tuple_to_not_contain_missing_value_passes() {
    run_expect_tuple_membership_success(
        "ca-stdlib-expect-tuple-to-not-contain-missing-value",
        r#"
get fun `test-ca-stdlib-to-not-contain-missing-value`() {
    var values = createEmptyTuple();
    values.push(7);
    values.push(8);

    expect(values).toNotContain(9);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ca_expect_tests/ca_stdlib_expect_tuple_to_not_contain_missing_value_passes.stdout.txt",
    );
}

#[test]
fn expect_tuple_to_contain_missing_value_in_empty_tuple_reports_runtime_diagnostic() {
    run_expect_tuple_membership_failure(
        "ca-stdlib-expect-tuple-to-contain-empty-tuple-missing-value",
        r#"
get fun `test-ca-stdlib-to-contain-empty-tuple-missing-value`() {
    var values = createEmptyTuple();

    expect(values).toContain(1);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_ca_expect_tests/ca_stdlib_expect_tuple_to_contain_missing_value_in_empty_tuple_reports_runtime_diagnostic.stdout.txt",
        &["Tuple doesn't contain the value"],
    );
}
