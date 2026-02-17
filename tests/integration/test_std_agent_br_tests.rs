//! Reserved integration test module for subagent BR.
//!
//! Ownership boundary for agent BR:
//! - tests/integration/test_std_agent_br_tests.rs
//! - tests/integration/snapshots/test_std_agent_br/**
//! - tests/integration/testdata/test_std_agent_br/**
//! - tests/support/test_std_agent_br/** (optional)
//!
//! Required test name prefix:
//! - br_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const FMT_TEST_IMPORTS: &str = r#"
import "../../lib/fmt"
import "../../lib/testing/expect"
"#;

fn wrap_fmt_test_source(test_body: &str) -> String {
    format!("{FMT_TEST_IMPORTS}\n{test_body}\n")
}

fn run_project_builder_fmt_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
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
fn br_stdlib_format5_mixed_specifiers_should_follow_placeholder_order_bug() {
    run_project_builder_fmt_failure(
        "br-stdlib-format5-placeholder-order-bug",
        r#"
get fun `test-br-stdlib-format5-placeholder-order-bug`() {
    val rendered = format5("{} | {:ton} | {:x} | {} | {}", 255, 1500000000, 16, "left", "right");
    // BUG: format5 applies {:x} and {:ton} while iterating args, not by placeholder position; expected "255 | 1.5 TON | 10 | left | right", got "16 | 1.5 TON | ff | left | right".
    expect(rendered).toEqual("255 | 1.5 TON | 10 | left | right");
}
"#,
        "integration/snapshots/test_std_agent_br/br_stdlib_format5_mixed_specifiers_should_follow_placeholder_order_bug.stdout.txt",
    );
}

#[test]
fn br_stdlib_format5_mixed_specifiers_should_follow_placeholder_order_in_fixture_project_bug() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/br_stdlib_format5_placeholder_order_bug.test.tolk";
    let source = wrap_fmt_test_source(
        r#"
get fun `test-br-stdlib-format5-placeholder-order-fixture-bug`() {
    val rendered = format5("{:ton} | {} | {:x} | {} | {}", 1500000000, 2000000000, 16, "mid", "end");
    // BUG: format5 consumes {:x} before {:ton} regardless of placeholder order; expected "1.5 TON | 2000000000 | 10 | mid | end", got "2 TON | 16 | 59682f00 | mid | end".
    expect(rendered).toEqual("1.5 TON | 2000000000 | 10 | mid | end");
}
"#,
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write BR fixture format5 placeholder-order test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_br/br_stdlib_format5_mixed_specifiers_should_follow_placeholder_order_in_fixture_project_bug.stdout.txt",
        );
}
