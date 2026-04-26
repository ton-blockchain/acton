use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

const NETWORK_IMPORTS_WITH_TRANSACTION: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/transaction"
"#;

fn run_network_success_case_with_imports(
    imports: &str,
    project_name: &str,
    test_body: &str,
    snapshot_path: &str,
) {
    let source = format!("{imports}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("network_storage_fee_missing", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_network_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    run_network_success_case_with_imports(NETWORK_IMPORTS, project_name, test_body, snapshot_path);
}

#[test]
fn network_get_account_storage_fee_returns_null_for_missing_account() {
    run_network_success_case(
        "bk-stdlib-network-get-account-storage-fee-missing-account",
        r#"
get fun `test bk stdlib network get account storage fee missing account`() {
    val missing = randomAddress("bk_missing_storage_fee_account");
    expect(testing.getAccountState(missing) == null).toBeTrue();
    expect(testing.getAccountStorageFee(missing, 86400) == null).toBeTrue();
    expect(testing.getAccountStorageFee(missing, 0) == null).toBeTrue();
}
"#,
        "integration/snapshots/test-runner/network_get_account_storage_fee_returns_null_for_missing_account/network_get_account_storage_fee_returns_null_for_missing_account.stdout.txt",
    );
}

#[test]
fn network_get_account_storage_fee_returns_non_null_for_existing_account_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let source = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"

get fun `test bk stdlib network get account storage fee existing account`() {
    val seconds = 86400;
    val treasury = testing.treasury("bk_storage_fee_sender");

    val storageFee = testing.getAccountStorageFee(treasury.address, seconds);
    expect(storageFee != null).toBeTrue();

    val zeroFee = testing.getAccountStorageFee(treasury.address, 0);
    expect(zeroFee != null).toBeTrue();
    expect(zeroFee!).toEqual(0);
    expect(storageFee! >= zeroFee!).toBeTrue();
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
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/network_get_account_storage_fee_returns_null_for_missing_account/network_get_account_storage_fee_returns_non_null_for_existing_account_in_fixture_project.stdout.txt",
        );
}

#[test]
fn account_state_variants_uninit_and_active_are_parsed_from_accounts() {
    run_network_success_case_with_imports(
        NETWORK_IMPORTS_WITH_TRANSACTION,
        "bk-stdlib-account-state-uninit-and-active-variants",
        r#"
get fun `test bk stdlib account state uninit and active variants`() {
    val uninitAddr = randomAddress("bk_state_variant_uninit_addr");
    testing.topUp(uninitAddr, ton("1"));

    val uninitAcc = testing.getShardAccount(uninitAddr)!.account.load();
    expect(uninitAcc is TlbAccountInfo).toBeTrue();
    if (uninitAcc is TlbAccountInfo) {
        expect(uninitAcc.storage.state is TlbAccountStateUninit).toBeTrue();
    }

    val treasury = testing.treasury("bk_state_variant_active_treasury");
    val activeAcc = testing.getShardAccount(treasury.address)!.account.load();
    expect(activeAcc is TlbAccountInfo).toBeTrue();
    if (activeAcc is TlbAccountInfo) {
        expect(activeAcc.storage.state is TlbAccountStateActive).toBeTrue();
    }
}
"#,
        "integration/snapshots/test-runner/network_get_account_storage_fee_returns_null_for_missing_account/account_state_variants_uninit_and_active_are_parsed_from_accounts.stdout.txt",
    );
}

#[test]
fn account_state_frozen_local_roundtrip_cell_works() {
    run_network_success_case_with_imports(
        NETWORK_IMPORTS_WITH_TRANSACTION,
        "bk-stdlib-account-state-frozen-local-roundtrip",
        r"
get fun `test bk stdlib account state frozen local roundtrip`() {
    val frozenHash = beginCell().storeUint(0x11, 32).endCell().hash();
    val frozen = TlbAccountStateFrozen { stateHash: frozenHash };
    val parsed = TlbAccountState.fromCell(frozen.toCell());

    expect(parsed is TlbAccountStateFrozen).toBeTrue();
    if (parsed is TlbAccountStateFrozen) {
        expect(parsed.stateHash).toEqual(frozenHash);
    }
}
",
        "integration/snapshots/test-runner/network_get_account_storage_fee_returns_null_for_missing_account/account_state_frozen_local_roundtrip_cell_works.stdout.txt",
    );
}

#[test]
fn account_state_frozen_roundtrip_via_set_account_bug() {
    run_network_success_case_with_imports(
        NETWORK_IMPORTS_WITH_TRANSACTION,
        "bk-stdlib-account-state-frozen-roundtrip-via-set-account-bug",
        r#"
get fun `test bk stdlib account state frozen roundtrip via set account bug`() {
    val baseAddr = randomAddress("bk_state_variant_frozen_base_addr");
    testing.topUp(baseAddr, ton("1"));

    val baseShard = testing.getShardAccount(baseAddr);
    expect(baseShard).toBeNotNull();
    val baseAcc = baseShard!.account.load();
    expect(baseAcc is TlbAccountInfo).toBeTrue();

    val frozenAddr = randomAddress("bk_state_variant_frozen_target_addr");
    val frozenHash = beginCell().storeUint(0x11, 32).endCell().hash();

    if (baseAcc is TlbAccountInfo) {
        val frozenAcc = TlbAccountInfo {
            addr: frozenAddr,
            storageStat: baseAcc.storageStat,
            storage: {
                lastTransLt: baseAcc.storage.lastTransLt,
                balance: baseAcc.storage.balance,
                state: TlbAccountStateFrozen { stateHash: frozenHash },
            },
        };
        testing.setShardAccount(
            frozenAddr,
            TlbShardAccount {
                account: (frozenAcc as TlbAccount).toCell(),
                lastTransHash: baseShard!.lastTransHash,
                lastTransLt: baseShard!.lastTransLt,
            },
        );
    }

    val frozenAccAfter = testing.getShardAccount(frozenAddr)!.account.load();
    expect(frozenAccAfter is TlbAccountInfo).toBeTrue();
    if (frozenAccAfter is TlbAccountInfo) {
        expect(frozenAccAfter.storage.state is TlbAccountStateFrozen).toBeTrue();
    }
}
"#,
        "integration/snapshots/test-runner/network_get_account_storage_fee_returns_null_for_missing_account/account_state_frozen_roundtrip_via_set_account_bug.stdout.txt",
    );
}
