use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const LEDGER_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun emptyBalances(): map<int32, int32> {
    return createEmptyMap<int32, int32>();
}

get fun singleBalance(): map<int32, int32> {
    var balances = createEmptyMap<int32, int32>();
    balances.set(7, 70);
    return balances;
}
";

const EE_MAP_TEST_PREPARE: &str = r#"
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
    val deployTxs = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("1.0"),
            dest: { stateInit: ledger.init },
        }),
    );
    expect(deployTxs).toHaveSuccessfulDeploy({ to: ledger.address });
    return ledger;
}
"#;

fn map_project(name: &str, test_cases: &str) -> crate::support::project::Project {
    let test_source = format!("{EE_MAP_TEST_PREPARE}\n{test_cases}");
    ProjectBuilder::new(name)
        .contract("ledger", LEDGER_CONTRACT)
        .test_file("map", &test_source)
        .build()
}

#[test]
fn expect_map_to_have_length_accepts_empty_and_single_entry_maps() {
    map_project(
        "ee-stdlib-map-to-have-length-empty-non-empty-boundaries",
        r#"
        get fun `test ee stdlib map to have length empty non empty boundaries`() {
            val ledger = deployLedger();

            val emptyBalances: map<int32, int32> = net.runGetMethod(ledger.address, "emptyBalances");
            expect(emptyBalances).toBeEmpty();
            expect(emptyBalances).toHaveLength(0);

            val oneEntryBalances: map<int32, int32> = net.runGetMethod(ledger.address, "singleBalance");
            expect(oneEntryBalances).toBeNonEmpty();
            expect(oneEntryBalances).toHaveLength(1);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/expect_map_to_have_length_accepts_empty_and_single_entry_maps/expect_map_to_have_length_accepts_empty_and_single_entry_maps.stdout.txt",
    );
}

#[test]
fn expect_map_to_have_length_reports_boundary_mismatch_for_single_entry_map() {
    map_project(
        "ee-stdlib-map-to-have-length-boundary-mismatch",
        r#"
        get fun `test ee stdlib map to have length boundary mismatch`() {
            val ledger = deployLedger();
            val oneEntryBalances: map<int32, int32> = net.runGetMethod(ledger.address, "singleBalance");

            expect(oneEntryBalances).toHaveLength(0);
        }
        "#,
    )
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/expect_map_to_have_length_accepts_empty_and_single_entry_maps/expect_map_to_have_length_reports_boundary_mismatch_for_single_entry_map.stdout.txt",
    );
}
