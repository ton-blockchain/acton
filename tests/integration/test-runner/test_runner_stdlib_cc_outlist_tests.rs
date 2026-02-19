//! Reserved integration test module for subagent CC.
//!
//! Ownership boundary for agent CC:
//! - tests/integration/test-runner/test_runner_stdlib_cc_outlist_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_cc_outlist_tests/**
//! - tests/integration/testdata/test_std_agent_cc/**
//! - tests/support/test_std_agent_cc/** (optional)
//!
//! Required test name prefix:
//! - cc_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const OUTLIST_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/outlist_expect"
import "../../lib/vm/vm"
"#;

fn run_outlist_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{OUTLIST_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("outlist_to_be_empty", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn outlist_to_be_empty_passes_for_empty_out_actions() {
    run_outlist_success(
        "cc-stdlib-outlist-to-be-empty-pass",
        r#"
get fun `test-cc-outlist-to-be-empty-pass`() {
    val out_actions = createEmptyTuple();
    expect(out_actions).toBeEmpty();
    expect(out_actions.size()).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_cc_outlist_tests/cc_stdlib_outlist_to_be_empty_passes_for_empty_out_actions.stdout.txt",
    );
}

#[test]
fn outlist_to_be_empty_fails_for_non_empty_out_actions() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cc_stdlib_outlist_to_be_empty_non_empty_fail.test.tolk";
    let source = format!(
        "{OUTLIST_IMPORTS}\n{}\n",
        r#"
get fun `test-cc-outlist-to-be-empty-non-empty-fail`() {
    val dest = net.randomAddress("counter");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: beginCell().storeUint(0xAABBCCDD, 32).endCell().beginParse(),
    });
    msg.send(SEND_MODE_REGULAR);

    val out_actions = vm.outActions();
    expect(out_actions).toBeEmpty();
}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write CC fixture outlist toBeEmpty failure test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_cc_outlist_tests/cc_stdlib_outlist_to_be_empty_fails_for_non_empty_out_actions.stdout.txt",
        );
}
