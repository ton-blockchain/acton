//! Reserved integration test module for subagent CJ.
//!
//! Ownership boundary for agent CJ:
//! - tests/integration/test_std_agent_cj_tests.rs
//! - tests/integration/snapshots/test_std_agent_cj/**
//! - tests/integration/testdata/test_std_agent_cj/**
//! - tests/support/test_std_agent_cj/** (optional)
//!
//! Required test name prefix:
//! - cj_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CJ_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"

fun changeLib(code: cell, mode: int): void asm "SETLIBCODE"
"#;

fn run_cj_stdlib_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CJ_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("out_actions", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_cj_stdlib_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CJ_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("out_actions", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cj_stdlib_parse_out_actions_preserves_reserve_and_change_library_kinds() {
    run_cj_stdlib_success(
        "cj-stdlib-parse-out-actions-preserves-kinds",
        r#"
get fun `test-cj-parse-out-actions-preserves-kinds`() {
    reserveToncoinsOnBalance(
        ton("0.05"),
        RESERVE_MODE_ALL_BUT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL
    );
    changeLib(beginCell().storeUint(0xAB, 8).endCell(), 2);

    val viaVm = vm.outActions();
    val viaRaw = vm.parseOutActions(vm.getC5());

    expect(viaVm.size()).toEqual(2);
    expect(viaRaw.size()).toEqual(2);

    expect(viaVm.at(0).kind()).toEqual("change-library");
    expect(viaVm.at(1).kind()).toEqual("reserve-currency");
    expect(viaRaw.at(0).kind()).toEqual("change-library");
    expect(viaRaw.at(1).kind()).toEqual("reserve-currency");
}
"#,
        "integration/snapshots/test_std_agent_cj/cj_stdlib_parse_out_actions_preserves_reserve_and_change_library_kinds.stdout.txt",
    );
}

#[test]
fn cj_stdlib_parse_out_actions_reserve_nanoton_is_misparsed_as_change_library_bug() {
    run_cj_stdlib_failure(
        "cj-stdlib-parse-out-actions-reserve-one-nanoton-kind-bug",
        r#"
get fun `test-cj-parse-out-actions-reserve-one-nanoton-kind-bug`() {
    reserveToncoinsOnBalance(1, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

    val parsed = vm.parseOutActions(vm.getC5());
    expect(parsed.size()).toEqual(1);

    // BUG: parseOutActions routes reserve-currency tuple with grams=1 through change-library decoding; expected reserve-currency, got change-library.
    expect(parsed.at(0).kind()).toEqual("reserve-currency");
}
"#,
        "integration/snapshots/test_std_agent_cj/cj_stdlib_parse_out_actions_reserve_nanoton_is_misparsed_as_change_library_bug.stdout.txt",
    );
}
