//! Reserved integration test module for subagent R.
//!
//! Ownership boundary for agent R:
//! - tests/integration/test_lib_agent_r_tests.rs
//! - tests/integration/snapshots/test_lib_agent_r/**
//! - tests/integration/testdata/test_lib_agent_r/**
//! - tests/support/test_lib_agent_r/** (optional)
//!
//! Required test name prefix:
//! - r_lib_api_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const TEST_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_account_state_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_code = format!(
        r#"
            {}

            {}
        "#,
        TEST_IMPORTS, test_body
    );

    ProjectBuilder::new(project_name)
        .test_file("account_state", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_account_state_failure_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_code = format!(
        r#"
            {}

            {}
        "#,
        TEST_IMPORTS, test_body
    );

    ProjectBuilder::new(project_name)
        .test_file("account_state", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn r_lib_api_top_up_materializes_account_and_increases_balance() {
    run_account_state_case(
        "r-lib-api-top-up-materializes-account",
        r#"
        get fun `test-top-up-materializes-account-and-increases-balance`() {
            val target = net.randomAddress("r-top-up-target");
            val initialBalance = net.balance(target);

            net.topUp(target, ton("1"));
            val firstBalance = net.balance(target);
            expect(firstBalance).toBeGreater(initialBalance);

            net.topUp(target, ton("2"));
            val secondBalance = net.balance(target);
            expect(secondBalance).toBeGreater(firstBalance);
        }
        "#,
        "integration/snapshots/test_lib_agent_r/r_lib_api_top_up_materializes_account_and_increases_balance.stdout.txt",
    );
}

#[test]
fn r_lib_api_set_shard_account_null_resets_state_and_balance() {
    run_account_state_case(
        "r-lib-api-set-shard-account-null",
        r#"
        get fun `test-set-shard-account-null-resets-state-and-balance`() {
            val target = net.randomAddress("r-shard-reset-target");
            net.topUp(target, ton("1"));

            val balanceAfterTopUp = net.balance(target);
            expect(balanceAfterTopUp).toBeGreater(0);

            net.setShardAccount(target, null);

            expect(net.balance(target)).toEqual(0);

            val shardAfterReset = net.getShardAccount(target);
            expect(shardAfterReset).toBeNotNull();
            expect(shardAfterReset!.lastTransLt).toEqual(0);
            expect(shardAfterReset!.lastTransHash).toEqual(0);
        }
        "#,
        "integration/snapshots/test_lib_agent_r/r_lib_api_set_shard_account_null_resets_state_and_balance.stdout.txt",
    );
}

#[test]
fn r_lib_api_set_account_preserves_shard_markers_for_existing_address() {
    run_account_state_case(
        "r-lib-api-set-account-preserves-shard-markers",
        r#"
        get fun `test-set-account-preserves-shard-markers`() {
            val target = net.randomAddress("r-set-account-target");

            net.topUp(target, ton("1"));
            net.topUp(target, ton("1"));

            val targetShardBefore = net.getShardAccount(target);
            expect(targetShardBefore).toBeNotNull();

            val sameAccount = net.getAccount(target);
            net.setAccount(target, sameAccount);

            val targetShardAfter = net.getShardAccount(target);
            expect(targetShardAfter).toBeNotNull();
            expect(targetShardAfter!.lastTransLt).toEqual(targetShardBefore!.lastTransLt);
            expect(targetShardAfter!.lastTransHash).toEqual(targetShardBefore!.lastTransHash);
        }
        "#,
        "integration/snapshots/test_lib_agent_r/r_lib_api_set_account_preserves_shard_markers_for_existing_address.stdout.txt",
    );
}

#[test]
fn r_lib_api_set_shard_account_copies_state_between_addresses() {
    run_account_state_case(
        "r-lib-api-set-shard-account-copies-state",
        r#"
        get fun `test-set-shard-account-copies-state-between-addresses`() {
            val source = net.randomAddress("r-shard-source");
            val target = net.randomAddress("r-shard-target");

            net.topUp(source, ton("1"));
            net.topUp(source, ton("2"));

            expect(net.balance(target)).toEqual(0);

            val sourceShard = net.getShardAccount(source);
            expect(sourceShard).toBeNotNull();
            val expectedLastLt = sourceShard!.lastTransLt;
            val expectedLastHash = sourceShard!.lastTransHash;
            val expectedBalance = net.balance(source);

            net.setShardAccount(target, sourceShard);

            val targetShard = net.getShardAccount(target);
            expect(targetShard).toBeNotNull();
            expect(targetShard!.lastTransLt).toEqual(expectedLastLt);
            expect(targetShard!.lastTransHash).toEqual(expectedLastHash);
            expect(net.balance(target)).toEqual(expectedBalance);
        }
        "#,
        "integration/snapshots/test_lib_agent_r/r_lib_api_set_shard_account_copies_state_between_addresses.stdout.txt",
    );
}

#[test]
fn r_lib_api_get_account_state_for_fresh_address_returns_non_null_bug() {
    run_account_state_failure_case(
        "r-lib-api-get-account-state-fresh-address-bug",
        r#"
        get fun `test-get-account-state-for-fresh-address-should-be-null`() {
            val target = net.randomAddress("r-fresh-account-state");
            // BUG: net.getAccountState(randomAddress) returns non-null default-like account data; expected null for undeployed account.
            expect(net.getAccountState(target)).toBeNull();
        }
        "#,
        "integration/snapshots/test_lib_agent_r/r_lib_api_get_account_state_for_fresh_address_returns_non_null_bug.stdout.txt",
    );
}
