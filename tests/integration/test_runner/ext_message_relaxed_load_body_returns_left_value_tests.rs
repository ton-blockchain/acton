use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXT_MESSAGE_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/tlb/either"
import "../../lib/tlb/maybe"
import "../../lib/types/message"

struct (0x43480001) ChExternalPayload {
    queryId: uint64
    amount: uint32
}

fun chNoInit(): Maybe<Either<StateInit, Cell<StateInit>>> {
    return Maybe<Either<StateInit, Cell<StateInit>>>.none();
}

fun chExtInfo(): ExtMsgInfoRelaxed {
    return ExtMsgInfoRelaxed {
        src: address("0:00000000000000000000000000000000000000000000000000000000000000CC"),
        dest: createAddressNone(),
        createdLt: 0,
        createdAt: 0,
    };
}
"#;

fn run_ext_message_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EXT_MESSAGE_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("ext_message_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ext_message_relaxed_load_body_returns_left_value() {
    run_ext_message_case(
        "ch-stdlib-ext-message-load-body-left",
        r#"
get fun `test-ch-stdlib-ext-message-load-body-left`() {
    val payload = ChExternalPayload {
        queryId: 41,
        amount: 900,
    };

    val msg = ExtMessageRelaxed<ChExternalPayload> {
        info: chExtInfo(),
        init: chNoInit(),
        body: Either<ChExternalPayload, Cell<ChExternalPayload>>.left(payload),
    };

    expect(msg.loadBody()).toEqual(payload);
}
"#,
        "integration/snapshots/test-runner/ext_message_relaxed_load_body_returns_left_value/ext_message_relaxed_load_body_returns_left_value.stdout.txt",
    );
}

#[test]
fn ext_message_relaxed_load_body_loads_right_cell_value() {
    run_ext_message_case(
        "ch-stdlib-ext-message-load-body-right",
        r#"
get fun `test-ch-stdlib-ext-message-load-body-right`() {
    val payload = ChExternalPayload {
        queryId: 77,
        amount: 321,
    };
    val payloadCell = payload.toCell() as Cell<ChExternalPayload>;

    val msg = ExtMessageRelaxed<ChExternalPayload> {
        info: chExtInfo(),
        init: chNoInit(),
        body: Either<ChExternalPayload, Cell<ChExternalPayload>>.right(payloadCell),
    };

    expect(msg.loadBody()).toEqual(payload);
}
"#,
        "integration/snapshots/test-runner/ext_message_relaxed_load_body_returns_left_value/ext_message_relaxed_load_body_loads_right_cell_value.stdout.txt",
    );
}
