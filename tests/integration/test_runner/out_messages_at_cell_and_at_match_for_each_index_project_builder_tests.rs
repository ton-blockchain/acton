use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CU_NETWORK_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/message"
import "../contracts/cu_messages"
"#;

const CU_MESSAGES: &str = r"
struct (0xC011AA11) CuNotice {
    queryId: uint64
}
";

const CU_FANOUT_CONTRACT: &str = r#"
import "cu_messages"

fun onInternalMessage(in: InMessage) {
    createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: in.senderAddress,
        body: CuNotice { queryId: 901 },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);

    createMessage({
        bounce: false,
        value: ton("0.07"),
        dest: in.senderAddress,
        body: CuNotice { queryId: 902 },
    }).send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CU_FIXTURE_MESSAGES: &str = r"
struct (0xC011AA22) CuFixtureNotice {
    id: uint32
}
";

const CU_FIXTURE_CONTRACT: &str = r#"
import "cu_fixture_messages"

fun onInternalMessage(in: InMessage) {
    createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: in.senderAddress,
        body: CuFixtureNotice { id: 42 },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CU_FIXTURE_TEST: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/message"
import "../contracts/cu_fixture_messages"

get fun `test-cu-fixture-out-messages-atcell-at-consistency`() {
    val sender = net.treasury("cu_fixture_sender");

    val init = ContractState {
        code: build("cu_fixture_out_messages"),
        data: createEmptyCell(),
    };
    val contractAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deploy = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }));
    expect(deploy).toHaveSuccessfulDeploy({ to: contractAddress });

    val sendRes = net.sendSingle(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: contractAddress,
        }),
    );

    expect(sendRes.outMessages.size()).toEqual(1);

    val raw = sendRes.outMessages.atCell(0);
    val parsed = sendRes.outMessages.at<CuFixtureNotice>(0);
    val viaRaw = (raw as Cell<MessageRelaxed<CuFixtureNotice>>)
        .load({ assertEndAfterReading: false });

    expect(parsed.info.dest).toEqual(viaRaw.info.dest);
    expect(parsed.info.value.grams).toEqual(viaRaw.info.value.grams);

    val bodyViaAt = parsed.loadBody();
    val bodyViaRaw = viaRaw.loadBody();
    expect(bodyViaAt.id).toEqual(bodyViaRaw.id);
    expect(bodyViaAt.id).toEqual(42);
}
"#;

fn run_network_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CU_NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/cu_messages", CU_MESSAGES)
        .contract("cu_fanout", CU_FANOUT_CONTRACT)
        .test_file("cu_out_messages_consistency", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn out_messages_at_cell_and_at_match_for_each_index_project_builder() {
    run_network_success(
        "cu-stdlib-out-messages-atcell-at-project-builder",
        r#"
get fun `test-cu-out-messages-atcell-at-project-builder`() {
    val sender = net.treasury("cu_sender");

    val fanoutInit = ContractState {
        code: build("cu_fanout"),
        data: createEmptyCell(),
    };
    val fanoutAddress = AutoDeployAddress { stateInit: fanoutInit }.calculateAddress();

    val deploy = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: fanoutInit,
        },
    }));
    expect(deploy).toHaveSuccessfulDeploy({ to: fanoutAddress });

    val sendRes = net.sendSingle(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.3"),
            dest: fanoutAddress,
        }),
    );

    expect(sendRes.outMessages.size()).toEqual(2);

    val raw0 = sendRes.outMessages.atCell(0);
    val parsed0 = sendRes.outMessages.at<CuNotice>(0);
    val viaRaw0 = (raw0 as Cell<MessageRelaxed<CuNotice>>)
        .load({ assertEndAfterReading: false });

    val raw1 = sendRes.outMessages.atCell(1);
    val parsed1 = sendRes.outMessages.at<CuNotice>(1);
    val viaRaw1 = (raw1 as Cell<MessageRelaxed<CuNotice>>)
        .load({ assertEndAfterReading: false });

    expect(parsed0.info.dest).toEqual(viaRaw0.info.dest);
    expect(parsed0.info.value.grams).toEqual(viaRaw0.info.value.grams);

    expect(parsed1.info.dest).toEqual(viaRaw1.info.dest);
    expect(parsed1.info.value.grams).toEqual(viaRaw1.info.value.grams);

    val bodyViaAt0 = parsed0.loadBody();
    val bodyViaAt1 = parsed1.loadBody();
    val bodyViaRaw0 = viaRaw0.loadBody();
    val bodyViaRaw1 = viaRaw1.loadBody();

    expect(bodyViaAt0.queryId).toEqual(bodyViaRaw0.queryId);
    expect(bodyViaAt1.queryId).toEqual(bodyViaRaw1.queryId);
    expect(bodyViaAt0.queryId == bodyViaAt1.queryId).toEqual(false);
}
"#,
        "integration/snapshots/test-runner/out_messages_at_cell_and_at_match_for_each_index_project_builder/out_messages_at_cell_and_at_match_for_each_index_project_builder.stdout.txt",
    );
}

#[test]
fn out_messages_at_cell_and_at_match_for_same_index_fixture_project() {
    let fixture = FixtureProject::load("basic");

    fs::write(
        fixture.path().join("contracts/cu_fixture_messages.tolk"),
        CU_FIXTURE_MESSAGES,
    )
    .expect("failed to write fixture messages for cu out-messages test");
    fs::write(
        fixture
            .path()
            .join("contracts/cu_fixture_out_messages.tolk"),
        CU_FIXTURE_CONTRACT,
    )
    .expect("failed to write fixture contract for cu out-messages test");

    let acton_path = fixture.path().join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_path).expect("failed to read fixture Acton.toml for cu test");
    acton_toml.push_str(
        r#"

[contracts.cu_fixture_out_messages]
name = "CuFixtureOutMessages"
src = "contracts/cu_fixture_out_messages.tolk"
depends = []
"#,
    );
    fs::write(&acton_path, acton_toml).expect("failed to update fixture Acton.toml for cu test");

    let test_path = "tests/cu_out_messages_consistency.test.tolk";
    fs::write(fixture.path().join(test_path), CU_FIXTURE_TEST)
        .expect("failed to write fixture test file for cu out-messages test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/out_messages_at_cell_and_at_match_for_each_index_project_builder/out_messages_at_cell_and_at_match_for_same_index_fixture_project.stdout.txt",
        );
}
