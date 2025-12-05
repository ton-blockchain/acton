use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun currentCounter(): int { return 0 }
get fun currentCounter2(arg: int): int { return arg }
get fun currentCounter3(arg: int): int { return arg + 10 }
get fun getCell(): cell { return beginCell().storeInt(32, 32).endCell() }
"#;

const TEST_PREPARE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build/build"
import "../../lib/io"
import "../../lib/emulation/network"
import "../../lib/fmt"

struct Counter {
    address: address
    init: ContractState
}

fun Counter.fromStorage() {
    val init = ContractState {
        code: build("simple"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Counter { address, init }
}

fun setupTest() {
    val counter = Counter.fromStorage();

    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });

    net.send(deployer.address, msg, SEND_MODE_PAY_FEES_SEPARATELY);
    return (counter, deployer)
}
"#;

#[test]
fn test_unknown_get_method_call() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test-foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int, tuple>(counter.address, "currentCounter999");
                println(format1("Counter: {}", counterRes));
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_unknown_get_method_call.stdout.txt");
}

#[test]
fn test_get_method_call_return_type_mismatch() {
    // TODO: fow now we cannot check this
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test-foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<address, tuple>(counter.address, "getCell");
                println(format1("Counter: {}", counterRes));
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_get_method_call_return_type_mismatch.stdout.txt",
        );
}

#[test]
fn test_no_arg_get_method_call() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test-foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int, tuple>(counter.address, "currentCounter2");
                println(format1("Counter: {}", counterRes));
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_no_arg_get_method_call.stdout.txt");
}

#[test]
fn test_no_arg_get_method_call_2() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test-foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int, tuple>(counter.address, "currentCounter3");
                println(format1("Counter: {}", counterRes));
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_no_arg_get_method_call_2.stdout.txt");
}

#[test]
fn test_test_file_not_found() {
    let project = ProjectBuilder::new("test-not-found").build();

    project
        .acton()
        .test()
        .path("nonexistent_test.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_file_not_found.stderr.txt",
        );
}

#[test]
fn test_test_directory_not_found() {
    let project = ProjectBuilder::new("test-dir-not-found").build();

    project
        .acton()
        .test()
        .path("nonexistent_directory")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_directory_not_found.stderr.txt",
        );
}

#[test]
fn test_test_invalid_file_extension() {
    let project = ProjectBuilder::new("test-invalid-ext")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file("invalid.txt", "some content")
        .build();

    project
        .acton()
        .test()
        .path("invalid.txt")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_invalid_file_extension.stderr.txt",
        );
}

#[test]
fn test_test_invalid_filter_regex() {
    let project = ProjectBuilder::new("test-invalid-regex")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test-foo`() {
                // test
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .filter("[invalid regex")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_invalid_filter_regex.stderr.txt",
        );
}

#[test]
fn test_test_invalid_exclude_pattern() {
    let project = ProjectBuilder::new("test-invalid-exclude")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test-foo`() {
                // test
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .exclude_pattern("[invalid glob")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_invalid_exclude_pattern.stderr.txt",
        );
}

#[test]
fn test_test_invalid_include_pattern() {
    let project = ProjectBuilder::new("test-invalid-include")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test-foo`() {
                // test
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .include_pattern("[invalid glob")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_invalid_include_pattern.stderr.txt",
        );
}

#[test]
fn test_test_invalid_coverage_format() {
    let project = ProjectBuilder::new("test-invalid-coverage-format")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test-foo`() {
                // test
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("invalid-format")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_invalid_coverage_format.stderr.txt",
        );
}

#[test]
fn test_test_invalid_reporter() {
    let project = ProjectBuilder::new("test-invalid-reporter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test-foo`() {
                // test
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_reporter("invalid-reporter")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_test_invalid_reporter.stderr.txt",
        );
}

#[test]
fn test_invalid_test_file_syntax() {
    let project = ProjectBuilder::new("test-invalid-syntax")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test-foo`() {
                let a = 10;
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_invalid_test_file_syntax.stderr.txt",
        );
}

#[test]
fn test_build_unknown_file() {
    let project = ProjectBuilder::new("test-unknown-file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build/build"

            get fun `test-foo`() {
                val cell = build("counter", "unknown.tolk")
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_build_unknown_file.stdout.txt");
}

#[test]
fn test_build_unknown_contract() {
    let project = ProjectBuilder::new("test-unknown-file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build/build"

            get fun `test-foo`() {
                val cell = build("counter")
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_build_unknown_contract.stdout.txt");
}

#[test]
fn test_run_get_method_of_not_deployed_contract() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            get fun `test-foo`() {
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                val res: int = net.runGetMethod(address, "counter");
                println(res);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_run_get_method_of_not_deployed_contract.stdout.txt",
        );
}

#[test]
fn test_run_get_method_of_not_deployed_contract_with_backtrace_full() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            get fun `test-foo`() {
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                val res: int = net.runGetMethod(address, "counter");
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
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_run_get_method_of_not_deployed_contract_with_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_send_message_to_not_deployed_contract() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            get fun `test-foo`() {
                val sender = net.treasury("treasury");
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");

                val msg = createMessage({
                    dest: address,
                    body: createEmptyCell(),
                    bounce: false,
                    value: ton("1"),
                });
                val res = net.send(sender.address, msg, 0);
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
        .assert_snapshot_matches(
            "integration/snapshots/test_send_message_to_not_deployed_contract.stdout.txt",
        );
}

#[test]
fn test_send_message_to_not_deployed_contract_with_register() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            get fun `test-foo`() {
                val sender = net.treasury("treasury");
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                net.registerAddress(address, "some unknown contract");

                val msg = createMessage({
                    dest: address,
                    body: createEmptyCell(),
                    bounce: false,
                    value: ton("1"),
                });
                val res = net.send(sender.address, msg, 0);
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
        .assert_snapshot_matches(
            "integration/snapshots/test_send_message_to_not_deployed_contract_with_register.stdout.txt",
        );
}

#[test]
fn test_debug_logs_in_contract() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r#"
            fun onInternalMessage(in: InMessage) {
                debug.print(in.body);
                debug.print(in.senderAddress);
            }
            fun onBouncedMessage(_: InMessageBounced) {}
            "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            get fun `test-foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = net.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg, 0);
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
        .assert_snapshot_matches("integration/snapshots/test_debug_logs_in_contract.stdout.txt");
}
