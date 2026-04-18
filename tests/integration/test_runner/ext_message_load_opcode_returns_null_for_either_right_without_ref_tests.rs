use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const MESSAGE_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/tlb/either"
import "../../lib/tlb/maybe"
import "../../lib/types/message"

fun ciNoInit(): TlbMaybe<TlbEither<StateInit, Cell<StateInit>>> {
    return TlbMaybe.none();
}

fun ciExtInfo(): TlbExtMsgInfoRelaxed {
    return TlbExtMsgInfoRelaxed {
        src: address("0:00000000000000000000000000000000000000000000000000000000000000C1"),
        dest: address("0:00000000000000000000000000000000000000000000000000000000000000C2")
            as any_address,
        createdLt: 0,
        createdAt: 0,
    };
}
"#;

fn run_message_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{MESSAGE_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("message_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ext_message_load_opcode_returns_null_for_either_right_without_ref() {
    run_message_case(
        "ci-stdlib-ext-message-load-opcode-right-without-ref",
        r"
get fun `test ci stdlib ext message load opcode right without ref`() {
    val body = beginCell().storeBool(true).endCell().beginParse();

    val msg = TlbExtMessageRelaxedGeneric {
        info: ciExtInfo(),
        init: ciNoInit(),
        body,
    };

    expect(msg.loadOpcode()).toBeNull();
}
",
        "integration/snapshots/test-runner/ext_message_load_opcode_returns_null_for_either_right_without_ref/ext_message_load_opcode_returns_null_for_either_right_without_ref.stdout.txt",
    );
}
