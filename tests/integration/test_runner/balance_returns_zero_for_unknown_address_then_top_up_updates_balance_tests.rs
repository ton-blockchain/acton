use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/io"
"#;

fn run_case(project_name: &str, test_body: &str, snapshot_path: &str, contains: &[&str]) {
    let test_source = format!("{NETWORK_IMPORTS}\n{test_body}\n");

    let output = ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("network", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .success();

    output.assert_passed(1);
    for needle in contains {
        output.assert_contains(needle);
    }
    output.assert_snapshot_matches(snapshot_path);
}

#[test]
fn balance_returns_zero_for_unknown_address_then_top_up_updates_balance() {
    run_case(
        "ah-stdlib-balance-zero-then-top-up",
        r#"
get fun `test-ah-stdlib-balance-zero-then-top-up`() {
    val target = net.randomAddress("ah_balance_target_zero_then_top_up");
    expect(net.balance(target)).toEqual(0);

    net.topUp(target, ton("1"));
    expect(net.balance(target)).toEqual(ton("1"));
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/balance_returns_zero_for_unknown_address_then_top_up_updates_balance.stdout.txt",
        &[],
    );
}

#[test]
fn top_up_is_additive_for_the_same_address() {
    run_case(
        "ah-stdlib-top-up-additive",
        r#"
get fun `test-ah-stdlib-top-up-is-additive`() {
    val target = net.randomAddress("ah_top_up_additive_target");

    net.topUp(target, ton("1"));
    net.topUp(target, ton("2"));

    expect(net.balance(target)).toEqual(ton("3"));
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/top_up_is_additive_for_the_same_address.stdout.txt",
        &[],
    );
}

#[test]
fn set_account_on_fresh_target_preserves_shard_markers() {
    run_case(
        "ah-stdlib-set-account-fresh-target-preserves-markers",
        r#"
get fun `test-ah-stdlib-set-account-fresh-target-preserves-markers`() {
    val target = net.randomAddress("ah_set_account_target_fresh");

    val beforeShard = net.getShardAccount(target);
    expect(beforeShard).toBeNotNull();

    val beforeLt = beforeShard!.lastTransLt;
    val beforeHash = beforeShard!.lastTransHash;
    val beforeBalance = net.balance(target);

    net.setAccount(target, net.getAccount(target));

    val targetShard = net.getShardAccount(target);
    expect(targetShard).toBeNotNull();
    expect(targetShard!.lastTransLt).toEqual(beforeLt);
    expect(targetShard!.lastTransHash).toEqual(beforeHash);
    expect(net.balance(target)).toEqual(beforeBalance);
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/set_account_on_fresh_target_preserves_shard_markers.stdout.txt",
        &[],
    );
}

#[test]
fn set_account_preserves_existing_shard_markers_and_balance() {
    run_case(
        "ah-stdlib-set-account-preserves-markers-and-balance",
        r#"
get fun `test-ah-stdlib-set-account-preserves-existing-shard-markers-and-balance`() {
    val target = net.randomAddress("ah_set_account_existing_target");

    net.topUp(target, ton("1"));
    net.topUp(target, ton("1"));

    val beforeShard = net.getShardAccount(target);
    expect(beforeShard).toBeNotNull();

    val beforeLt = beforeShard!.lastTransLt;
    val beforeHash = beforeShard!.lastTransHash;
    val beforeBalance = net.balance(target);

    net.setAccount(target, net.getAccount(target));

    val afterShard = net.getShardAccount(target);
    expect(afterShard).toBeNotNull();
    expect(afterShard!.lastTransLt).toEqual(beforeLt);
    expect(afterShard!.lastTransHash).toEqual(beforeHash);
    // BUG: net.getAccount(target) after topUp behaves like AccountNone in setAccount path; expected balance to stay unchanged, got zeroed balance.
    expect(net.balance(target)).toEqual(beforeBalance);
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/set_account_preserves_existing_shard_markers_and_balance.stdout.txt",
        &[],
    );
}

#[test]
fn set_shard_account_copies_markers_and_balance_between_addresses() {
    run_case(
        "ah-stdlib-set-shard-account-copy-state",
        r#"
get fun `test-ah-stdlib-set-shard-account-copy-state`() {
    val source = net.randomAddress("ah_set_shard_source");
    val target = net.randomAddress("ah_set_shard_target");

    net.topUp(source, ton("1"));
    net.topUp(source, ton("2"));

    val sourceShard = net.getShardAccount(source);
    expect(sourceShard).toBeNotNull();

    val expectedLt = sourceShard!.lastTransLt;
    val expectedHash = sourceShard!.lastTransHash;
    val expectedBalance = net.balance(source);

    net.setShardAccount(target, sourceShard);

    val targetShard = net.getShardAccount(target);
    expect(targetShard).toBeNotNull();
    expect(targetShard!.lastTransLt).toEqual(expectedLt);
    expect(targetShard!.lastTransHash).toEqual(expectedHash);
    expect(net.balance(target)).toEqual(expectedBalance);
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/set_shard_account_copies_markers_and_balance_between_addresses.stdout.txt",
        &[],
    );
}

#[test]
fn set_shard_account_null_resets_balance_and_markers() {
    run_case(
        "ah-stdlib-set-shard-account-null-reset",
        r#"
get fun `test-ah-stdlib-set-shard-account-null-reset`() {
    val target = net.randomAddress("ah_set_shard_null_target");
    net.topUp(target, ton("1"));

    val before = net.getShardAccount(target);
    expect(before).toBeNotNull();
    expect(before!.lastTransLt as int).toBeGreater(0);

    net.setShardAccount(target, null);

    val after = net.getShardAccount(target);
    expect(after).toBeNotNull();
    expect(after!.lastTransLt as int).toEqual(0);
    expect(after!.lastTransHash as int).toEqual(0);
    expect(net.balance(target)).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/set_shard_account_null_resets_balance_and_markers.stdout.txt",
        &[],
    );
}

#[test]
fn register_address_name_is_used_in_transaction_output() {
    run_case(
        "ah-stdlib-register-address-output-name",
        r#"
get fun `test-ah-stdlib-register-address-output-name`() {
    val deployer = net.treasury("ah_register_address_deployer");
    val target = address("0:0000000000000000000000000000000000000000000000000000000000000011");

    net.registerAddress(target, "ah_registered_target");

    val msg = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: target,
    });

    val res = net.send(deployer.address, msg);
    expect(res.size()).toBeGreater(0);
    println(res);
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/register_address_name_is_used_in_transaction_output.stdout.txt",
        &["ah_registered_target"],
    );
}

#[test]
fn register_code_cell_name_is_used_for_auto_deploy_output() {
    run_case(
        "ah-stdlib-register-code-cell-output-name",
        r#"
get fun `test-ah-stdlib-register-code-cell-output-name`() {
    val deployer = net.treasury("ah_register_code_deployer");
    val code = build("simple");

    net.registerCodeCell(code, "ah_registered_simple_code");

    val init = ContractState {
        code: code,
        data: createEmptyCell(),
    };
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    });

    val res = net.send(deployer.address, msg);
    expect(res.size()).toBeGreater(0);
    println(res);
}
"#,
        "integration/snapshots/test-runner/balance_returns_zero_for_unknown_address_then_top_up_updates_balance/register_code_cell_name_is_used_for_auto_deploy_output.stdout.txt",
        &["ah_registered_simple_code"],
    );
}
