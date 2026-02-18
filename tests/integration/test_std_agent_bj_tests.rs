//! Reserved integration test module for subagent BJ.
//!
//! Ownership boundary for agent BJ:
//! - tests/integration/test_std_agent_bj_tests.rs
//! - tests/integration/snapshots/test_std_agent_bj/**
//! - tests/integration/testdata/test_std_agent_bj/**
//! - tests/support/test_std_agent_bj/** (optional)
//!
//! Required test name prefix:
//! - bj_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_bj_project_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_source = format!("{NETWORK_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("bj_get_account_state_transition", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_bj_fixture_success_case(test_file: &str, test_body: &str, snapshot_path: &str) {
    let fixture = FixtureProject::load("basic");
    let test_path = format!("tests/{test_file}.test.tolk");
    let test_source = format!("{NETWORK_IMPORTS}\n{test_body}\n");

    fs::write(fixture.path().join(&test_path), test_source)
        .expect("failed to write bj fixture test file");

    fixture
        .acton()
        .test()
        .path(&test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn bj_stdlib_get_account_state_after_top_up_returns_account_info_with_expected_balance() {
    run_bj_project_case(
        "bj-stdlib-get-account-state-after-top-up",
        r#"
get fun `test-bj-stdlib-get-account-state-after-top-up`() {
    val target = net.randomAddress("bj_state_after_top_up_balance_target");

    net.topUp(target, ton("2"));

    val state = net.getAccountState(target);
    expect(state).toBeNotNull();
    expect(state!.storage.balance.grams).toEqual(ton("2"));
}
"#,
        "integration/snapshots/test_std_agent_bj/bj_stdlib_get_account_state_after_top_up_returns_account_info_with_expected_balance.stdout.txt",
    );
}

#[test]
fn bj_stdlib_get_account_state_transitions_from_null_to_non_null_after_top_up() {
    run_bj_fixture_success_case(
        "bj_get_account_state_transition_bug",
        r#"
get fun `test-bj-stdlib-get-account-state-transition-bug`() {
    val target = net.randomAddress("bj_state_transition_before_after_top_up");
    val before = net.getAccountState(target);
    expect(before == null).toEqual(true);

    net.topUp(target, ton("1"));

    val after = net.getAccountState(target);
    expect(after).toBeNotNull();
    expect(after!.storage.balance.grams).toEqual(ton("1"));
}
"#,
        "integration/snapshots/test_std_agent_bj/bj_stdlib_get_account_state_should_transition_from_null_to_non_null_after_top_up_bug.stdout.txt",
    );
}
