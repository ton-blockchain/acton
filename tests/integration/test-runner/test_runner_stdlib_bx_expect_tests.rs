//! Reserved integration test module for subagent BX.
//!
//! Ownership boundary for agent BX:
//! - tests/integration/test-runner/test_runner_stdlib_bx_expect_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_bx_expect_tests/**
//! - tests/integration/testdata/test_std_agent_bx/**
//! - tests/support/test_std_agent_bx/** (optional)
//!
//! Required test name prefix:
//! - bx_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/ffi/ffi"
"#;

fn run_bx_expect_suite(
    project_name: &str,
    test_source: &str,
) -> crate::support::assertions::TestOutput {
    ProjectBuilder::new(project_name)
        .test_file("expect_nan", test_source)
        .build()
        .acton()
        .test()
        .run()
}

#[test]
fn expect_nan_matchers_accept_nan_and_non_nan_values() {
    let source = format!(
        r#"{EXPECT_IMPORTS}

get fun `test-bx-expect-nan-pass`() {{
    val value = ffi.nan();
    expect(value).toBeNaN();
    expect(0).toBeNonNaN();
    expect(-1).toBeNonNaN();
}}
"#
    );

    run_bx_expect_suite("bx-stdlib-expect-nan-pass", &source)
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_bx_expect_tests/bx_stdlib_expect_nan_matchers_accept_nan_and_non_nan_values.stdout.txt",
        );
}

#[test]
fn expect_nan_matchers_report_mismatch_for_nan_and_non_nan_edges() {
    let source = format!(
        r#"{EXPECT_IMPORTS}

get fun `test-bx-expect-nan-fail-regular-int`() {{
    expect(0).toBeNaN();
}}

get fun `test-bx-expect-non-nan-fail-nan`() {{
    expect(ffi.nan()).toBeNonNaN();
}}
"#
    );

    run_bx_expect_suite("bx-stdlib-expect-nan-fail", &source)
        .failure()
        .assert_failed(2)
        .assert_contains("expect(actual).toBeNaN()")
        .assert_contains("expect(actual).toBeNonNaN()")
        .assert_contains("Values are equal but expected to be different")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_bx_expect_tests/bx_stdlib_expect_nan_matchers_report_mismatch_for_nan_and_non_nan_edges.stdout.txt",
        );
}
