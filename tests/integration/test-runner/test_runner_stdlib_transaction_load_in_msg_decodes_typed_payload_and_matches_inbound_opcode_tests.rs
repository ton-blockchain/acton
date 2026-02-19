use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DA_TRANSACTION_IMPORTS: &str = r#"
import "@stdlib/reflection"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/types/message"
import "../../lib/types/transaction"

struct (0xDA700001) DaInlinePayload {
    queryId: uint64
    amount: uint32
}
"#;

fn run_transaction_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DA_TRANSACTION_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("da_transaction_helpers", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn transaction_load_in_msg_decodes_typed_payload_and_matches_inbound_opcode() {
    run_transaction_success(
        "da-stdlib-transaction-load-in-msg-inline-opcode",
        r#"
get fun `test-da-stdlib-transaction-load-in-msg-inline-opcode`() {
    val sender = net.treasury("da_sender_inline");
    val destination = net.randomAddress("da_destination_inline");

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: destination,
            body: DaInlinePayload {
                queryId: 11,
                amount: 22,
            },
        }),
    );

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();

    val inMsg = tx.loadInMsg<DaInlinePayload>();
    val inBody = inMsg.loadBody();

    expect(inBody).toEqual(DaInlinePayload { queryId: 11, amount: 22 });
    expect(inMsg.info.src).toEqual(sender.address as any_address);
    expect(inMsg.info.dest).toEqual(destination);

    val genericInMsg = tx.messages.load().inMsg.unwrap().load();
    expect(genericInMsg.loadOpcode()).toEqual(reflect.serializationPrefixOf<DaInlinePayload>());
    expect(genericInMsg.info.src).toEqual(sender.address as any_address);
    expect(genericInMsg.info.dest).toEqual(destination);

    var rawBody = genericInMsg.body;
    expect(rawBody.loadBool()).toBeFalse();
    expect(rawBody.loadUint(32)).toEqual(reflect.serializationPrefixOf<DaInlinePayload>());
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_transaction_load_in_msg_decodes_typed_payload_and_matches_inbound_opcode_tests/transaction_load_in_msg_decodes_typed_payload_and_matches_inbound_opcode.stdout.txt",
    );
}

#[test]
fn transaction_load_in_msg_in_fixture_project_matches_inbound_opcode() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/da_transaction_load_in_msg_fixture_opcode.test.tolk";
    let source = format!(
        "{imports}\n{body}\n",
        imports = DA_TRANSACTION_IMPORTS,
        body = r#"
get fun `test-da-stdlib-transaction-load-in-msg-fixture-opcode`() {
    val sender = net.treasury("da_sender_fixture");
    val destination = net.randomAddress("da_destination_fixture");
    val payload = DaInlinePayload { queryId: 77, amount: 99 };

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: destination,
            body: payload,
        }),
    );

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();

    val inMsg = tx.loadInMsg<DaInlinePayload>();
    expect(inMsg.loadBody()).toEqual(payload);
    expect(inMsg.info.src).toEqual(sender.address as any_address);
    expect(inMsg.info.dest).toEqual(destination);

    val genericInMsg = tx.messages.load().inMsg.unwrap().load();
    expect(genericInMsg.loadOpcode()).toEqual(reflect.serializationPrefixOf<DaInlinePayload>());
    expect(genericInMsg.info.src).toEqual(sender.address as any_address);
    expect(genericInMsg.info.dest).toEqual(destination);

    var rawBody = genericInMsg.body;
    expect(rawBody.loadBool()).toBeFalse();
    expect(rawBody.loadUint(32)).toEqual(reflect.serializationPrefixOf<DaInlinePayload>());
}
"#
    );

    fs::write(fixture.path().join(test_path), source).expect("failed to write da fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_transaction_load_in_msg_decodes_typed_payload_and_matches_inbound_opcode_tests/transaction_load_in_msg_in_fixture_project_matches_inbound_opcode.stdout.txt",
        );
}
