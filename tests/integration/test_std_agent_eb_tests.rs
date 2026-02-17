//! Reserved integration test module for subagent EB.
//!
//! Ownership boundary for agent EB:
//! - tests/integration/test_std_agent_eb_tests.rs
//! - tests/integration/snapshots/test_std_agent_eb/**
//! - tests/integration/testdata/test_std_agent_eb/**
//! - tests/support/test_std_agent_eb/** (optional)
//!
//! Required test name prefix:
//! - eb_stdlib_

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

#[test]
fn eb_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders() {
    run_fmt_success(
        "eb-stdlib-format4-extra-args-ignored",
        r#"
get fun `test-eb-stdlib-format4-extra-args-ignored`() {
    val rendered = format4("{}: {:ton}", "alpha", 2500000000, "unused", 255);
    expect(rendered).toEqual("alpha: 2.5 TON");
}
"#,
        "integration/snapshots/test_std_agent_eb/eb_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders.stdout.txt",
    );
}

#[test]
fn eb_stdlib_format4_leaves_unmatched_placeholder_when_template_has_five_slots() {
    run_fmt_success(
        "eb-stdlib-format4-missing-placeholder-slot",
        r#"
get fun `test-eb-stdlib-format4-missing-placeholder-slot`() {
    val rendered = format4("a={} b={} c={} d={} e={}", 1, 2, 3, 4);
    expect(rendered).toEqual("a=1 b=2 c=3 d=4 e={}");
}
"#,
        "integration/snapshots/test_std_agent_eb/eb_stdlib_format4_leaves_unmatched_placeholder_when_template_has_five_slots.stdout.txt",
    );
}
