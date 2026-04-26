use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CX_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
import "../../lib/emulation/testing"

fun changeLib(code: cell, mode: int): void asm "SETLIBCODE"
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CX_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("out_actions", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn change_library_decoding_via_parse_out_actions() {
    run_success(
        "cx-stdlib-change-library-decoding-via-parse-out-actions",
        r#"
get fun `test cx change library decoding via parse out actions`() {
    val libraryCell = beginCell().storeUint(0xC0DECAFE, 32).endCell();
    changeLib(libraryCell, 2);

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(1);

    val parsedAction = parsed.at(0);
    expect(parsedAction.kind()).toEqual("change-library");
    expect(parsedAction is TlbOutActionChangeLibrary).toBeTrue();

    if (parsedAction is TlbOutActionChangeLibrary) {
        expect(parsedAction.mode).toEqual(2);
        expect(parsedAction.libref is TlbLibRefRef).toBeTrue();
        if (parsedAction.libref is TlbLibRefRef) {
            expect(parsedAction.libref.library).toEqual(libraryCell);
        }
    }
}
"#,
        "integration/snapshots/test-runner/change_library_decoding_via_parse_out_actions/change_library_decoding_via_parse_out_actions.stdout.txt",
    );
}
