//! Reserved integration test module for subagent EK.
//!
//! Ownership boundary for agent EK:
//! - tests/integration/test-runner/test_runner_stdlib_ek_assert_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ek_assert_tests/**
//! - tests/integration/testdata/test_std_agent_ek/**
//! - tests/support/test_std_agent_ek/** (optional)
//!
//! Required test name prefix:
//! - ek_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
"#;

fn wrap_assert_source(test_body: &str) -> String {
    format!("{ASSERT_IMPORTS}\n{test_body}\n")
}

#[test]
fn assert_equal_passes_for_identical_tuple_values() {
    let source = wrap_assert_source(
        r#"
get fun `test-ek-stdlib-assert-equal-pass-identical-tuple`() {
    var left = createEmptyTuple();
    left.push(10);
    left.push("ok");

    var right = createEmptyTuple();
    right.push(10);
    right.push("ok");

    Assert.equal(left, right, "ek Assert.equal pass tuple");
}
"#,
    );

    ProjectBuilder::new("ek-stdlib-assert-equal-pass-identical-tuple")
        .test_file("assert_equal_pass", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_ek_assert_tests/ek_stdlib_assert_equal_passes_for_identical_tuple_values.stdout.txt",
        );
}

#[test]
fn assert_equal_reports_actual_and_expected_on_failure() {
    let source = wrap_assert_source(
        r#"
get fun `test-ek-stdlib-assert-equal-failure-diagnostics`() {
    Assert.equal(42, 41, "ek Assert.equal mismatch diagnostic");
}
"#,
    );

    ProjectBuilder::new("ek-stdlib-assert-equal-failure-diagnostics")
        .test_file("assert_equal_failure", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("ek Assert.equal mismatch diagnostic")
        .assert_contains("(")
        .assert_contains("42")
        .assert_contains("41")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_ek_assert_tests/ek_stdlib_assert_equal_reports_actual_and_expected_on_failure.stdout.txt",
        );
}
