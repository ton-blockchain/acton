//! Reserved for agent-cx.
//! Prefix: cx_stdlib_
//! Ownership: this file and tests/integration/snapshots/test_std_agent_cx/**
//! Agent will add targeted stdlib integration tests here.

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CX_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"

fun changeLib(code: cell, mode: int): void asm "SETLIBCODE"
"#;

fn run_cx_stdlib_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CX_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
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
fn cx_stdlib_change_library_to_tuple_roundtrip_via_parse_out_actions_is_broken_bug() {
    run_cx_stdlib_failure(
        "cx-stdlib-change-library-to-tuple-roundtrip-bug",
        r#"
get fun `test-cx-change-library-to-tuple-roundtrip-bug`() {
    val libraryCell = beginCell().storeUint(0xC0DECAFE, 32).endCell();
    changeLib(libraryCell, 2);

    val parsed = vm.parseOutActions(vm.getC5());
    expect(parsed.size()).toEqual(1);

    val parsedAction = parsed.at(0);
    expect(parsedAction.kind()).toEqual("change-library");
    expect(parsedAction is OutActionChangeLibrary).toBeTrue();

    if (parsedAction is OutActionChangeLibrary) {
        // BUG: OutActionChangeLibrary.toTuple should roundtrip through OutAction.fromTuple
        // after vm.parseOutActions, but parsing the produced tuple aborts with exit_code=7
        // ("not an integer").
        val restored = OutAction.fromTuple(parsedAction.toTuple());
        expect(restored.kind()).toEqual("change-library");
    }
}
"#,
        "integration/snapshots/test_std_agent_cx/cx_stdlib_change_library_to_tuple_roundtrip_via_parse_out_actions_is_broken_bug.stdout.txt",
    );
}
