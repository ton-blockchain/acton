//! Reserved integration test module for subagent EA.
//!
//! Ownership boundary for agent EA:
//! - tests/integration/test_std_agent_ea_tests.rs
//! - tests/integration/snapshots/test_std_agent_ea/**
//! - tests/integration/testdata/test_std_agent_ea/**
//! - tests/support/test_std_agent_ea/** (optional)
//!
//! Required test name prefix:
//! - ea_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FMT_TEST_IMPORTS: &str = r#"
import "../../lib/fmt"
import "../../lib/testing/expect"
"#;

fn wrap_fmt_test_source(test_body: &str) -> String {
    format!("{FMT_TEST_IMPORTS}\n{test_body}\n")
}

fn run_fmt_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = wrap_fmt_test_source(test_body);
    ProjectBuilder::new(project_name)
        .test_file("fmt_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ea_stdlib_format3_mixed_placeholders_should_follow_template_order_bug() {
    run_fmt_failure(
        "ea-stdlib-format3-mixed-placeholder-order-bug",
        r#"
get fun `test-ea-stdlib-format3-mixed-placeholder-order-bug`() {
    val rendered = format3("{} | {:x} | {:ton}", 255, 16, 1500000000);
    // BUG: format3 resolves placeholders by formatter priority instead of placeholder order; expected "255 | 10 | 1.5 TON", got "1500000000 | ff | 0.000000016 TON".
    expect(rendered).toEqual("255 | 10 | 1.5 TON");
}
"#,
        "integration/snapshots/test_std_agent_ea/ea_stdlib_format3_mixed_placeholders_should_follow_template_order_bug.stdout.txt",
    );
}

#[test]
fn ea_stdlib_format3_escaped_braces_around_placeholder_should_collapse_bug() {
    run_fmt_failure(
        "ea-stdlib-format3-escaped-braces-bug",
        r#"
get fun `test-ea-stdlib-format3-escaped-braces-bug`() {
    val rendered = format3("wrap={{{}}} hex={:x} ton={:ton}", "inner", 255, 2500000000);
    // BUG: format3 matches "{}" inside "{{{}}}" instead of honoring escaped braces; expected "wrap={inner} hex=ff ton=2.5 TON", got "wrap={{inner}} hex=ff ton=2.5 TON".
    expect(rendered).toEqual("wrap={inner} hex=ff ton=2.5 TON");
}
"#,
        "integration/snapshots/test_std_agent_ea/ea_stdlib_format3_escaped_braces_around_placeholder_should_collapse_bug.stdout.txt",
    );
}
