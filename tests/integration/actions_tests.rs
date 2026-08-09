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
            import "../gen/child.code.tolk"

            fun onInternalMessage(in: InMessage) {
                 val addr = AutoDeployAddress {
                    stateInit: ContractState {
                        code: childCompiledCode(),
                        data: createEmptyCell(),
                    },
                }.calculateAddress();

                reserveGramsOnBalance(grams("0.1"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

                val outMsg = createMessage({
                    dest: addr,
                    bounce: false,
                    value: grams("0.5"),
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
            import "../../lib/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            import "../gen/child.code.tolk"

            get fun `test action fail`() {
                val deployer = testing.treasury("deployer");

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
                    value: grams("0.2"),
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
        .assert_snapshot_matches("integration/snapshots/actions/test_action_fail.stdout.txt");
}

#[test]
fn test_invalid_action_fail() {
    let project = invalid_action_fail_project("action-fail").build();

    project
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("action fail")
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_invalid_action_fail.stdout.txt",
        );
}

#[test]
fn test_invalid_action_fail_without_backtrace() {
    let project = invalid_action_fail_project("action-fail").build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("action fail")
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_invalid_action_fail_without_backtrace.stdout.txt",
        );
}

#[test]
fn test_invalid_action_fail_without_backtrace_verbose() {
    let project = invalid_action_fail_project("action-fail-verbose").build();

    project
        .acton()
        .test()
        .verbose()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("action fail")
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_invalid_action_fail_without_backtrace_verbose.stdout.txt",
        );
}

#[test]
fn test_action_tree_shows_set_code_and_change_library_sources() {
    let project =
        code_and_library_action_failure_project("action-tree-code-and-library-sources").build();

    project
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_action_tree_shows_set_code_and_change_library_sources.stdout.txt",
        );
}

#[test]
fn test_action_tree_hides_code_and_library_actions_without_logs() {
    let project =
        code_and_library_action_failure_project("action-tree-code-and-library-sources-no-logs")
            .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_action_tree_hides_code_and_library_actions_without_logs.stdout.txt",
        );
}

#[test]
fn test_action_tree_shows_code_and_library_actions_without_locations_on_verbose() {
    let project =
        code_and_library_action_failure_project("action-tree-code-and-library-sources-verbose")
            .build();

    project
        .acton()
        .test()
        .verbose()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_action_tree_shows_code_and_library_actions_without_locations_on_verbose.stdout.txt",
        );
}

#[test]
fn test_action_tree_shows_code_and_library_action_locations_on_coverage() {
    let project =
        code_and_library_action_failure_project("action-tree-code-and-library-sources-coverage")
            .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("action-coverage.txt")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/actions/test_action_tree_shows_code_and_library_action_locations_on_coverage.stdout.txt",
        );
}

fn code_and_library_action_failure_project(project_name: &str) -> ProjectBuilder {
    ProjectBuilder::new(project_name)
        .contract(
            "simple",
            r#"
                fun contract.setLibraryCode(code: cell, mode: int): void
                    asm "SETLIBCODE"

                fun onInternalMessage(in: InMessage) {
                    contract.setCodePostponed(beginCell().storeUint(0xCA, 8).endCell());
                    contract.setLibraryCode(beginCell().storeUint(0xFE, 8).endCell(), 2);
                    reserveGramsOnBalance(grams("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
                }

                fun onBouncedMessage(_: InMessageBounced) {}
            "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test action metadata includes code and library updates`() {
                val deployer = testing.treasury("deployer");

                val addr = AutoDeployAddress {
                    stateInit: ContractState {
                        code: build("simple"),
                        data: createEmptyCell(),
                    },
                };

                val triggerMsg = createMessage({
                    bounce: false,
                    value: grams("0.2"),
                    dest: addr,
                });

                val res = net.send(deployer.address, triggerMsg);
                println(res);
            }
        "#,
        )
}

fn invalid_action_fail_project(project_name: &str) -> ProjectBuilder {
    ProjectBuilder::new(project_name)
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
            import "../../lib/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test action fail`() {
                val deployer = testing.treasury("deployer");

                 val addr = AutoDeployAddress {
                     stateInit: ContractState {
                         code: build("simple"),
                         data: createEmptyCell(),
                     },
                 };

                // Trigger internal message that will cause action fail
                val triggerMsg = createMessage({
                    bounce: false,
                    value: grams("0.2"),
                    dest: addr,
                });

                val res = net.send(deployer.address, triggerMsg);
                println(res);
            }
        "#,
        )
}
