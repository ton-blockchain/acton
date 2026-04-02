use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DF_MESSAGES: &str = r"
struct (0xDF000001) DfPing {
    queryId: uint64
}

struct (0xDF000002) DfNotice {
    queryId: uint64
}
";

const DF_ECHO_CONTRACT: &str = r#"
import "df_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val ping = lazy DfPing.fromSlice(in.body);

    createMessage({
        bounce: false,
        value: ton("0.05"),
        dest: in.senderAddress,
        body: DfNotice {
            queryId: ping.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DF_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/big_array"
import "../../lib/types/message"
import "../../lib/types/out_actions"
import "../../lib/types/transaction"
import "../contracts/df_messages"
"#;

fn run_project_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DF_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .file("contracts/df_messages", DF_MESSAGES)
        .contract("df_echo", DF_ECHO_CONTRACT)
        .test_file("df_send_single_vs_send", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn net_send_single_matches_net_send_first_result_transaction_and_action_in_project_builder() {
    run_project_success(
        "df-stdlib-send-single-vs-send-first-result-project-builder",
        r#"
get fun `test-df-send-single-vs-send-first-result-project-builder`() {
    val sender = net.treasury("df_sender_project");

    val init = ContractState {
        code: build("df_echo"),
        data: createEmptyCell(),
    };
    val echoAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deploy = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    });
    expect(net.send(sender.address, deploy)).toHaveSuccessfulDeploy({ to: echoAddress });

    val sendSingleResult = net.sendSingle(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.4"),
            dest: echoAddress,
            body: DfPing { queryId: 41 },
        }),
    );

    val sendResultList = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.4"),
            dest: echoAddress,
            body: DfPing { queryId: 41 },
        }),
    );
    expect(sendResultList.size() > 0).toBeTrue();

    val sendFirstResult = sendResultList.at(0);
    val sendSingleTx = sendSingleResult.tx.load();
    val sendFirstTx = sendFirstResult.tx.load();

    expect(sendSingleResult.childTxs.size()).toEqual(0);
    expect(sendSingleTx.getAccountAddress()).toEqual(sendFirstTx.getAccountAddress());
    expect(sendSingleTx.outmsgCnt).toEqual(sendFirstTx.outmsgCnt);

    val sendSingleIn = sendSingleTx.loadBody<DfPing>();
    val sendFirstIn = sendFirstTx.loadBody<DfPing>();
    expect(sendSingleIn.queryId).toEqual(sendFirstIn.queryId);
    expect(sendSingleIn.queryId).toEqual(41);

    val sendSingleActions = sendSingleResult.allOutActions();
    val sendFirstActions = sendFirstResult.allOutActions();
    expect(sendSingleActions.size()).toEqual(sendFirstActions.size());

    val sendSingleAction = sendSingleActions.getSendMessageAt(0);
    val sendFirstAction = sendFirstActions.getSendMessageAt(0);
    expect(sendSingleAction).toBeNotNull();
    expect(sendFirstAction).toBeNotNull();

    expect(sendSingleAction!.mode).toEqual(sendFirstAction!.mode);
    expect(sendSingleAction!.loadBody<DfNotice>()).toEqual(sendFirstAction!.loadBody<DfNotice>());

    expect(sendSingleResult.outMessages.size()).toEqual(sendFirstResult.outMessages.size());
    val sendSingleOut = sendSingleResult.outMessages.at<DfNotice>(0);
    val sendFirstOut = sendFirstResult.outMessages.at<DfNotice>(0);

    expect(sendSingleOut.info.dest).toEqual(sendFirstOut.info.dest);
    expect(sendSingleOut.info.value.grams).toEqual(sendFirstOut.info.value.grams);

    val sendSingleOutBody = sendSingleOut.loadBody();
    val sendFirstOutBody = sendFirstOut.loadBody();
    expect(sendSingleOutBody).toEqual(sendFirstOutBody);
}
"#,
        "integration/snapshots/test-runner/net_send_single_matches_net_send_first_result_transaction_and_action_in_project_builder/net_send_single_matches_net_send_first_result_transaction_and_action_in_project_builder.stdout.txt",
    );
}

#[test]
fn net_send_single_matches_net_send_first_result_transaction_and_action_in_fixture_project() {
    let fixture = FixtureProject::load("basic");

    fs::write(
        fixture.path().join("contracts/df_messages.tolk"),
        DF_MESSAGES,
    )
    .expect("failed to write fixture messages for df sendSingle/send equivalence test");
    fs::write(
        fixture.path().join("contracts/df_echo.tolk"),
        DF_ECHO_CONTRACT,
    )
    .expect("failed to write fixture contract for df sendSingle/send equivalence test");

    let acton_path = fixture.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_path)
        .expect("failed to read fixture Acton.toml for df sendSingle/send equivalence test");
    acton_toml.push_str(
        r#"

[contracts.df_echo]
name = "DfEcho"
src = "contracts/df_echo.tolk"
depends = []
"#,
    );
    fs::write(&acton_path, acton_toml)
        .expect("failed to update fixture Acton.toml for df sendSingle/send equivalence test");

    let test_path = "tests/df_send_single_vs_send_first_result_equivalence.test.tolk";
    let source = format!(
        r#"{DF_IMPORTS}
get fun `test-df-send-single-vs-send-first-result-fixture-project`() {{
    val sender = net.treasury("df_sender_fixture");

    val init = ContractState {{
        code: build("df_echo"),
        data: createEmptyCell(),
    }};
    val echoAddress = AutoDeployAddress {{ stateInit: init }}.calculateAddress();

    val deploy = createMessage({{
        bounce: false,
        value: ton("1"),
        dest: {{
            stateInit: init,
        }},
    }});
    expect(net.send(sender.address, deploy)).toHaveSuccessfulDeploy({{ to: echoAddress }});

    val sendSingleResult = net.sendSingle(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.35"),
            dest: echoAddress,
            body: DfPing {{ queryId: 77 }},
        }}),
    );

    val sendResultList = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.35"),
            dest: echoAddress,
            body: DfPing {{ queryId: 77 }},
        }}),
    );
    expect(sendResultList.size() > 0).toBeTrue();

    val sendFirstResult = sendResultList.at(0);
    val sendSingleTx = sendSingleResult.tx.load();
    val sendFirstTx = sendFirstResult.tx.load();

    expect(sendSingleResult.childTxs.size()).toEqual(0);
    expect(sendSingleTx.getAccountAddress()).toEqual(sendFirstTx.getAccountAddress());
    expect(sendSingleTx.outmsgCnt).toEqual(sendFirstTx.outmsgCnt);

    val sendSingleIn = sendSingleTx.loadBody<DfPing>();
    val sendFirstIn = sendFirstTx.loadBody<DfPing>();
    expect(sendSingleIn.queryId).toEqual(sendFirstIn.queryId);
    expect(sendSingleIn.queryId).toEqual(77);

    val sendSingleActions = sendSingleResult.allOutActions();
    val sendFirstActions = sendFirstResult.allOutActions();
    expect(sendSingleActions.size()).toEqual(sendFirstActions.size());

    val sendSingleAction = sendSingleActions.getSendMessageAt(0);
    val sendFirstAction = sendFirstActions.getSendMessageAt(0);
    expect(sendSingleAction).toBeNotNull();
    expect(sendFirstAction).toBeNotNull();

    expect(sendSingleAction!.mode).toEqual(sendFirstAction!.mode);
    expect(sendSingleAction!.loadBody<DfNotice>()).toEqual(sendFirstAction!.loadBody<DfNotice>());

    expect(sendSingleResult.outMessages.size()).toEqual(sendFirstResult.outMessages.size());
    val sendSingleOut = sendSingleResult.outMessages.at<DfNotice>(0);
    val sendFirstOut = sendFirstResult.outMessages.at<DfNotice>(0);

    expect(sendSingleOut.info.dest).toEqual(sendFirstOut.info.dest);
    expect(sendSingleOut.info.value.grams).toEqual(sendFirstOut.info.value.grams);

    val sendSingleOutBody = sendSingleOut.loadBody();
    val sendFirstOutBody = sendFirstOut.loadBody();
    expect(sendSingleOutBody).toEqual(sendFirstOutBody);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write fixture test for df sendSingle/send equivalence");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/net_send_single_matches_net_send_first_result_transaction_and_action_in_project_builder/net_send_single_matches_net_send_first_result_transaction_and_action_in_fixture_project.stdout.txt",
        );
}
