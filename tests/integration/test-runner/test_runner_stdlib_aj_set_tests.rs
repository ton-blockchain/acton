//! Reserved integration test module for subagent AJ.
//!
//! Ownership boundary for agent AJ:
//! - tests/integration/test-runner/test_runner_stdlib_aj_set_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/**
//! - tests/integration/testdata/test_std_agent_aj/**
//! - tests/support/test_std_agent_aj/** (optional)
//!
//! Required test name prefix:
//! - aj_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const VM_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/fmt"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

fn run_aj_vm_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_source = format!("{VM_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("aj_vm_helpers", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_aj_vm_failure_case(
    project_name: &str,
    test_body: &str,
    expected_message: &str,
    snapshot_path: &str,
) {
    let test_source = format!("{VM_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("aj_vm_helpers", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains(expected_message)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn set_time_and_logical_time_update_c7_slots() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-set-time-and-logical-slots-3-4-5",
        r#"
get fun `test-aj-stdlib-vm-set-time-and-logical-slots-3-4-5`() {
    vm.setTime(1700001234);
    vm.setBlockLogicalTime(123456789);
    vm.setLogicalTime(223456789);

    val c7 = vm.getC7();
    val params = c7.get(0) as tuple;

    expect(params.get(3) as int).toEqual(1700001234);
    expect(params.get(4) as int).toEqual(123456789);
    expect(params.get(5) as int).toEqual(223456789);
    expect(blockchain.now()).toEqual(1700001234);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_set_time_and_logical_time_update_c7_slots.stdout.txt",
    );
}

#[test]
fn set_original_balance_updates_balance_tuple_with_and_without_extra_dict() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-set-original-balance-slot-7",
        r#"
get fun `test-aj-stdlib-vm-set-original-balance-slot-7`() {
    vm.setOriginalBalance(ton("3"));
    var c7 = vm.getC7();
    var params = c7.get(0) as tuple;
    val withoutExtra = params.get(7) as tuple;
    expect(withoutExtra.get(0) as int).toEqual(ton("3"));
    expect(withoutExtra.get(1) as dict?).toBeNull();

    val extraCurrencies = net.getConfig().toLowLevelDict();
    vm.setOriginalBalance(ton("5"), extraCurrencies);
    c7 = vm.getC7();
    params = c7.get(0) as tuple;
    val withExtra = params.get(7) as tuple;
    expect(withExtra.get(0) as int).toEqual(ton("5"));
    expect(withExtra.get(1) as dict?).toBeNotNull();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_set_original_balance_updates_balance_tuple_with_and_without_extra_dict.stdout.txt",
    );
}

#[test]
fn set_config_root_dict_replaces_c7_root_config_slot() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-set-config-root-slot-9",
        r#"
get fun `test-aj-stdlib-vm-set-config-root-slot-9`() {
    var config = net.getConfig();
    var version = config.getGlobalVersion();
    version.version += 1;
    config.setGlobalVersion(version);

    val root = config.toLowLevelDict();
    vm.setConfigRootDict(root);

    val c7 = vm.getC7();
    val params = c7.get(0) as tuple;
    val c7Root = params.get(9) as Config;
    val c7Version = c7Root.getGlobalVersion();

    expect(c7Version.version).toEqual(version.version);
    expect(c7Version.capabilities).toEqual(version.capabilities);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_set_config_root_dict_replaces_c7_root_config_slot.stdout.txt",
    );
}

#[test]
fn set_and_get_config_unpacked_round_trip_slot_14() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-config-unpacked-slot-14",
        r#"
get fun `test-aj-stdlib-vm-config-unpacked-slot-14`() {
    var unpacked = createEmptyTuple();
    unpacked.push(777);
    unpacked.push("aj-unpacked");
    unpacked.push(false);

    vm.setConfigUnpacked(unpacked);
    val actual = vm.getConfigUnpacked();

    expect(actual.size()).toEqual(3);
    expect(actual.get(0) as int).toEqual(777);
    expect(actual.get(1) as string).toEqual("aj-unpacked");
    expect(actual.get(2) as bool).toEqual(false);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_set_and_get_config_unpacked_round_trip_slot_14.stdout.txt",
    );
}

#[test]
fn register_library_accepts_code_and_empty_cells() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-register-library",
        r#"
get fun `test-aj-stdlib-vm-register-library`() {
    val codeCell = build("simple");

    vm.registerLibrary(codeCell);
    vm.registerLibrary(createEmptyCell());

    expect(true).toBeTrue();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_register_library_accepts_code_and_empty_cells.stdout.txt",
    );
}

#[test]
fn convert_address_supports_raw_and_user_friendly_forms() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-convert-address-valid",
        r#"
get fun `test-aj-stdlib-vm-convert-address-valid`() {
    val raw = "0:0000000000000000000000000000000000000000000000000000000000000000";
    val friendly = "EQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAM9c";

    val fromRaw = vm.convertAddress(raw);
    val fromFriendly = vm.convertAddress(friendly);

    val renderedRaw = format1("{}", fromRaw);
    val renderedFriendly = format1("{}", fromFriendly);

    expect(renderedRaw).toEqual(renderedFriendly);
    expect(renderedRaw).toEqual("kQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAHTW");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_convert_address_supports_raw_and_user_friendly_forms.stdout.txt",
    );
}

#[test]
fn convert_address_reports_invalid_input() {
    run_aj_vm_failure_case(
        "aj-stdlib-vm-convert-address-invalid",
        r#"
get fun `test-aj-stdlib-vm-convert-address-invalid`() {
    val _ = vm.convertAddress("not-an-address");
}
"#,
        "Failed to convert address from not-an-address",
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_convert_address_reports_invalid_input.stdout.txt",
    );
}

#[test]
fn get_config_param_generic_is_not_usable_bug() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-get-config-param-generic-bug",
        r#"
get fun `test-aj-stdlib-vm-get-config-param-generic-bug`() {
    vm.setTime(1700001000);
    val now = vm.getConfigParam<int>(3);
    expect(now).toEqual(1700001000);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_get_config_param_generic_is_not_usable_bug.stdout.txt",
    );
}

#[test]
fn cell_from_hex_decodes_valid_boc_hex() {
    run_aj_vm_success_case(
        "aj-stdlib-vm-cell-from-hex-valid",
        r#"
get fun `test-aj-stdlib-vm-cell-from-hex-valid`() {
    val decoded = vm.cellFromHex("b5ee9c72010101010002000000");
    expect(decoded).toEqual(createEmptyCell());
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_cell_from_hex_decodes_valid_boc_hex.stdout.txt",
    );
}

#[test]
fn cell_from_hex_reports_invalid_hex() {
    run_aj_vm_failure_case(
        "aj-stdlib-vm-cell-from-hex-invalid",
        r#"
get fun `test-aj-stdlib-vm-cell-from-hex-invalid`() {
    val _ = vm.cellFromHex("deadbeef");
}
"#,
        "Failed to decode cell hex deadbeef",
        "integration/snapshots/test-runner/test_runner_stdlib_aj_set_tests/aj_stdlib_cell_from_hex_reports_invalid_hex.stdout.txt",
    );
}
