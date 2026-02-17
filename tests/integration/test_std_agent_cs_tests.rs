//! Reserved integration test module for subagent CS.
//!
//! Ownership boundary for agent CS:
//! - tests/integration/test_std_agent_cs_tests.rs
//! - tests/integration/snapshots/test_std_agent_cs/**
//! - tests/integration/testdata/test_std_agent_cs/**
//! - tests/support/test_std_agent_cs/** (optional)
//!
//! Required test name prefix:
//! - cs_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const CS_MESSAGES: &str = r#"
struct (0xCC510001) CsRoute {
    queryId: uint64
    mid: address
    finalDest: address
}

struct (0xCC510002) CsRelay {
    queryId: uint64
    finalDest: address
}

struct (0xCC510003) CsDelivered {
    queryId: uint64
    hop: uint8
}
"#;

const CS_ROOT_CONTRACT: &str = r#"
import "cs_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy CsRoute.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.mid,
        body: CsRelay {
            queryId: msg.queryId,
            finalDest: msg.finalDest,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CS_MID_CONTRACT: &str = r#"
import "cs_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy CsRelay.fromSlice(in.body);
    createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: msg.finalDest,
        body: CsDelivered {
            queryId: msg.queryId,
            hop: 2,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CS_SINK_CONTRACT: &str = r#"
import "cs_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy CsDelivered.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CS_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/out_actions"
import "../contracts/cs_messages"

fun deployCsHarness() {
    val sender = net.treasury("sender");

    val rootInit = ContractState {
        code: build("cs_root"),
        data: createEmptyCell(),
    };
    val rootAddress = AutoDeployAddress { stateInit: rootInit }.calculateAddress();

    val midInit = ContractState {
        code: build("cs_mid"),
        data: createEmptyCell(),
    };
    val midAddress = AutoDeployAddress { stateInit: midInit }.calculateAddress();

    val sinkInit = ContractState {
        code: build("cs_sink"),
        data: createEmptyCell(),
    };
    val sinkAddress = AutoDeployAddress { stateInit: sinkInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: rootInit,
        },
    }))).toHaveSuccessfulDeploy({ to: rootAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: midInit,
        },
    }))).toHaveSuccessfulDeploy({ to: midAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: sinkInit,
        },
    }))).toHaveSuccessfulDeploy({ to: sinkAddress });

    return (sender, rootAddress, midAddress, sinkAddress);
}

fun sendCsRoute(sender: Treasury, rootAddress: address, midAddress: address, sinkAddress: address, queryId: uint64): SendResultList {
    return net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.6"),
            dest: rootAddress,
            body: CsRoute {
                queryId,
                mid: midAddress,
                finalDest: sinkAddress,
            },
        }),
    );
}
"#;

fn run_cs_project_builder_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/cs_messages", CS_MESSAGES)
        .contract("cs_root", CS_ROOT_CONTRACT)
        .contract("cs_mid", CS_MID_CONTRACT)
        .contract("cs_sink", CS_SINK_CONTRACT)
        .test_file("cs_send_result_all_out_actions", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn cs_stdlib_send_result_all_out_actions_flattens_root_and_child_transactions_in_project_builder() {
    run_cs_project_builder_success(
        "cs-stdlib-send-result-all-out-actions-project-builder",
        r#"
get fun `test-cs-send-result-all-out-actions-project-builder`() {
    val (sender, rootAddress, midAddress, sinkAddress) = deployCsHarness();
    val txs = sendCsRoute(sender, rootAddress, midAddress, sinkAddress, 901);

    expect(txs).toHaveLength(3);
    expect(txs).toHaveSuccessfulTx<CsRoute>({ from: sender.address, to: rootAddress });
    expect(txs).toHaveSuccessfulTx<CsRelay>({ from: rootAddress, to: midAddress });
    expect(txs).toHaveSuccessfulTx<CsDelivered>({ from: midAddress, to: sinkAddress });

    val rootActions = txs.at(0).allOutActions();
    val childActions = txs.at(1).allOutActions();

    expect(rootActions.size()).toEqual(1);
    expect(childActions.size()).toEqual(1);
    expect(rootActions.at(0).kind()).toEqual("send-message");
    expect(childActions.at(0).kind()).toEqual("send-message");

    val rootSend = rootActions.getSendMessageAt(0);
    val childSend = childActions.getSendMessageAt(0);
    expect(rootSend).toBeNotNull();
    expect(childSend).toBeNotNull();
    expect(rootSend!.mode).toEqual(SEND_MODE_REGULAR);
    expect(childSend!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);

    val firstHop = rootActions.getSendMessageBodyAt<CsRelay>(0);
    val secondHop = childActions.getSendMessageBodyAt<CsDelivered>(0);
    expect(firstHop).toBeNotNull();
    expect(secondHop).toBeNotNull();
    expect(firstHop!.queryId).toEqual(901);
    expect(firstHop!.finalDest).toEqual(sinkAddress);
    expect(secondHop!.queryId).toEqual(901);
    expect(secondHop!.hop).toEqual(2);

    val flattenedCount = rootActions.size() + childActions.size();
    expect(flattenedCount).toEqual(2);
}
"#,
        "integration/snapshots/test_std_agent_cs/cs_stdlib_send_result_all_out_actions_flattens_root_and_child_transactions_in_project_builder.stdout.txt",
    );
}

#[test]
fn cs_stdlib_send_result_all_out_actions_terminal_transaction_without_actions_triggers_exit63_bug_in_fixture_project(
) {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cs_send_result_all_out_actions_terminal.test.tolk";

    let source = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/out_actions"
import "../contracts/counter_messages"

get fun `test-cs-send-result-all-out-actions-terminal`() {
    val deployer = net.treasury("deployer");
    val init = ContractState {
        code: build("counter"),
        data: Storage { id: 902, counter: 0 }.toCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    });
    val deployRes = net.send(deployer.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({ to: counterAddress });

    val txs = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: counterAddress,
            body: IncreaseCounter {
                queryId: 9,
                increaseBy: 3,
            },
        }),
    );

    expect(txs).toHaveLength(1);
    expect(txs).toHaveSuccessfulTx<IncreaseCounter>({
        from: deployer.address,
        to: counterAddress,
    });

    // BUG: SendResult.allOutActions should return an empty list for transactions without out actions, expected size=0, got exit_code=63.
    val terminalActions = txs.at(0).allOutActions();
    expect(terminalActions.size()).toEqual(0);
}
"#;
    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write cs fixture allOutActions test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_cs/cs_stdlib_send_result_all_out_actions_handles_terminal_transaction_without_actions_in_fixture_project.stdout.txt",
        );
}
