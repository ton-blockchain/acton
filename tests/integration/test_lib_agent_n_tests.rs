use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FORWARD_MESSAGES: &str = r#"
struct (0x1000f001) TriggerForward {
    queryId: uint64
    target: address
}

struct (0x1000f002) Notify {
    queryId: uint64
}
"#;

const FORWARDER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerForward.fromSlice(in.body);

    val out = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: msg.target,
        body: Notify {
            queryId: msg.queryId,
        },
    });
    out.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const RECEIVER_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }
    val _msg = lazy Notify.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ECHO_MESSAGES: &str = r#"
struct (0x2000f001) TriggerEcho {
    queryId: uint64
}

struct (0x2000f002) EchoNotice {
    queryId: uint64
}
"#;

const ECHO_CONTRACT: &str = r#"
import "messages"

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerEcho.fromSlice(in.body);

    val notice = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: in.senderAddress,
        body: EchoNotice {
            queryId: msg.queryId,
        },
    });
    notice.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ECHO_FAIL_BOUNCE_CONTRACT: &str = r#"
import "messages"

const ERR_REJECT_BOUNCE = 777;

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy TriggerEcho.fromSlice(in.body);

    val notice = createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: in.senderAddress,
        body: EchoNotice {
            queryId: msg.queryId,
        },
    });
    notice.send(SEND_MODE_PAY_FEES_SEPARATELY);
}

fun onBouncedMessage(_: InMessageBounced) {
    throw ERR_REJECT_BOUNCE;
}
"#;

#[test]
fn n_lib_api_send_single_keeps_out_messages_without_executing_children() {
    let project = ProjectBuilder::new("n-lib-api-send-single-unprocessed-child")
        .file("contracts/messages", FORWARD_MESSAGES)
        .contract("forwarder", FORWARDER_CONTRACT)
        .contract("receiver", RECEIVER_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"
            import "../contracts/messages"

            get fun `test-send-single-keeps-out-messages`() {
                val sender = net.treasury("sender");

                val forwarderInit = ContractState {
                    code: build("forwarder"),
                    data: createEmptyCell(),
                };
                val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

                val receiverInit = ContractState {
                    code: build("receiver"),
                    data: createEmptyCell(),
                };
                val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

                val deployForwarder = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: forwarderInit,
                    },
                });
                val deployForwarderRes = net.send(sender.address, deployForwarder);
                expect(deployForwarderRes).toHaveSuccessfulDeploy({ to: forwarderAddress });

                val deployReceiver = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: receiverInit,
                    },
                });
                val deployReceiverRes = net.send(sender.address, deployReceiver);
                expect(deployReceiverRes).toHaveSuccessfulDeploy({ to: receiverAddress });

                val trigger = createMessage({
                    bounce: false,
                    value: ton("0.5"),
                    dest: forwarderAddress,
                    body: TriggerForward {
                        queryId: 7,
                        target: receiverAddress,
                    },
                });

                val sendSingleRes = net.sendSingle(sender.address, trigger);
                expect(sendSingleRes.outMessages.size()).toEqual(1);
                expect(sendSingleRes.childTxs.size()).toEqual(0);

                val notice = sendSingleRes.outMessages.at<Notify>(0).loadBody();
                expect(notice.queryId).toEqual(7);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_n/n_lib_api_send_single_keeps_out_messages_without_executing_children.stdout.txt",
        );
}

#[test]
fn n_lib_api_send_executes_child_transactions_and_matches_notify_expectation() {
    let project = ProjectBuilder::new("n-lib-api-send-processes-children")
        .file("contracts/messages", FORWARD_MESSAGES)
        .contract("forwarder", FORWARDER_CONTRACT)
        .contract("receiver", RECEIVER_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"
            import "../contracts/messages"

            get fun `test-send-processes-child-transactions`() {
                val sender = net.treasury("sender");

                val forwarderInit = ContractState {
                    code: build("forwarder"),
                    data: createEmptyCell(),
                };
                val forwarderAddress = AutoDeployAddress { stateInit: forwarderInit }.calculateAddress();

                val receiverInit = ContractState {
                    code: build("receiver"),
                    data: createEmptyCell(),
                };
                val receiverAddress = AutoDeployAddress { stateInit: receiverInit }.calculateAddress();

                val deployForwarder = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: forwarderInit,
                    },
                });
                val deployForwarderRes = net.send(sender.address, deployForwarder);
                expect(deployForwarderRes).toHaveSuccessfulDeploy({ to: forwarderAddress });

                val deployReceiver = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: receiverInit,
                    },
                });
                val deployReceiverRes = net.send(sender.address, deployReceiver);
                expect(deployReceiverRes).toHaveSuccessfulDeploy({ to: receiverAddress });

                val trigger = createMessage({
                    bounce: false,
                    value: ton("0.5"),
                    dest: forwarderAddress,
                    body: TriggerForward {
                        queryId: 11,
                        target: receiverAddress,
                    },
                });

                val sendRes = net.send(sender.address, trigger);
                expect(sendRes).toHaveSuccessfulTx<TriggerForward>({
                    from: sender.address,
                    to: forwarderAddress,
                });
                expect(sendRes).toHaveSuccessfulTx<Notify>({
                    from: forwarderAddress,
                    to: receiverAddress,
                });
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_n/n_lib_api_send_executes_child_transactions_and_matches_notify_expectation.stdout.txt",
        );
}

#[test]
fn n_lib_api_send_single_bounce_roundtrip_matches_bounced_notify_opcode() {
    let project = ProjectBuilder::new("n-lib-api-send-single-bounce-roundtrip")
        .file("contracts/messages", ECHO_MESSAGES)
        .contract("echo", ECHO_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"
            import "../contracts/messages"

            get fun `test-send-single-bounce-roundtrip-bounced-opcode`() {
                val sender = net.treasury("sender");

                val echoInit = ContractState {
                    code: build("echo"),
                    data: createEmptyCell(),
                };
                val echoAddress = AutoDeployAddress { stateInit: echoInit }.calculateAddress();

                val deployEcho = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: echoInit,
                    },
                });
                val deployEchoRes = net.send(sender.address, deployEcho);
                expect(deployEchoRes).toHaveSuccessfulDeploy({ to: echoAddress });

                val trigger = createMessage({
                    bounce: false,
                    value: ton("0.5"),
                    dest: echoAddress,
                    body: TriggerEcho {
                        queryId: 13,
                    },
                });

                val sendSingleRes = net.sendSingle(sender.address, trigger);
                expect(sendSingleRes.outMessages.size()).toEqual(1);
                expect(sendSingleRes.childTxs.size()).toEqual(0);

                val noticeBody = sendSingleRes.outMessages.at<EchoNotice>(0).loadBody().toCell();
                val bouncedBody = beginCell()
                    .storeUint(0xFFFFFFFF, 32)
                    .storeSlice(noticeBody.beginParse())
                    .endCell();

                val bouncedMsg = createMessage({
                    bounce: false,
                    value: ton("0.3"),
                    dest: echoAddress,
                    body: bouncedBody,
                }).bounced();

                val bouncedRes = net.send(sender.address, bouncedMsg);
                expect(bouncedRes).toHaveBouncedTx<EchoNotice>({
                    from: sender.address,
                    to: echoAddress,
                });
                expect(bouncedRes).toHaveTx<EchoNotice>({
                    from: sender.address,
                    to: echoAddress,
                    bounced: true,
                });
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_n/n_lib_api_send_single_bounce_roundtrip_matches_bounced_notify_opcode.stdout.txt",
        );
}

#[test]
fn n_lib_api_send_single_bounce_roundtrip_matches_failed_tx_exit_code() {
    let project = ProjectBuilder::new("n-lib-api-send-single-bounce-failed-match")
        .file("contracts/messages", ECHO_MESSAGES)
        .contract("echo", ECHO_FAIL_BOUNCE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"
            import "../contracts/messages"

            const ERR_REJECT_BOUNCE = 777;

            get fun `test-send-single-bounce-roundtrip-failed-exit-code`() {
                val sender = net.treasury("sender");

                val echoInit = ContractState {
                    code: build("echo"),
                    data: createEmptyCell(),
                };
                val echoAddress = AutoDeployAddress { stateInit: echoInit }.calculateAddress();

                val deployEcho = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: echoInit,
                    },
                });
                val deployEchoRes = net.send(sender.address, deployEcho);
                expect(deployEchoRes).toHaveSuccessfulDeploy({ to: echoAddress });

                val trigger = createMessage({
                    bounce: false,
                    value: ton("0.5"),
                    dest: echoAddress,
                    body: TriggerEcho {
                        queryId: 21,
                    },
                });

                val sendSingleRes = net.sendSingle(sender.address, trigger);
                expect(sendSingleRes.outMessages.size()).toEqual(1);

                val noticeBody = sendSingleRes.outMessages.at<EchoNotice>(0).loadBody().toCell();
                val bouncedBody = beginCell()
                    .storeUint(0xFFFFFFFF, 32)
                    .storeSlice(noticeBody.beginParse())
                    .endCell();

                val bouncedMsg = createMessage({
                    bounce: false,
                    value: ton("0.3"),
                    dest: echoAddress,
                    body: bouncedBody,
                }).bounced();

                val bouncedRes = net.send(sender.address, bouncedMsg);
                expect(bouncedRes).toHaveBouncedTx<EchoNotice>({
                    from: sender.address,
                    to: echoAddress,
                });
                expect(bouncedRes).toHaveFailedTx<EchoNotice>({
                    from: sender.address,
                    to: echoAddress,
                    bounced: true,
                    exitCode: ERR_REJECT_BOUNCE,
                });
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_n/n_lib_api_send_single_bounce_roundtrip_matches_failed_tx_exit_code.stdout.txt",
        );
}
