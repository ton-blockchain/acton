use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use acton_config::color::ColorMode;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const GET_METHOD_FAILURE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun currentCounterFail(): int { throw 10 }
";

const GET_METHOD_FAILURE_TEST_PREPARE: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"

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
    return counter
}
"#;

const TEAMCITY_COMPLEX_COMPARISON_TESTS: &str = r#"
import "../../lib/testing/expect"

struct Point {
    x: int,
    y: int,
}

struct Segment {
    start: Point,
    end: Point,
}

fun balances(first: int32, second: int32): map<int32, int32> {
    var value = createEmptyMap<int32, int32>();
    value.set(1, first);
    value.set(2, second);
    return value;
}

get fun `test tuple diff`() {
    expect((10, 20, 30)).toEqual((10, 20, 31));
}

get fun `test struct diff`() {
    expect(Point { x: 1, y: 2 }).toEqual(Point { x: 1, y: 3 });
}

get fun `test nested struct diff`() {
    val actual = Segment {
        start: Point { x: 1, y: 2 },
        end: Point { x: 3, y: 4 },
    };
    val expected = Segment {
        start: Point { x: 1, y: 9 },
        end: Point { x: 3, y: 4 },
    };

    expect(actual).toEqual(expected);
}

get fun `test nullable diff`() {
    val actual: int? = 10;
    val expected: int? = null;

    expect(actual).toEqual(expected);
}

get fun `test map diff`() {
    expect(balances(10, 20)).toEqual(balances(10, 30));
}
"#;

const FUZZ_FAILURE_TESTS: &str = r#"
import "../../lib/testing/expect"

@test.fuzz({ runs: 2, seed: 17 })
get fun `test fuzz fails with inputs`(value: int) {
    expect(value).toEqual(1);
}
"#;

#[test]
fn test_teamcity_reporter_basic_passing() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains("##teamcity[testSuiteStarted")
        .assert_contains("##teamcity[testStarted")
        .assert_contains("##teamcity[testFinished")
        .assert_contains("##teamcity[testSuiteFinished")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_basic_passing.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_with_failing_test() {
    FixtureProject::load("basic")
        .with_contract_slot(1) // Enable "throw 10;" in contract
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .failure()
        .assert_failed(2)
        .assert_contains("##teamcity[testFailed")
        .assert_contains("exit_code=10")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_with_failing_test.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_with_get_method_failure() {
    ProjectBuilder::new("teamcity_get_method_failure")
        .contract("simple", GET_METHOD_FAILURE_CONTRACT)
        .test_file(
            "test",
            (GET_METHOD_FAILURE_TEST_PREPARE.to_string()
                + r#"
            get fun `test get method failure`() {
                val counter = setupTest();
                val _res: int = net.runGetMethod(counter.address, "currentCounterFail");
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("##teamcity[testFailed")
        .assert_contains("Cannot execute get method")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_with_get_method_failure.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_with_fuzz_failure_includes_seed_and_inputs() {
    ProjectBuilder::new("teamcity_fuzz_failure")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", FUZZ_FAILURE_TESTS)
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("##teamcity[testFailed")
        .assert_contains("Fuzz seed: 17")
        .assert_contains("Inputs: value=0")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_with_fuzz_failure.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_with_skipped_test() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .filter("nonexistent")
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .failure()
        .assert_passed(0)
        .assert_contains("##teamcity[testSuiteStarted")
        .assert_contains("##teamcity[testSuiteFinished")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_with_skipped_test.stdout.txt",
        );
}

#[test]
fn test_junit_reporter_basic_passing() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("junit")
        .run()
        .success()
        .assert_passed(2)
        .assert_file_exists("test-results/TEST-counter.test.tolk.xml")
        .assert_file_contains(
            "test-results/TEST-counter.test.tolk.xml",
            r#"<testsuite name="counter.test.tolk""#,
        )
        .assert_file_contains(
            "test-results/TEST-counter.test.tolk.xml",
            r#"<testcase name="test should increase counter""#,
        )
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_junit_basic_passing.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "test-results/TEST-counter.test.tolk.xml",
            "integration/snapshots/reporters/test_junit_basic_passing.xml.gen",
        );
}

#[test]
fn test_junit_reporter_with_failing_test() {
    FixtureProject::load("basic")
        .with_contract_slot(1) // Enable "throw 10;" in contract
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("junit")
        .run()
        .failure()
        .assert_failed(2)
        .assert_contains("exit_code=10")
        .assert_file_snapshot_matches(
            "test-results/TEST-counter.test.tolk.xml",
            "integration/snapshots/reporters/test_junit_reporter_with_failing_test.xml.gen",
        )
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_junit_with_failing_test.stdout.txt",
        );
}

#[test]
fn test_junit_reporter_with_fuzz_failure_includes_seed_and_inputs() {
    ProjectBuilder::new("junit_fuzz_failure")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", FUZZ_FAILURE_TESTS)
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("junit")
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Fuzz case 1/2")
        .assert_file_snapshot_matches(
            "test-results/TEST-test.test.tolk.xml",
            "integration/snapshots/reporters/test_junit_reporter_with_fuzz_failure.xml.gen",
        )
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_junit_with_fuzz_failure.stdout.txt",
        );
}

#[test]
fn test_junit_reporter_includes_captured_stdout_and_stderr() {
    ProjectBuilder::new("junit_captured_output")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "output",
            r#"
            import "../../lib/io"

            get fun `test junit captured output`() {
                println("junit stdout <&>");
                eprintln("junit stderr <&>");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("junit")
        .run()
        .success()
        .assert_file_snapshot_matches(
            "test-results/TEST-output.test.tolk.xml",
            "integration/snapshots/reporters/test_junit_reporter_includes_captured_stdout_and_stderr.xml.gen",
        );
}

#[test]
fn test_multiple_reporters_console_and_teamcity() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains("✓")
        .assert_contains("##teamcity[testSuiteStarted")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_multiple_reporters_console_teamcity.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_multiple_files() {
    ProjectBuilder::new("multi_file_teamcity")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "wallet_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test wallet balance`() {
                expect(1).toEqual(1);
            }

            get fun `test wallet utils`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "utils_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test pow2 basic`() {
                expect(1).toEqual(1);
            }

            get fun `test pow2 edge`() {
                expect(1).toEqual(2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .failure()
        .assert_passed(3)
        .assert_failed(1)
        .assert_contains("##teamcity[testSuiteStarted")
        .assert_contains("##teamcity[testSuiteFinished")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_multiple_files.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_with_skipped_and_todo_tests() {
    ProjectBuilder::new("teamcity_skip_todo")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "skip_todo",
            r#"
            import "../../lib/testing/expect"

            @test.skip("skip | ' []")
            get fun `test teamcity skipped`() {
                expect(1).toEqual(2);
            }

            @test.todo("todo | ' []")
            get fun `test teamcity todo`() {
                expect(1).toEqual(2);
            }

            get fun `test teamcity passes`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .success()
        .assert_skipped(1)
        .assert_todo(1)
        .assert_contains("##teamcity[testIgnored")
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_with_skipped_and_todo_tests.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_escapes_location_hint_special_chars() {
    let project = ProjectBuilder::new("teamcity_location_hint_escape")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/teamcity '[] .test.tolk",
            r"
            get fun `test teamcity '|[]`() {
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_reporter_escapes_location_hint_special_chars.stdout.txt",
        );
}

#[test]
fn test_teamcity_reporter_comparison_failure_snapshots_complex_values() {
    ProjectBuilder::new("teamcity_complex_comparison_failures")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("complex_diffs", TEAMCITY_COMPLEX_COMPARISON_TESTS)
        .build()
        .acton()
        .test()
        .with_reporter("teamcity")
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_teamcity_comparison_failures_complex_values.stdout.txt",
        );
}

#[test]
fn test_junit_reporter_multiple_files_with_failures() {
    ProjectBuilder::new("multi_file_junit_failures")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "wallet",
            r#"
            import "../../lib/testing/expect"

            get fun `test wallet balance`() {
                expect(1).toEqual(1);
            }

            get fun `test wallet utils`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "utils",
            r#"
            import "../../lib/testing/expect"

            get fun `test pow2 basic`() {
                expect(1).toEqual(1);
            }

            get fun `test pow2 edge`() {
                expect(1).toEqual(2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("junit")
        .run()
        .failure()
        .assert_passed(3)
        .assert_failed(1)
        .assert_file_snapshot_matches(
            "test-results/TEST-wallet.test.tolk.xml",
            "integration/snapshots/reporters/test_junit_reporter_multiple_files_with_failures_wallet_test.xml.gen",
        )
        .assert_file_snapshot_matches(
            "test-results/TEST-utils.test.tolk.xml",
            "integration/snapshots/reporters/test_junit_reporter_multiple_files_with_failures_utils_test.xml.gen",
        )
        .assert_snapshot_matches(
            "integration/snapshots/reporters/test_junit_multiple_files_with_failures.stdout.txt",
        );
}

#[test]
fn test_junit_reporter_merge_keeps_suites_with_same_basename_in_different_dirs() {
    let project = ProjectBuilder::new("junit_merge_same_basename")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/a/shared.test.tolk",
            r"
            get fun `test shared a`() {
            }
        ",
        )
        .raw_file(
            "tests/b/shared.test.tolk",
            r"
            get fun `test shared b`() {
            }
        ",
        )
        .build();

    project
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("junit")
        .with_junit_merge()
        .run()
        .success()
        .assert_passed(2)
        .assert_file_snapshot_matches(
            "test-results/junit-results.xml",
            "integration/snapshots/reporters/test_junit_reporter_merge_keeps_suites_with_same_basename_in_different_dirs.xml.gen",
        );
}

#[test]
fn test_junit_reporter_with_merge() {
    ProjectBuilder::new("junit_merge_test")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "first_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test first`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "second_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test second`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("junit")
        .with_junit_merge()
        .run()
        .success()
        .assert_passed(2)
        .assert_file_exists("test-results/junit-results.xml")
        .assert_file_contains("test-results/junit-results.xml", r"<testsuites")
        .assert_file_contains("test-results/junit-results.xml", r"<testsuite")
        .assert_file_snapshot_matches(
            "test-results/junit-results.xml",
            "integration/snapshots/reporters/test_junit_with_merge.xml.gen",
        );
}

#[test]
fn test_dot_reporter_basic() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .success()
        .assert_contains("··")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_basic.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_with_failures() {
    FixtureProject::load("basic")
        .with_contract_slot(1) // Enable "throw 10;" in contract
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure()
        .assert_contains("xx")
        .assert_contains("✗ test should increase counter")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_with_failures.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_with_fuzz_failure_includes_seed_and_inputs() {
    ProjectBuilder::new("dot_fuzz_failure")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", FUZZ_FAILURE_TESTS)
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure()
        .assert_contains("✗ test fuzz fails with inputs")
        .assert_contains("Fuzz seed: 17")
        .assert_contains("Inputs: value=0")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_with_fuzz_failure.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_with_skipped_and_todo_tests() {
    ProjectBuilder::new("dot_skip_todo")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test.skip("not relevant for this run")
            get fun `test dot skipped`() {
                expect(1).toEqual(2);
            }

            @test.todo("cover later")
            get fun `test dot todo`() {
                expect(1).toEqual(2);
            }

            get fun `test dot passes`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .success()
        .assert_contains("○□·")
        .assert_snapshot_matches("integration/snapshots/test-runner/reporters/dot/test_dot_with_skipped_and_todo.stdout.txt");
}

#[test]
fn test_dot_reporter_failure_details_color_snapshot() {
    ProjectBuilder::new("dot_failure_details_color")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test dot colored diff`() {
                expect(1).toEqual(2);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .failure()
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_failure_details_colors.stdout.svg",
        );
}

#[test]
fn test_dot_reporter_with_get_method_failure() {
    ProjectBuilder::new("dot_get_method_failure")
        .contract("simple", GET_METHOD_FAILURE_CONTRACT)
        .test_file(
            "test",
            (GET_METHOD_FAILURE_TEST_PREPARE.to_string()
                + r#"
            get fun `test get method failure`() {
                val counter = setupTest();
                val _res: int = net.runGetMethod(counter.address, "currentCounterFail");
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure()
        .assert_contains("✗ test get method failure")
        .assert_contains("Cannot execute get method")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_with_get_method_failure.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_with_get_method_failure_and_backtrace_full() {
    ProjectBuilder::new("dot_get_method_failure_backtrace")
        .contract("simple", GET_METHOD_FAILURE_CONTRACT)
        .test_file(
            "test",
            (GET_METHOD_FAILURE_TEST_PREPARE.to_string()
                + r#"
            get fun `test get method failure with backtrace`() {
                val counter = setupTest();
                val _res: int = net.runGetMethod(counter.address, "currentCounterFail");
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .with_backtrace("full")
        .run()
        .failure()
        .assert_contains("✗ test get method failure with backtrace")
        .assert_contains("Backtrace:")
        .assert_contains("Called from:")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_with_get_method_failure_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_with_special_get_method_exit_code_hints() {
    ProjectBuilder::new("dot_get_method_special_exit_codes")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"

            get fun `test dot not deployed get method hint`() {
                val address = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                val _res: int = net.runGetMethod(address, "counter");
            }

            get fun `test dot contract without code get method hint`() {
                val deployer = testing.treasury("deployer");
                val address = AutoDeployAddress {
                    stateInit: beginCell()
                        .storeBool(false)
                        .storeBool(false)
                        .storeBool(false)
                        .storeBool(false)
                        .storeBool(false)
                        .endCell(),
                };

                val outMsg = createMessage({
                    bounce: BounceMode.NoBounce,
                    value: ton("0.1"),
                    dest: address,
                });
                net.send(deployer.address, outMsg);

                val _res: int = net.runGetMethod(address.calculateAddress(), "counter");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure()
        .assert_contains("Cannot run method of not deployed contract")
        .assert_contains("Cannot run method of contract without code")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_special_get_method_exit_code_hints.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_runtime_colors_include_backtrace_and_stderr() {
    ProjectBuilder::new("dot_runtime_color")
        .contract("simple", GET_METHOD_FAILURE_CONTRACT)
        .test_file(
            "test",
            ("import \"../../lib/io\"\n".to_string()
                + GET_METHOD_FAILURE_TEST_PREPARE
                + r#"
            get fun `test dot colored runtime details`() {
                eprintln("colored stderr line");
                val counter = setupTest();
                val _res: int = net.runGetMethod(counter.address, "currentCounterFail");
            }
        "#)
            .as_str(),
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .with_backtrace("full")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .failure()
        .assert_contains("stderr |")
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_runtime_colors.stdout.svg",
        );
}

#[test]
fn test_dot_reporter_transaction_matcher_failures_include_trees() {
    let output = ProjectBuilder::new("dot_transaction_matcher_failures")
        .contract(
            "failing",
            r"
            fun onInternalMessage(_: InMessage) {
                throw 77;
            }

            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .test_file(
            "tx_failures",
            r#"
            import "../../lib/build"
            import "../../lib/emulation/network"
            import "../../lib/emulation/testing"
            import "../../lib/testing/expect"

            fun sendFailing() {
                val init = ContractState {
                    code: build("failing"),
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

                return net.send(sender.address, msg);
            }

            get fun `test dot missing successful tx tree`() {
                val results = sendFailing();
                expect(results).toHaveSuccessfulTx();
            }

            get fun `test dot unexpected failed tx tree`() {
                val results = sendFailing();
                expect(results).toHaveAllSuccessfulTxs();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure();

    output
        .assert_contains("Cannot find transaction")
        .assert_contains("Unexpected transaction")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_transaction_matcher_failures.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_runtime_failure_details() {
    ProjectBuilder::new("dot_runtime_failure_details")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "runtime",
            r#"
            import "../../lib/testing/expect"

            @test.gas_limit(100)
            get fun `test dot gas limit exceeded`() {
                var i = 0;
                while (i < 1000) {
                    i = i + 1;
                }
                expect(1).toEqual(1);
            }

            @test.fail_with(42)
            get fun `test dot wrong expected exit code`() {
                throw 99;
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure()
        .assert_contains("Gas limit exceeded")
        .assert_contains("Expected exit_code=42, got=99")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_runtime_failure_details.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_prints_failures_before_coverage() {
    ProjectBuilder::new("dot_failure_before_coverage")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/validator",
            r"
            fun validate(value: int): bool {
                if (value > 0) {
                    return true;
                }
                return false;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/validator"

            get fun `test dot coverage failing`() {
                val result = validate(10);
                expect(result).toEqual(false);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .with_coverage()
        .run()
        .failure()
        .assert_contains("✗ test dot coverage failing")
        .assert_contains(" COVERAGE ")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_failures_before_coverage.stdout.txt",
        );
}

#[test]
fn test_dot_reporter_multiple_files() {
    ProjectBuilder::new("dot_multi_file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "first_test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/io"

            get fun `test first`() {
                println("First test output");
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "second_test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/io"

            get fun `test second`() {
                expect(2).toEqual(2);
            }

            get fun `test second fail`() {
                println("This test will fail");
                eprintln("Error output");
                expect(1).toEqual(2); // This will fail
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("dot")
        .run()
        .failure()
        .assert_contains("··x")
        .assert_contains("stdout |")
        .assert_contains("stderr |")
        .assert_contains("First test output")
        .assert_contains("This test will fail")
        .assert_contains("Error output")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/reporters/dot/test_dot_multiple_files.stdout.txt",
        );
}
