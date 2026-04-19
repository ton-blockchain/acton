use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};
use std::fs;
use toml_edit::DocumentMut;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun currentCounter(): int { return 0 }
get fun currentCounter2(arg: int): int { return arg }
get fun currentCounter3(arg: int): int { return arg + 10 }
get fun currentCounterFail(): int { throw 10 }
get fun getCell(): cell { return beginCell().storeInt(32, 32).endCell() }
";

const CUSTOM_GET_EXIT_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

enum Errors {
    AbiFailure = 709
}

get fun currentCounter(): int { return 0 }
get fun currentCounterCustomFail(): int { throw Errors.AbiFailure }
";

const TEST_PREPARE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build"
import "../../lib/io"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
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

    val deployer = testing.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: counter.init,
        },
    });

    net.send(deployer.address, msg);
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

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounter999");
                println("Counter: {}", counterRes);
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
fn test_unknown_get_method_call_with_backtrace_full() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounter999");
                println("Counter: {}", counterRes);
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_unknown_get_method_call_with_backtrace_full.stdout.txt",
        );
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

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<address>(counter.address, "getCell");
                println("Counter: {}", counterRes);
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

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounter2");
                println("Counter: {}", counterRes);
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

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounter3");
                println("Counter: {}", counterRes);
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
fn test_no_arg_get_method_call_2_with_backtrace_full() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounter3");
                println("Counter: {}", counterRes);
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_no_arg_get_method_call_2_with_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_get_method_call_shows_exit_code_variant() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounterFail");
                println("Counter: {}", counterRes);
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_get_method_call_shows_exit_code_variant.stdout.txt",
        );
}

#[test]
fn test_get_method_call_uses_contract_abi_for_custom_exit_code() {
    ProjectBuilder::new("simple-custom-exit")
        .contract("simple", CUSTOM_GET_EXIT_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounterCustomFail");
                println("Counter: {}", counterRes);
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_not_contains("Error: Errors.AbiFailure")
        .assert_snapshot_matches(
            "integration/snapshots/test_get_method_call_uses_contract_abi_for_custom_exit_code.stdout.txt",
        );
}

#[test]
fn test_get_method_call_uses_contract_abi_for_custom_exit_code_with_backtrace_full() {
    ProjectBuilder::new("simple-custom-exit-backtrace")
        .contract("simple", CUSTOM_GET_EXIT_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounterCustomFail");
                println("Counter: {}", counterRes);
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_not_contains("Error: Errors.AbiFailure")
        .assert_snapshot_matches(
            "integration/snapshots/test_get_method_call_uses_contract_abi_for_custom_exit_code_with_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_get_method_call_shows_backtrace_with_full_mode() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounterFail");
                println("Counter: {}", counterRes);
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_get_method_call_shows_backtrace_with_full_mode.stdout.txt",
        );
}

#[test]
fn test_get_method_call_shows_backtrace_with_full_mode_from_config() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .with_test_config(TestConfig {
            backtrace: Some("full".to_string()),
            ..TestConfig::default()
        })
        .test_file(
            "test",
            (TEST_PREPARE.to_string()
                + r#"

            get fun `test foo`() {
                val (counter, deployer) = setupTest();

                val counterRes = net.runGetMethod<int>(counter.address, "currentCounterFail");
                println("Counter: {}", counterRes);
            }
        "#)
                .as_str(),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_get_method_call_shows_backtrace_with_full_mode_from_config.stdout.txt",
        );
}

#[test]
fn test_debug_dump_stack_output() {
    let project = ProjectBuilder::new("test-debug-dump-stack-output")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r"
            get fun `test debug dump stack output`() {
                debug.dumpStack();
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .verbose()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches("integration/snapshots/test_debug_dump_stack_output.stdout.txt");
}

#[test]
fn test_debug_dump_stack_output_mixed_with_stdout_and_stderr() {
    let project = ProjectBuilder::new("test-debug-dump-stack-mixed-output")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"

            get fun `test debug dump stack mixed output`() {
                println("before");
                debug.dumpStack();
                println("after");
                eprintln("err");
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .verbose()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("Test output:")
        .assert_contains("Test stderr:")
        .assert_snapshot_matches(
            "integration/snapshots/test_debug_dump_stack_output_mixed_with_stdout_and_stderr.stdout.txt",
        );
}

#[test]
fn test_debug_dump_stack_output_multiple_debug_lines() {
    let project = ProjectBuilder::new("test-debug-dump-stack-multiple-debug-lines")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            get fun `test debug dump stack multiple debug lines`() {
                debug.printString("dbg-line");
                debug.dumpStack();
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
        .assert_contains("dbg-line")
        .assert_snapshot_matches(
            "integration/snapshots/test_debug_dump_stack_output_multiple_debug_lines.stdout.txt",
        );
}

#[test]
fn test_debug_dump_stack_output_requires_verbose_flag() {
    let project = ProjectBuilder::new("test-debug-dump-stack-output-default-off")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r"
            get fun `test debug dump stack output requires verbose`() {
                debug.dumpStack();
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_not_contains("stack(0 values)")
        .assert_snapshot_matches(
            "integration/snapshots/test_debug_dump_stack_output_requires_verbose_flag.stdout.txt",
        );
}

#[test]
fn test_debug_dump_stack_output_rejects_verbose_level_above_one() {
    let project = ProjectBuilder::new("test-debug-dump-stack-output-verbose-level")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r"
            get fun `test debug dump stack output rejects verbose level above one`() {
                debug.dumpStack();
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .arg("-vv")
        .run()
        .failure()
        .assert_stderr_contains("Verbosity levels above 1 are not supported yet")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_debug_dump_stack_output_rejects_verbose_level_above_one.stderr.txt",
        );
}

#[test]
fn test_test_file_not_found() {
    let project = ProjectBuilder::new("test-not-found").build();

    project
        .acton()
        .test()
        .path("nonexistent.test.tolk")
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
            r"
            get fun `test foo`() {
                // test
            }
        ",
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
            r"
            get fun `test foo`() {
                // test
            }
        ",
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
            r"
            get fun `test foo`() {
                // test
            }
        ",
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
            r"
            get fun `test foo`() {
                // test
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("invalid-format")
        .run()
        .failure()
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
            r"
            get fun `test foo`() {
                // test
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .with_reporter("invalid-reporter")
        .run()
        .failure()
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
            r"
            get fun `test foo`() {
                let a = 10;
            }
        ",
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
            import "../../lib/build"

            get fun `test foo`() {
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
            import "../../lib/build"

            get fun `test foo`() {
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
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
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
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
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
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
                val sender = testing.treasury("treasury");
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");

                val msg = createMessage({
                    dest: address,
                    body: createEmptyCell(),
                    bounce: false,
                    value: ton("1"),
                });
                val res = net.send(sender.address, msg);
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
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
                val sender = testing.treasury("treasury");
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                net.registerAddress(address, "some unknown contract");

                val msg = createMessage({
                    dest: address,
                    body: createEmptyCell(),
                    bounce: false,
                    value: ton("1"),
                });
                val res = net.send(sender.address, msg);
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
fn test_run_get_method_of_deployed_contract_with_null_code() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
                val deployer = testing.treasury("deployer");
                val address = AutoDeployAddress {
                    stateInit: beginCell()
                        .storeBool(false) // fixed_prefix_length:(Maybe (## 5))
                        .storeBool(false) // special:(Maybe TickTock)
                        .storeBool(false) // code:(Maybe ^Cell)
                        .storeBool(false) // data:(Maybe ^Cell)
                        .storeBool(false) // library:(Maybe ^Cell)
                        .endCell(),
                };

                val outMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    value: ton("0.1"),
                    dest: address,
                });
                net.send(deployer.address, outMsg);

                val res: int = net.runGetMethod(address.calculateAddress(), "counter");
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
            "integration/snapshots/test_run_get_method_of_deployed_contract_with_null_code.stdout.txt",
        );
}

#[test]
fn test_run_get_method_of_deployed_contract_with_null_code_with_backtrace_full() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
                val deployer = testing.treasury("deployer");
                val address = AutoDeployAddress {
                    stateInit: beginCell()
                        .storeBool(false) // fixed_prefix_length:(Maybe (## 5))
                        .storeBool(false) // special:(Maybe TickTock)
                        .storeBool(false) // code:(Maybe ^Cell)
                        .storeBool(false) // data:(Maybe ^Cell)
                        .storeBool(false) // library:(Maybe ^Cell)
                        .endCell(),
                };

                val outMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    value: ton("0.1"),
                    dest: address,
                });
                net.send(deployer.address, outMsg);

                val res: int = net.runGetMethod(address.calculateAddress(), "counter");
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
            "integration/snapshots/test_run_get_method_of_deployed_contract_with_null_code_with_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_send_invalid_message() {
    let project = ProjectBuilder::new("test-get")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
                val deployer = testing.treasury("deployer");
                val address = AutoDeployAddress {
                    stateInit: beginCell()
                        .storeBool(false) // fixed_prefix_length:(Maybe (## 5))
                        .endCell(),
                };

                val outMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    value: ton("0.1"),
                    dest: address,
                });
                net.send(deployer.address, outMsg);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_send_invalid_message.stdout.txt");
}

#[test]
fn test_debug_logs_in_contract() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r"
            fun onInternalMessage(in: InMessage) {
                debug.print(in.body);
                debug.print(in.senderAddress);
            }
            fun onBouncedMessage(_: InMessageBounced) {}
            ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = testing.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg);
                println(sender.address);
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

#[test]
fn test_filter_all_test() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r"
                get fun `test foo`() {}
                get fun `test bar`() {}
            ",
        )
        .build()
        .acton()
        .test()
        .filter("1111111")
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_filter_all_test.stdout.txt");
}

#[test]
fn test_filter_all_test_with_several_test_files() {
    ProjectBuilder::new("simple")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r"
                get fun `test foo`() {}
                get fun `test bar`() {}
            ",
        )
        .test_file(
            "test2",
            r"
                get fun `test baz`() {}
                get fun `test qux`() {}
            ",
        )
        .build()
        .acton()
        .test()
        .filter("1111111")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_filter_all_test_with_several_test_files.stdout.txt",
        );
}

#[test]
fn test_auto_register_refs_if_any() {
    let project = ProjectBuilder::new("dep-lib")
        .contract("lib", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            r#"
            import "../gen/lib.code.tolk"

            fun onInternalMessage(in: InMessage) {
                 val address = AutoDeployAddress {
                    stateInit: ContractState {
                        code: libCompiledCode(),
                        data: createEmptyCell(),
                    },
                };

                val outMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    value: ton("0.1"),
                    dest: address,
                });
                outMsg.send(SEND_MODE_PAY_FEES_SEPARATELY);
            }
        "#,
            vec![("lib", Some("library_ref"), None, None)],
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
                val address = AutoDeployAddress {
                    stateInit: ContractState {
                        code: build("main"),
                        data: createEmptyCell(),
                    },
                };

                // Trigger internal message that will cause action fail
                val triggerMsg = createMessage({
                    bounce: false,
                    value: ton("0.2"),
                    dest: address,
                });

                val res = net.send(deployer.address, triggerMsg);
                println(res);
            }
        "#,
        )
        .build();

    let output = project.acton().test().run().success();

    output
        .assert_snapshot_matches("integration/snapshots/test_auto_register_refs_if_any.stdout.txt");
}

fn replace_library_ref_boc(generated: &str, new_boc_b64: &str) -> String {
    let marker = "\" base64>B B>boc hashu";
    let marker_idx = generated
        .find(marker)
        .expect("generated dependency file must contain library_ref asm marker");
    let open_quote_idx = generated[..marker_idx]
        .rfind('"')
        .expect("generated dependency file must contain opening quote before boc");
    let value_start = open_quote_idx + 1;

    format!(
        "{}{}{}",
        &generated[..value_start],
        new_boc_b64,
        &generated[marker_idx..]
    )
}

#[test]
fn test_missing_library_ref_is_reported_in_transaction_tree() {
    let project = ProjectBuilder::new("dep-lib-missing-library-ref")
        .contract("lib", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            r#"
            import "../gen/lib.code.tolk"

            fun onInternalMessage(in: InMessage) {
                if (in.body.isEmpty()) {
                    return;
                }

                val childInit = ContractState {
                    code: libCompiledCode(),
                    data: createEmptyCell(),
                };

                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.2"),
                    dest: {
                        stateInit: childInit,
                    },
                });

                outMsg.send(SEND_MODE_PAY_FEES_SEPARATELY);
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![("lib", Some("library_ref"), None, None)],
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/build"
            import "../../lib/io"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test missing library ref is reported`() {
                val deployer = testing.treasury("deployer");
                val mainStateInit = ContractState {
                    code: build("main"),
                    data: createEmptyCell(),
                };
                val mainAddress = AutoDeployAddress { stateInit: mainStateInit }.calculateAddress();

                val deployMain = net.send(
                    deployer.address,
                    createMessage({
                        bounce: false,
                        value: ton("1"),
                        dest: {
                            stateInit: mainStateInit,
                        },
                    }),
                );
                expect(deployMain).toHaveLength(1);

                val triggerRes = net.send(
                    deployer.address,
                    createMessage({
                        bounce: false,
                        value: ton("1"),
                        dest: mainAddress,
                        body: beginCell().storeUint(1, 32).endCell(),
                    }),
                );

                println(triggerRes);
            }
        "#,
        )
        .build();

    project.acton().build().run().success();

    let generated_dep_path = project.path().join("gen/lib.code.tolk");
    let generated_dep = fs::read_to_string(&generated_dep_path)
        .expect("must read generated dependency function for lib");
    let empty_cell_boc = Boc::encode_base64(Cell::default());
    let tampered_dep = replace_library_ref_boc(&generated_dep, &empty_cell_boc);

    let main_contract_path = project.path().join("contracts/main.tolk");
    let main_contract =
        fs::read_to_string(&main_contract_path).expect("must read main contract source");
    let main_contract_without_import =
        main_contract.replace("import \"../gen/lib.code.tolk\"\n", "");
    fs::write(
        &main_contract_path,
        format!("{main_contract_without_import}\n{tampered_dep}\n"),
    )
    .expect("must rewrite main contract with tampered library_ref function");

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml: DocumentMut = fs::read_to_string(&acton_toml_path)
        .expect("must read Acton.toml")
        .parse()
        .expect("Acton.toml must parse");
    acton_toml["contracts"]["main"]["depends"] =
        toml_edit::Item::Value(toml_edit::Value::Array(toml_edit::Array::default()));
    fs::write(&acton_toml_path, acton_toml.to_string()).expect("must update Acton.toml");

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_missing_library_ref_is_reported_in_transaction_tree.stdout.txt",
        );
}

#[test]
fn test_test_success_search_param_for_tx_with_compute_exit_code_10() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r"
            fun onInternalMessage(in: InMessage) {
                throw 10
            }
            ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = testing.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg);
                expect(res).toHaveSuccessfulTx();
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_test_success_search_param_for_tx_with_compute_exit_code_10.stdout.txt");
}

#[test]
fn test_test_success_search_param_for_tx_with_action_exit_code_37() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r#"
            fun onInternalMessage(in: InMessage) {
                reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
            }
            "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = testing.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg);
                expect(res).toHaveSuccessfulTx();
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_test_success_search_param_for_tx_with_action_exit_code_37.stdout.txt");
}

#[test]
fn test_test_success_search_param_for_tx_with_both_compute_and_action_exit_code() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r#"
            fun onInternalMessage(in: InMessage) {
                reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
                throw 10
            }
            "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = testing.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg);
                expect(res).toHaveSuccessfulTx();
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_test_success_search_param_for_tx_with_both_compute_and_action_exit_code.stdout.txt");
}

#[test]
fn test_test_all_successful_tx_matcher_with_fail() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r"
            fun onInternalMessage(in: InMessage) {
                throw 10
            }
            ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = testing.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg);
                expect(res).toHaveAllSuccessfulTxs();
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
            "integration/snapshots/test_test_all_successful_tx_matcher_with_fail.stdout.txt",
        );
}

#[test]
fn test_test_all_successful_tx_matcher_without_fail() {
    let project = ProjectBuilder::new("test-get")
        .contract(
            "simple",
            r"
            fun onInternalMessage(in: InMessage) {
                throw 0
            }
            ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/io"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress {
                    stateInit: init,
                };

                val sender = testing.treasury("sender");
                val msg = createMessage({
                    bounce: false,
                    value: ton("1"),
                    dest: address,
                    body: beginCell().storeUint(1, 32).endCell(),
                });
                val res = net.send(sender.address, msg);
                expect(res).toHaveAllSuccessfulTxs();
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
            "integration/snapshots/test_test_all_successful_tx_matcher_without_fail.stdout.txt",
        );
}

#[test]
fn test_expect_to_equal_decimal_success() {
    ProjectBuilder::new("decimal-success")
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                expect(1500000000).toEqualDecimal(1500000000, 9);
                expect(-1500000000).toEqualDecimal(-1500000000, 9);
                expect(100).toEqualDecimal(100, 0);
                expect(100).toEqualDecimal(100, 2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success();
}

#[test]
fn test_expect_to_equal_decimal_failure() {
    ProjectBuilder::new("decimal-failure")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test foo`() {
                expect(1500000000).toEqualDecimal(1600000000, 9);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test_expect_to_equal_decimal_failure.stdout.txt",
        );
}
