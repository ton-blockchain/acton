use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EM_MATCHES_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/tlb/either"
import "../../lib/tlb/maybe"
import "../../lib/types/message"

struct (0xE4000001) EmNotice {
    queryId: uint64
}

struct (0xE4000002) EmOtherNotice {
    queryId: uint64
}

fun emExternalOutMessage(
    src: address,
    dest: any_address,
    body: RemainingBitsAndRefs,
): Message<RemainingBitsAndRefs, ExternalOutMessageInfo> {
    return Message<RemainingBitsAndRefs, ExternalOutMessageInfo> {
        info: ExternalOutMessageInfo {
            src,
            dest,
            createdLt: 1,
            createdAt: 2,
        },
        init: Maybe<Either<StateInit, Cell<StateInit>>>.none(),
        body: EitherLeft {
            value: body,
        },
    };
}
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EM_MATCHES_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("em_message_matches", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn external_out_message_matches_src_to_and_body_prefix() {
    run_success_case(
        "em-stdlib-message-matches-external-out-filters",
        r#"
get fun `test-em-message-matches-external-out-filters`() {
    val src = address("0:00000000000000000000000000000000000000000000000000000000000000E1");
    val wrongSrc = address("0:00000000000000000000000000000000000000000000000000000000000000E2");
    val dest = createAddressNone();
    val wrongDest = address("0:00000000000000000000000000000000000000000000000000000000000000E3")
        as any_address;

    val msg = emExternalOutMessage(
        src,
        dest,
        EmNotice { queryId: 77 }.toCell().beginParse(),
    );

    expect(msg.matches({ from: src, to: dest })).toBeTrue();
    expect(msg.matches({ from: wrongSrc, to: dest })).toBeFalse();
    expect(msg.matches({ from: src, to: wrongDest })).toBeFalse();
    expect(msg.matches<EmNotice>({ from: src, to: dest })).toBeTrue();
    expect(msg.matches<EmOtherNotice>({ from: src, to: dest })).toBeFalse();
}
"#,
        "integration/snapshots/test-runner/external_out_message_matches_src_to_and_body_prefix/external_out_message_matches_src_to_and_body_prefix_bug.stdout.txt",
    );
}
