//! Reserved for agent-cz.
//! Prefix: cz_stdlib_
//! Ownership: this file and tests/integration/snapshots/test-runner/test_runner_stdlib_cz_change_tests/**
//! Agent-owned tests for change-library action decoding and LibRef variants.

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CZ_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/testing/assert"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"

fun changeLib(code: cell, mode: int): void asm "SETLIBCODE"
"#;

fn run_cz_stdlib_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CZ_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("change_library_decode", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn change_library_decodes_libref_ref_branch() {
    run_cz_stdlib_success(
        "cz-stdlib-change-library-decodes-libref-ref-branch",
        r#"
get fun `test-cz-change-library-decodes-libref-ref-branch`() {
    val expectedCell = beginCell()
        .storeUint(0xBEEF, 16)
        .storeUint(0xCAFE, 16)
        .endCell();

    changeLib(expectedCell, 2);

    val outActions = vm.parseOutActions(vm.getC5());
    expect(outActions.size()).toEqual(1);
    val action = outActions.at(0);
    expect(action.kind()).toEqual("change-library");
    expect(action is OutActionChangeLibrary).toBeTrue();

    if (action is OutActionChangeLibrary) {
        expect(action.mode).toEqual(2);
        expect(action.libref is LibRefRef).toBeTrue();
        if (action.libref is LibRefRef) {
            expect(action.libref.library).toEqual(expectedCell);
        } else {
            Assert.fail("expected LibRefRef");
        }
    }
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_cz_change_tests/cz_stdlib_change_library_decodes_libref_ref_branch.stdout.txt",
    );
}

#[test]
fn change_library_mode_is_preserved_for_remove_action() {
    run_cz_stdlib_success(
        "cz-stdlib-change-library-mode-preserved-remove",
        r#"
get fun `test-cz-change-library-mode-preserved-remove`() {
    val libCell = beginCell().storeUint(0xAA, 8).endCell();
    changeLib(libCell, 0);

    val outActions = vm.parseOutActions(vm.getC5());
    expect(outActions.size()).toEqual(1);
    val action = outActions.at(0);
    expect(action.kind()).toEqual("change-library");

    if (action is OutActionChangeLibrary) {
        expect(action.mode).toEqual(0);
    } else {
        Assert.fail("expected OutActionChangeLibrary");
    }
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_cz_change_tests/cz_stdlib_change_library_mode_is_preserved_for_remove_action.stdout.txt",
    );
}
