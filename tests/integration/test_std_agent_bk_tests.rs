//! Reserved integration test module for subagent BK.
//!
//! Ownership boundary for agent BK:
//! - tests/integration/test_std_agent_bk_tests.rs
//! - tests/integration/snapshots/test_std_agent_bk/**
//! - tests/integration/testdata/test_std_agent_bk/**
//! - tests/support/test_std_agent_bk/** (optional)
//!
//! Required test name prefix:
//! - bk_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_network_failure_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("network_storage_fee_missing", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn bk_stdlib_network_get_account_storage_fee_returns_null_for_missing_account_bug() {
    run_network_failure_case(
        "bk-stdlib-network-get-account-storage-fee-missing-account",
        r#"
get fun `test-bk-stdlib-network-get-account-storage-fee-missing-account`() {
    val missing = net.randomAddress("bk_missing_storage_fee_account");
    expect(net.getAccountState(missing)).toBeNull();
    // BUG: net.getAccountStorageFee should return null for a missing account; expected null, got 0.
    expect(net.getAccountStorageFee(missing, 86400)).toBeNull();
}
"#,
        "integration/snapshots/test_std_agent_bk/bk_stdlib_network_get_account_storage_fee_returns_null_for_missing_account_bug.stdout.txt",
    );
}

#[test]
fn bk_stdlib_network_get_account_storage_fee_returns_non_null_for_existing_account_bug_in_fixture_project(
) {
    let fixture = FixtureProject::load("basic");
    let source = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

get fun `test-bk-stdlib-network-get-account-storage-fee-existing-account`() {
    val seconds = 86400;
    val treasury = net.treasury("bk_storage_fee_sender");

    // BUG: net.getAccountStorageFee should return a non-null fee for a deployed treasury account; expected int value, got deserialization error (exit code 9).
    val storageFee = net.getAccountStorageFee(treasury.address, seconds);
    expect(storageFee).toBeNotNull();
}
"#;

    fs::write(
        fixture
            .path()
            .join("tests/network_get_account_storage_fee_existing.test.tolk"),
        source,
    )
    .expect("failed to write network getAccountStorageFee fixture test");

    fixture
        .acton()
        .test()
        .path("tests/network_get_account_storage_fee_existing.test.tolk")
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("extra data remaining in deserialized cell")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_bk/bk_stdlib_network_get_account_storage_fee_returns_non_null_for_existing_account_bug_in_fixture_project.stdout.txt",
        );
}
