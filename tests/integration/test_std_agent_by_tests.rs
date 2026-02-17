//! Reserved integration test module for subagent BY.
//!
//! Ownership boundary for agent BY:
//! - tests/integration/test_std_agent_by_tests.rs
//! - tests/integration/snapshots/test_std_agent_by/**
//! - tests/integration/testdata/test_std_agent_by/**
//! - tests/support/test_std_agent_by/** (optional)
//!
//! Required test name prefix:
//! - by_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn run_by_expect_suite(
    project_name: &str,
    test_source: &str,
) -> crate::support::assertions::TestOutput {
    ProjectBuilder::new(project_name)
        .test_file("expect_bool", test_source)
        .build()
        .acton()
        .test()
        .run()
}

#[test]
fn by_stdlib_expect_bool_matchers_pass_for_true_and_false_values() {
    let source = format!(
        r#"{EXPECT_IMPORTS}

get fun `test-by-expect-bool-pass-true`() {{
    expect(true).toBeTrue();
}}

get fun `test-by-expect-bool-pass-false`() {{
    expect(false).toBeFalse();
}}
"#
    );

    run_by_expect_suite("by-stdlib-expect-bool-pass", &source)
        .success()
        .assert_passed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_by/by_stdlib_expect_bool_matchers_pass_for_true_and_false_values.stdout.txt",
        );
}

#[test]
fn by_stdlib_expect_bool_matcher_mismatch_reports_tuple_diff() {
    let source = format!(
        r#"{EXPECT_IMPORTS}

get fun `test-by-expect-bool-fail-to-be-true`() {{
    expect(false).toBeTrue();
}}

get fun `test-by-expect-bool-fail-to-be-false`() {{
    expect(true).toBeFalse();
}}
"#
    );

    run_by_expect_suite("by-stdlib-expect-bool-fail", &source)
        .failure()
        .assert_failed(2)
        .assert_contains("expect(actual).toBeTrue()")
        .assert_contains("expect(actual).toBeFalse()")
        .assert_contains("false,\n            true")
        .assert_contains("true,\n            false")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_by/by_stdlib_expect_bool_matcher_mismatch_reports_tuple_diff.stdout.txt",
        );
}
