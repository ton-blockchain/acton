//! Reserved for agent-db.
//! Prefix: db_stdlib_
//! Ownership: this file and tests/integration/snapshots/test-runner/test_runner_stdlib_db_transaction_tests/**
//! Agent will add targeted stdlib integration tests here.

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DB_TRANSACTION_IMPORTS: &str = r#"
import "@stdlib/reflection"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/types/message"
import "../../lib/types/transaction"

struct (0xDB700001) DbNestedMeta {
    nonce: uint32
    acknowledged: bool
}

struct (0xDB700002) DbNestedPayload {
    queryId: uint64
    amount: uint32
    meta: DbNestedMeta
}
"#;

fn run_db_transaction_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DB_TRANSACTION_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("db_transaction_load_body_nested", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn transaction_load_body_decodes_nested_payload_from_inbound_message() {
    run_db_transaction_success(
        "db-stdlib-transaction-load-body-nested-inline",
        r#"
get fun `test-db-stdlib-transaction-load-body-nested-inline`() {
    val sender = net.treasury("db_sender_inline");
    val destination = net.randomAddress("db_destination_inline");
    val payload = DbNestedPayload {
        queryId: 901,
        amount: 77,
        meta: DbNestedMeta {
            nonce: 5,
            acknowledged: true,
        },
    };

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

    val typedBody = tx.loadBody<DbNestedPayload>();
    expect(typedBody).toEqual(payload);
    expect(typedBody.meta).toEqual(DbNestedMeta { nonce: 5, acknowledged: true });

    val inMsg = tx.loadInMsg<DbNestedPayload>();
    expect(inMsg.loadBody()).toEqual(payload);
    expect(inMsg.info.src).toEqual(sender.address as any_address);
    expect(inMsg.info.dest).toEqual(destination);

    val genericInMsg = tx.messages.load().inMsg.unwrap().load();
    expect(genericInMsg.loadOpcode()).toEqual(reflect.serializationPrefixOf<DbNestedPayload>());

    var rawBody = genericInMsg.body;
    expect(rawBody.loadBool()).toBeFalse();
    expect(rawBody.loadUint(32)).toEqual(reflect.serializationPrefixOf<DbNestedPayload>());
    expect(rawBody.loadUint(64)).toEqual(payload.queryId);
    expect(rawBody.loadUint(32)).toEqual(payload.amount);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_db_transaction_tests/db_stdlib_transaction_load_body_decodes_nested_payload_from_inbound_message.stdout.txt",
    );
}

#[test]
fn transaction_load_body_decodes_nested_payload_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/db_transaction_load_body_nested_fixture.test.tolk";
    let source = format!(
        "{imports}\n{body}\n",
        imports = DB_TRANSACTION_IMPORTS,
        body = r#"
get fun `test-db-stdlib-transaction-load-body-nested-fixture`() {
    val sender = net.treasury("db_sender_fixture");
    val destination = net.randomAddress("db_destination_fixture");
    val payload = DbNestedPayload {
        queryId: 902,
        amount: 88,
        meta: DbNestedMeta {
            nonce: 6,
            acknowledged: false,
        },
    };

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.3"),
            dest: destination,
            body: payload,
        }),
    );

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();

    val typedBody = tx.loadBody<DbNestedPayload>();
    expect(typedBody).toEqual(payload);
    expect(typedBody.meta).toEqual(DbNestedMeta { nonce: 6, acknowledged: false });

    val inMsg = tx.loadInMsg<DbNestedPayload>();
    expect(inMsg.loadBody()).toEqual(payload);
    expect(inMsg.info.src).toEqual(sender.address as any_address);
    expect(inMsg.info.dest).toEqual(destination);

    val genericInMsg = tx.messages.load().inMsg.unwrap().load();
    expect(genericInMsg.loadOpcode()).toEqual(reflect.serializationPrefixOf<DbNestedPayload>());
}
"#
    );

    fs::write(fixture.path().join(test_path), source).expect("failed to write db fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_db_transaction_tests/db_stdlib_transaction_load_body_decodes_nested_payload_in_fixture_project.stdout.txt",
        );
}
