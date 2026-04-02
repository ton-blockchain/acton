use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const LEDGER_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun balances(): map<int32, int32> {
    var m = createEmptyMap<int32, int32>();
    m.set(1, 10);
    m.set(2, 20);
    return m;
}

get fun emptyBalances(): map<int32, int32> {
    var m = createEmptyMap<int32, int32>();
    return m;
}

get fun duplicateValues(): map<int32, int32> {
    var m = createEmptyMap<int32, int32>();
    m.set(1, 77);
    m.set(2, 77);
    m.set(3, 88);
    return m;
}

get fun updatedBalances(): map<int32, int32> {
    var m = createEmptyMap<int32, int32>();
    m.set(7, 10);
    m.set(8, 30);
    m.set(7, 90);
    return m;
}
";

const MAP_TEST_PREPARE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/build/build"
import "../../lib/emulation/network"

struct Ledger {
    address: address
    init: ContractState
}

fun Ledger.fromStorage() {
    val init = ContractState {
        code: build("ledger"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Ledger { address, init };
}

fun deployLedger(): Ledger {
    val ledger = Ledger.fromStorage();
    val deployer = net.treasury("deployer");
    val deployMsg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: ledger.init,
        },
    });

    val deployTxs = net.send(deployer.address, deployMsg);
    expect(deployTxs).toHaveSuccessfulDeploy({ to: ledger.address });

    return ledger;
}
"#;

fn map_project(name: &str, test_cases: &str) -> crate::support::project::Project {
    let test_source = format!("{MAP_TEST_PREPARE}\n{test_cases}");
    ProjectBuilder::new(name)
        .contract("ledger", LEDGER_CONTRACT)
        .test_file("map", &test_source)
        .build()
}

#[test]
fn map_contains_and_absence_matchers_pass_for_contract_side_map() {
    map_project(
        "l-lib-api-map-contains-pass",
        r#"
        get fun `test-map-contains-and-absence`() {
            val ledger = deployLedger();
            val balances: map<int32, int32> = net.runGetMethod(ledger.address, "balances");

            expect(balances).toContainKey(1);
            expect(balances).toContainValue(20);
            expect(balances).toNotContainKey(99);
            expect(balances).toNotContainValue(777);
            expect(balances).toBeNonEmpty();
            expect(balances).toHaveLength(2);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/api_map/map_contains_and_absence_matchers_pass_for_contract_side_map.stdout.txt",
    );
}

#[test]
fn map_empty_matchers_pass_for_empty_contract_side_map() {
    map_project(
        "l-lib-api-map-empty-pass",
        r#"
        get fun `test-map-empty-matchers`() {
            val ledger = deployLedger();
            val balances: map<int32, int32> = net.runGetMethod(ledger.address, "emptyBalances");

            expect(balances).toBeEmpty();
            expect(balances).toNotContainKey(1);
            expect(balances).toNotContainValue(10);
            expect(balances).toHaveLength(0);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/api_map/map_empty_matchers_pass_for_empty_contract_side_map.stdout.txt",
    );
}

#[test]
fn map_have_length_counts_unique_keys_after_overwrite() {
    map_project(
        "l-lib-api-map-length-overwrite-pass",
        r#"
        get fun `test-map-length-after-overwrite`() {
            val ledger = deployLedger();
            val balances: map<int32, int32> = net.runGetMethod(ledger.address, "updatedBalances");

            expect(balances).toContainKey(7);
            expect(balances).toContainValue(90);
            expect(balances).toNotContainValue(10);
            expect(balances).toBeNonEmpty();
            expect(balances).toHaveLength(2);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/api_map/map_have_length_counts_unique_keys_after_overwrite.stdout.txt",
    );
}

#[test]
fn map_to_contain_key_failure_reports_missing_key() {
    map_project(
        "l-lib-api-map-contain-key-fail",
        r#"
        get fun `test-map-missing-key-fails`() {
            val ledger = deployLedger();
            val balances: map<int32, int32> = net.runGetMethod(ledger.address, "balances");

            expect(balances).toContainKey(404);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/api_map/map_to_contain_key_failure_reports_missing_key.stdout.txt",
    );
}

#[test]
fn map_to_not_contain_value_failure_reports_present_value() {
    map_project(
        "l-lib-api-map-not-contain-value-fail",
        r#"
        get fun `test-map-not-contain-value-fails`() {
            val ledger = deployLedger();
            val balances: map<int32, int32> = net.runGetMethod(ledger.address, "duplicateValues");

            expect(balances).toNotContainValue(77);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/api_map/map_to_not_contain_value_failure_reports_present_value.stdout.txt",
    );
}
