use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CHILD_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
get fun childMarker(): int { return 1 }
"#;

#[test]
fn register_code_cell_labels_auto_deploy_transactions() {
    ProjectBuilder::new("lib-api-register-code-cell-label")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "register_code",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/io"

            get fun `test-register-code-cell-label`() {
                val deployer = net.treasury("q_deployer");
                val code = build("simple");
                net.registerCodeCell(code, "q_simple_alias");

                val init = ContractState {
                    code: code,
                    data: createEmptyCell(),
                };

                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });

                val res = net.send(deployer.address, msg);
                println(res);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_register_code_cell/register_code_cell_labels_auto_deploy_transactions.stdout.txt",
        );
}

#[test]
fn register_code_cell_last_registration_wins_for_same_hash() {
    ProjectBuilder::new("lib-api-register-code-cell-last-wins")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "register_code",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/io"

            get fun `test-register-code-cell-last-wins`() {
                val deployer = net.treasury("q_deployer");
                val code = build("simple");

                net.registerCodeCell(code, "q_alias_old");
                net.registerCodeCell(code, "q_alias_new");

                val init = ContractState {
                    code: code,
                    data: createEmptyCell(),
                };

                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });

                val res = net.send(deployer.address, msg);
                println(res);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_register_code_cell/register_code_cell_last_registration_wins_for_same_hash.stdout.txt",
        );
}

#[test]
fn register_code_cell_does_not_rename_other_contract_hashes() {
    ProjectBuilder::new("lib-api-register-code-cell-is-hash-specific")
        .contract("simple", SIMPLE_CONTRACT)
        .contract("child", CHILD_CONTRACT)
        .test_file(
            "register_code",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/io"

            get fun `test-register-code-cell-is-hash-specific`() {
                val deployer = net.treasury("q_deployer");
                net.registerCodeCell(build("simple"), "q_simple_alias");

                val simpleInit = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val simpleMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: simpleInit,
                    },
                });
                val simpleRes = net.send(deployer.address, simpleMsg);
                println(simpleRes);

                val childInit = ContractState {
                    code: build("child"),
                    data: createEmptyCell(),
                };
                val childMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: childInit,
                    },
                });
                val childRes = net.send(deployer.address, childMsg);
                println(childRes);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_register_code_cell/register_code_cell_does_not_rename_other_contract_hashes.stdout.txt",
        );
}

#[test]
fn register_address_name_has_priority_over_registered_code_name() {
    ProjectBuilder::new("lib-api-register-address-over-code-name")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "register_code",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/io"

            get fun `test-register-address-priority-over-code-name`() {
                val deployer = net.treasury("q_deployer");
                val code = build("simple");
                net.registerCodeCell(code, "q_code_alias");

                val init = ContractState {
                    code: code,
                    data: createEmptyCell(),
                };
                val target = AutoDeployAddress {
                    stateInit: init,
                }.calculateAddress();
                net.registerAddress(target, "q_address_alias");

                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });

                val res = net.send(deployer.address, msg);
                println(res);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_register_code_cell/register_address_name_has_priority_over_registered_code_name.stdout.txt",
        );
}

#[test]
fn register_code_cell_from_get_deployed_code_applies_to_future_transactions() {
    ProjectBuilder::new("lib-api-register-code-cell-from-deployed-code")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "register_code",
            r#"
            import "../../lib/build/build"
            import "../../lib/emulation/network"
            import "../../lib/io"

            get fun `test-register-code-cell-from-get-deployed-code`() {
                val deployer = net.treasury("q_deployer");
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val target = AutoDeployAddress {
                    stateInit: init,
                }.calculateAddress();

                val deployMsg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: {
                        stateInit: init,
                    },
                });
                net.send(deployer.address, deployMsg);

                val code = net.getDeployedCode(target);
                if (code == null) {
                    throw 555;
                }

                net.registerCodeCell(code, "q_loaded_alias");

                val pingMsg = createMessage({
                    bounce: false,
                    value: ton("0.2"),
                    body: createEmptyCell(),
                    dest: target,
                });
                val secondRes = net.send(deployer.address, pingMsg);
                println(secondRes);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_register_code_cell/register_code_cell_from_get_deployed_code_applies_to_future_transactions.stdout.txt",
        );
}
