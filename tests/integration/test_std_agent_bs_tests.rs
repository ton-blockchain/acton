//! Reserved integration test module for subagent BS.
//!
//! Ownership boundary for agent BS:
//! - tests/integration/test_std_agent_bs_tests.rs
//! - tests/integration/snapshots/test_std_agent_bs/**
//! - tests/integration/testdata/test_std_agent_bs/**
//! - tests/support/test_std_agent_bs/** (optional)
//!
//! Required test name prefix:
//! - bs_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FMT_TEST_IMPORTS: &str = r#"
import "../../lib/fmt"
import "../../lib/testing/expect"
"#;

fn wrap_fmt_test_source(test_body: &str) -> String {
    format!("{FMT_TEST_IMPORTS}\n{test_body}\n")
}

fn run_fmt_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = wrap_fmt_test_source(test_body);
    ProjectBuilder::new(project_name)
        .test_file("fmt_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
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
fn bs_stdlib_format2_plain_placeholders_use_default_formatter_for_int_and_bool() {
    run_fmt_success(
        "bs-stdlib-format2-default-formatter",
        r#"
get fun `test-bs-stdlib-format2-default-formatter`() {
    val rendered = format2("left={} right={}", -42, true);
    expect(rendered).toEqual("left=-42 right=true");
}
"#,
        "integration/snapshots/test_std_agent_bs/bs_stdlib_format2_plain_placeholders_use_default_formatter_for_int_and_bool.stdout.txt",
    );
}

#[test]
fn bs_stdlib_format2_escaped_braces_around_placeholder_should_collapse_bug() {
    run_fmt_failure(
        "bs-stdlib-format2-escaped-braces-bug",
        r#"
get fun `test-bs-stdlib-format2-escaped-braces-bug`() {
    val rendered = format2("open={{{}}} close={}", "inner", "done");
    // BUG: format2 matches "{}" inside "{{{}}}" and leaves doubled braces; expected "open={inner} close=done", got "open={{inner}} close=done".
    expect(rendered).toEqual("open={inner} close=done");
}
"#,
        "integration/snapshots/test_std_agent_bs/bs_stdlib_format2_escaped_braces_around_placeholder_should_collapse_bug.stdout.txt",
    );
}
