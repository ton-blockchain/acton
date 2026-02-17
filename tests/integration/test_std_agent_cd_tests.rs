//! Reserved integration test module for subagent CD.
//!
//! Ownership boundary for agent CD:
//! - tests/integration/test_std_agent_cd_tests.rs
//! - tests/integration/snapshots/test_std_agent_cd/**
//! - tests/integration/testdata/test_std_agent_cd/**
//! - tests/support/test_std_agent_cd/** (optional)
//!
//! Required test name prefix:
//! - cd_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const OUTLIST_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/testing/outlist_expect"
"#;

fn run_outlist_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{OUTLIST_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("outlist_non_empty", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("expect(actual).toNotEqual(expected)")
        .assert_contains("Values are equal but expected to be different")
        .assert_contains("0")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cd_stdlib_outlist_to_be_non_empty_empty_list_reports_failure_message() {
    run_outlist_failure(
        "cd-stdlib-outlist-to-be-non-empty-empty-list",
        r#"
get fun `test-cd-stdlib-outlist-to-be-non-empty-empty-list`() {
    val out_actions = createEmptyTuple();
    expect(out_actions).toBeNonEmpty();
}
"#,
        "integration/snapshots/test_std_agent_cd/cd_stdlib_outlist_to_be_non_empty_empty_list_reports_failure_message.stdout.txt",
    );
}
