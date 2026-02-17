//! Reserved integration test module for subagent DD.
//!
//! Ownership boundary for agent DD:
//! - tests/integration/test_std_agent_dd_tests.rs
//! - tests/integration/snapshots/test_std_agent_dd/**
//! - tests/integration/testdata/test_std_agent_dd/**
//! - tests/support/test_std_agent_dd/** (optional)
//!
//! Required test name prefix:
//! - dd_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DD_MESSAGES: &str = r#"
struct (0xDD180001) DdTrigger {
    queryId: uint64
    relay: address
}

struct (0xDD180002) DdForward {
    queryId: uint64
}

struct (0xDD180003) DdAlphaNotice {
    queryId: uint64
    origin: uint8
}

struct (0xDD180004) DdBetaNotice {
    queryId: uint64
    origin: uint8
}
"#;

const DD_ROOT_CONTRACT: &str = r#"
import "dd_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy DdTrigger.fromSlice(in.body);

    createExternalLogMessage({
        dest: createAddressNone(),
        body: DdAlphaNotice {
            queryId: msg.queryId,
            origin: 1,
        },
    }).send(SEND_MODE_REGULAR);

    createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.relay,
        body: DdForward {
            queryId: msg.queryId,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DD_RELAY_CONTRACT: &str = r#"
import "dd_messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy DdForward.fromSlice(in.body);

    createExternalLogMessage({
        dest: createAddressNone(),
        body: DdBetaNotice {
            queryId: msg.queryId,
            origin: 2,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DD_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/dd_messages"

fun deployDdHarness() {
    val sender = net.treasury("sender");

    val relayInit = ContractState {
        code: build("dd_relay"),
        data: createEmptyCell(),
    };
    val relayAddress = AutoDeployAddress { stateInit: relayInit }.calculateAddress();

    val rootInit = ContractState {
        code: build("dd_root"),
        data: createEmptyCell(),
    };
    val rootAddress = AutoDeployAddress { stateInit: rootInit }.calculateAddress();

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: relayInit,
        },
    }))).toHaveSuccessfulDeploy({ to: relayAddress });

    expect(net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: rootInit,
        },
    }))).toHaveSuccessfulDeploy({ to: rootAddress });

    return (sender, rootAddress, relayAddress);
}

fun sendDdTrigger(sender: Treasury, rootAddress: address, relayAddress: address, queryId: uint64): SendResultList {
    return net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.4"),
            dest: rootAddress,
            body: DdTrigger {
                queryId,
                relay: relayAddress,
            },
        }),
    );
}
"#;

fn run_dd_project_builder_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DD_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/dd_messages", DD_MESSAGES)
        .contract("dd_root", DD_ROOT_CONTRACT)
        .contract("dd_relay", DD_RELAY_CONTRACT)
        .test_file("dd_find_external_out_message", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn dd_stdlib_find_external_out_message_filters_by_type_and_source_across_transactions() {
    run_dd_project_builder_success(
        "dd-stdlib-find-external-out-message-type-and-source",
        r#"
get fun `test-dd-find-external-out-message-type-and-source`() {
    val (sender, rootAddress, relayAddress) = deployDdHarness();
    val txs = sendDdTrigger(sender, rootAddress, relayAddress, 451);

    expect(txs).toHaveLength(2);
    expect(txs).toHaveSuccessfulTx<DdTrigger>({
        from: sender.address,
        to: rootAddress,
    });
    expect(txs).toHaveSuccessfulTx<DdForward>({
        from: rootAddress,
        to: relayAddress,
    });

    // BUG: SendResultList.findExternalOutMessage fails to compile because network.tolk uses tuple.get with type arguments.
    val alpha = txs.findExternalOutMessage<DdAlphaNotice>({
        from: rootAddress,
        to: createAddressNone(),
    });
    val beta = txs.findExternalOutMessage<DdBetaNotice>({
        from: relayAddress,
        to: createAddressNone(),
    });

    expect(alpha).toBeDefined();
    expect(beta).toBeDefined();
    expect(alpha.unwrap().loadBody()).toEqual(DdAlphaNotice {
        queryId: 451,
        origin: 1,
    });
    expect(beta.unwrap().loadBody()).toEqual(DdBetaNotice {
        queryId: 451,
        origin: 2,
    });

    val wrongType = txs.findExternalOutMessage<DdBetaNotice>({
        from: rootAddress,
        to: createAddressNone(),
    });
    expect(wrongType).toBeNone();
}
"#,
        "integration/snapshots/test_std_agent_dd/dd_stdlib_find_external_out_message_filters_by_type_and_source_across_transactions.stdout.txt",
    );
}

#[test]
fn dd_stdlib_find_external_out_message_uses_body_type_per_send_result_list() {
    run_dd_project_builder_success(
        "dd-stdlib-find-external-out-message-per-send",
        r#"
get fun `test-dd-find-external-out-message-per-send`() {
    val (sender, rootAddress, relayAddress) = deployDdHarness();
    val first = sendDdTrigger(sender, rootAddress, relayAddress, 700);
    val second = sendDdTrigger(sender, rootAddress, relayAddress, 701);

    expect(first).toHaveLength(2);
    expect(second).toHaveLength(2);

    // BUG: SendResultList.findExternalOutMessage fails to compile because network.tolk uses tuple.get with type arguments.
    val firstBeta = first.findExternalOutMessage<DdBetaNotice>({
        from: relayAddress,
        to: createAddressNone(),
    });
    val secondBeta = second.findExternalOutMessage<DdBetaNotice>({
        from: relayAddress,
        to: createAddressNone(),
    });
    expect(firstBeta).toBeDefined();
    expect(secondBeta).toBeDefined();
    expect(firstBeta.unwrap().loadBody()).toEqual(DdBetaNotice {
        queryId: 700,
        origin: 2,
    });
    expect(secondBeta.unwrap().loadBody()).toEqual(DdBetaNotice {
        queryId: 701,
        origin: 2,
    });

    val secondWrongOpcode = second.findExternalOutMessage<DdAlphaNotice>({
        from: relayAddress,
        to: createAddressNone(),
    });
    expect(secondWrongOpcode).toBeNone();
}
"#,
        "integration/snapshots/test_std_agent_dd/dd_stdlib_find_external_out_message_uses_body_type_per_send_result_list.stdout.txt",
    );
}
