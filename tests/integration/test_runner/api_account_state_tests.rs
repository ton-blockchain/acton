use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const TEST_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

fn run_account_state_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_code = format!(
        r"
            {TEST_IMPORTS}

            {test_body}
        "
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

#[test]
fn top_up_materializes_account_and_increases_balance() {
    run_account_state_case(
        "r-lib-api-top-up-materializes-account",
        r#"
        get fun `test top up materializes account and increases balance`() {
            val target = randomAddress("r-top-up-target");
            val initialBalance = testing.getAccountBalance(target);

            testing.topUp(target, ton("1"));
            val firstBalance = testing.getAccountBalance(target);
            expect(firstBalance).toBeGreater(initialBalance);

            testing.topUp(target, ton("2"));
            val secondBalance = testing.getAccountBalance(target);
            expect(secondBalance).toBeGreater(firstBalance);
        }
        "#,
        "integration/snapshots/test-runner/api_account_state/top_up_materializes_account_and_increases_balance.stdout.txt",
    );
}

#[test]
fn set_shard_account_null_resets_state_and_balance() {
    run_account_state_case(
        "r-lib-api-set-shard-account-null",
        r#"
        get fun `test set shard account null resets state and balance`() {
            val target = randomAddress("r-shard-reset-target");
            testing.topUp(target, ton("1"));

            val balanceAfterTopUp = testing.getAccountBalance(target);
            expect(balanceAfterTopUp).toBeGreater(0);

            testing.setShardAccount(target, null);

            expect(testing.getAccountBalance(target)).toEqual(0);

            val shardAfterReset = testing.getShardAccount(target);
            expect(shardAfterReset).toBeNotNull();
            expect(shardAfterReset!.lastTransLt).toEqual(0);
            expect(shardAfterReset!.lastTransHash).toEqual(0);
        }
        "#,
        "integration/snapshots/test-runner/api_account_state/set_shard_account_null_resets_state_and_balance.stdout.txt",
    );
}

#[test]
fn set_account_preserves_shard_markers_for_existing_address() {
    run_account_state_case(
        "r-lib-api-set-account-preserves-shard-markers",
        r#"
        get fun `test set account preserves shard markers`() {
            val target = randomAddress("r-set-account-target");

            testing.topUp(target, ton("1"));
            testing.topUp(target, ton("1"));

            val targetShardBefore = testing.getShardAccount(target);
            expect(targetShardBefore).toBeNotNull();

            testing.setShardAccount(target, targetShardBefore);

            val targetShardAfter = testing.getShardAccount(target);
            expect(targetShardAfter).toBeNotNull();
            expect(targetShardAfter!.lastTransLt).toEqual(targetShardBefore!.lastTransLt);
            expect(targetShardAfter!.lastTransHash).toEqual(targetShardBefore!.lastTransHash);
        }
        "#,
        "integration/snapshots/test-runner/api_account_state/set_account_preserves_shard_markers_for_existing_address.stdout.txt",
    );
}

#[test]
fn set_shard_account_copies_state_between_addresses() {
    run_account_state_case(
        "r-lib-api-set-shard-account-copies-state",
        r#"
        get fun `test set shard account copies state between addresses`() {
            val source = randomAddress("r-shard-source");
            val target = randomAddress("r-shard-target");

            testing.topUp(source, ton("1"));
            testing.topUp(source, ton("2"));

            expect(testing.getAccountBalance(target)).toEqual(0);

            val sourceShard = testing.getShardAccount(source);
            expect(sourceShard).toBeNotNull();
            val expectedLastLt = sourceShard!.lastTransLt;
            val expectedLastHash = sourceShard!.lastTransHash;
            val expectedBalance = testing.getAccountBalance(source);

            testing.setShardAccount(target, sourceShard);

            val targetShard = testing.getShardAccount(target);
            expect(targetShard).toBeNotNull();
            expect(targetShard!.lastTransLt).toEqual(expectedLastLt);
            expect(targetShard!.lastTransHash).toEqual(expectedLastHash);
            expect(testing.getAccountBalance(target)).toEqual(expectedBalance);
        }
        "#,
        "integration/snapshots/test-runner/api_account_state/set_shard_account_copies_state_between_addresses.stdout.txt",
    );
}

#[test]
fn get_account_state_for_fresh_address_is_null_before_top_up() {
    run_account_state_case(
        "r-lib-api-get-account-state-fresh-address",
        r#"
        get fun `test get account state for fresh address should be null`() {
            val target = randomAddress("r-fresh-account-state");
            val before = testing.getAccountState(target);
            expect(before == null).toEqual(true);

            testing.topUp(target, ton("1"));
            val after = testing.getAccountState(target);
            expect(after == null).toEqual(false);
            expect(after!.storage.balance.grams).toEqual(ton("1"));
        }
        "#,
        "integration/snapshots/test-runner/api_account_state/get_account_state_for_fresh_address_is_null_before_top_up.stdout.txt",
    );
}
