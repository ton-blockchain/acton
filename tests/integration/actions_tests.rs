use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn test_action_fail() {
    let project = ProjectBuilder::new("action-fail")
        .contract(
            "child",
            r"
            fun onInternalMessage(in: InMessage) {}
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .contract_with_deps(
            "simple",
            r#"
            import "../gen/child_code.tolk"

            fun onInternalMessage(in: InMessage) {
                 val addr = AutoDeployAddress {
                    stateInit: ContractState {
                        code: childCompiledCode(),
                        data: createEmptyCell(),
                    },
                }.calculateAddress();

                reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

                val outMsg = createMessage({
                    dest: addr,
                    bounce: false,
                    value: ton("0.5"),
                });
                outMsg.send(SEND_MODE_REGULAR);
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec!["child"],
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/testing/transaction_expect"

            import "../gen/child_code.tolk"

            get fun `test action fail`() {
                val deployer = net.treasury("deployer");

                {
                    val addr = AutoDeployAddress {
                        stateInit: ContractState {
                            code: childCompiledCode(),
                            data: createEmptyCell(),
                        },
                    }.calculateAddress();
                    net.registerAddress(addr, "new-child");
                }

                 val addr = AutoDeployAddress {
                     stateInit: ContractState {
                         code: build("simple"),
                         data: createEmptyCell(),
                     },
                 };

                // Trigger internal message that will cause action fail
                val triggerMsg = createMessage({
                    bounce: false,
                    value: ton("0.2"),
                    dest: addr,
                });

                val res = net.send(deployer.address, triggerMsg);
                println(res);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("action fail")
        .assert_snapshot_matches("integration/snapshots/test_action_fail.stdout.txt");
}

#[test]
fn test_invalid_action_fail() {
    let project = ProjectBuilder::new("action-fail")
        .contract(
            "simple",
            r"
                fun onInternalMessage(in: InMessage) {
                    sendRawMessage(beginCell().endCell(), 0);
                }
            ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/testing/transaction_expect"

            get fun `test action fail`() {
                val deployer = net.treasury("deployer");

                 val addr = AutoDeployAddress {
                     stateInit: ContractState {
                         code: build("simple"),
                         data: createEmptyCell(),
                     },
                 };

                // Trigger internal message that will cause action fail
                val triggerMsg = createMessage({
                    bounce: false,
                    value: ton("0.2"),
                    dest: addr,
                });

                val res = net.send(deployer.address, triggerMsg);
                println(res);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("action fail")
        .assert_snapshot_matches("integration/snapshots/test_invalid_action_fail.stdout.txt");
}

#[test]
fn test_invalid_action_fail_without_backtrace() {
    let project = ProjectBuilder::new("action-fail")
        .contract(
            "simple",
            r"
                fun onInternalMessage(in: InMessage) {
                    sendRawMessage(beginCell().endCell(), 0);
                }
            ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/testing/transaction_expect"

            get fun `test action fail`() {
                val deployer = net.treasury("deployer");

                 val addr = AutoDeployAddress {
                     stateInit: ContractState {
                         code: build("simple"),
                         data: createEmptyCell(),
                     },
                 };

                // Trigger internal message that will cause action fail
                val triggerMsg = createMessage({
                    bounce: false,
                    value: ton("0.2"),
                    dest: addr,
                });

                val res = net.send(deployer.address, triggerMsg);
                println(res);
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
        .assert_contains("action fail")
        .assert_snapshot_matches(
            "integration/snapshots/test_invalid_action_fail_without_backtrace.stdout.txt",
        );
}
