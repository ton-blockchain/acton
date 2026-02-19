use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CV_OUT_ACTION_IMPORTS: &str = r#"
import "@stdlib/reflection"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/types/message"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"

struct (0xC0DE0001) InlineParityPayload {
    queryId: uint64
    amount: uint32
}

struct (0xC0DE0002) RefParityPayload {
    queryId: uint64
    part1: uint256
    part2: uint256
    part3: uint256
}
"#;

fn run_project_builder_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CV_OUT_ACTION_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("cv_out_action_parity", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn out_action_send_message_load_message_and_generic_are_parity_for_inline_body() {
    run_project_builder_case(
        "cv-stdlib-out-action-inline-parity",
        r#"
get fun `test-cv-out-action-inline-parity`() {
    val dest = net.randomAddress("cv_inline_parity_dest");
    createMessage({
        bounce: false,
        value: ton("1.5"),
        dest,
        body: InlineParityPayload {
            queryId: 17,
            amount: 23,
        },
    }).send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);

    val outActions = vm.outActions();
    expect(outActions.size()).toEqual(1);
    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();
    expect(action!.mode).toEqual(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);

    val typedMsg = action!.loadMessage<InlineParityPayload>();
    val genericMsg = action!.loadGenericMessage();
    val typedBody = typedMsg.loadBody();

    expect(genericMsg.loadOpcode()).toEqual(reflect.serializationPrefixOf<InlineParityPayload>());
    expect(typedMsg.info.dest).toEqual(dest);
    expect(typedMsg.info.value.grams).toEqual(ton("1.5"));
    expect(genericMsg.info.dest).toEqual(dest);
    expect(genericMsg.info.value.grams).toEqual(ton("1.5"));

    var genericBody = genericMsg.body;
    expect(genericBody.loadBool()).toBeFalse();
    val opcode = genericBody.loadUint(32);
    val queryId = genericBody.loadUint(64);
    val amount = genericBody.loadUint(32);

    expect(opcode).toEqual(reflect.serializationPrefixOf<InlineParityPayload>());
    expect(queryId).toEqual(typedBody.queryId);
    expect(amount).toEqual(typedBody.amount);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_out_action_send_message_load_message_and_generic_are_parity_for_inline_body_tests/out_action_send_message_load_message_and_generic_are_parity_for_inline_body.stdout.txt",
    );
}

#[test]
fn out_action_send_message_load_message_and_generic_are_parity_for_ref_body_in_fixture_project()
 {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cv_out_action_ref_parity.test.tolk";
    let source = format!(
        r#"{CV_OUT_ACTION_IMPORTS}
get fun `test-cv-out-action-ref-parity`() {{
    val dest = net.randomAddress("cv_ref_parity_dest");
    createMessage({{
        bounce: false,
        value: ton("2.25"),
        dest,
        body: RefParityPayload {{
            queryId: 99,
            part1: 0x11,
            part2: 0x22,
            part3: 0x33,
        }},
    }}).send(SEND_MODE_PAY_FEES_SEPARATELY);

    val outActions = vm.outActions();
    expect(outActions.size()).toEqual(1);
    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();
    expect(action!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);

    val typedMsg = action!.loadMessage<RefParityPayload>();
    val genericMsg = action!.loadGenericMessage();
    val typedBody = typedMsg.loadBody();

    expect(genericMsg.loadOpcode()).toEqual(reflect.serializationPrefixOf<RefParityPayload>());
    expect(typedMsg.info.dest).toEqual(dest);
    expect(typedMsg.info.value.grams).toEqual(ton("2.25"));
    expect(genericMsg.info.dest).toEqual(dest);
    expect(genericMsg.info.value.grams).toEqual(ton("2.25"));

    var genericBody = genericMsg.body;
    expect(genericBody.loadBool()).toBeTrue();
    var bodyRef = genericBody.loadRef().beginParse();
    val opcode = bodyRef.loadUint(32);
    val queryId = bodyRef.loadUint(64);
    val part1 = bodyRef.loadUint(256);
    val part2 = bodyRef.loadUint(256);
    val part3 = bodyRef.loadUint(256);

    expect(opcode).toEqual(reflect.serializationPrefixOf<RefParityPayload>());
    expect(queryId).toEqual(typedBody.queryId);
    expect(part1).toEqual(typedBody.part1);
    expect(part2).toEqual(typedBody.part2);
    expect(part3).toEqual(typedBody.part3);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source).expect("failed to write cv fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_out_action_send_message_load_message_and_generic_are_parity_for_inline_body_tests/out_action_send_message_load_message_and_generic_are_parity_for_ref_body_in_fixture_project.stdout.txt",
        );
}
