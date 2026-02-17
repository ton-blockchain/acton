//! Reserved integration test module for subagent CY.
//!
//! Ownership boundary for agent CY:
//! - tests/integration/test_std_agent_cy_tests.rs
//! - tests/integration/snapshots/test_std_agent_cy/**
//! - tests/integration/testdata/test_std_agent_cy/**
//! - tests/support/test_std_agent_cy/** (optional)
//!
//! Required test name prefix:
//! - cy_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CY_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/types/out_actions"
"#;

fn run_cy_stdlib_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CY_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("out_actions_malformed", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Unknown out action")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cy_stdlib_out_action_from_tuple_rejects_single_item_tuple() {
    run_cy_stdlib_failure(
        "cy-stdlib-out-action-from-tuple-rejects-single-item",
        r#"
get fun `test-cy-out-action-from-tuple-rejects-single-item`() {
    var malformedAction = createEmptyTuple();
    malformedAction.push(777);

    var outActions = createEmptyTuple();
    outActions.push(malformedAction);
    outActions.at(0);
}
"#,
        "integration/snapshots/test_std_agent_cy/cy_stdlib_out_action_from_tuple_rejects_single_item_tuple.stdout.txt",
    );
}

#[test]
fn cy_stdlib_out_action_from_tuple_rejects_oversized_tuple_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cy_out_action_from_tuple_oversized.test.tolk";
    let source = format!(
        r#"{CY_OUT_ACTIONS_IMPORTS}
get fun `test-cy-out-action-from-tuple-rejects-oversized`() {{
    var malformedAction = createEmptyTuple();
    malformedAction.push(createEmptyCell());
    malformedAction.push(1);
    malformedAction.push(2);
    malformedAction.push(3);
    malformedAction.push(4);

    var outActions = createEmptyTuple();
    outActions.push(malformedAction);
    outActions.at(0);
}}
"#
    );
    fs::write(fixture.path().join(test_path), source).expect("failed to write cy fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Unknown out action")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_cy/cy_stdlib_out_action_from_tuple_rejects_oversized_tuple_in_fixture_project.stdout.txt",
        );
}
