use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
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
        .assert_snapshot_matches("integration/snapshots/test_teamcity_basic_passing.stdout.txt");
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
            "integration/snapshots/test_teamcity_with_failing_test.stdout.txt",
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
        .success()
        .assert_passed(0)
        .assert_contains("##teamcity[testSuiteStarted")
        .assert_contains("##teamcity[testSuiteFinished")
        .assert_snapshot_matches(
            "integration/snapshots/test_teamcity_with_skipped_test.stdout.txt",
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
        .assert_file_exists("test-results/TEST-counter_test.tolk.xml")
        .assert_file_contains(
            "test-results/TEST-counter_test.tolk.xml",
            r#"<testsuite name="counter_test.tolk""#,
        )
        .assert_file_contains(
            "test-results/TEST-counter_test.tolk.xml",
            r#"<testcase name="test-should-increase-counter""#,
        )
        .assert_snapshot_matches("integration/snapshots/test_junit_basic_passing.stdout.txt")
        .assert_file_snapshot_matches(
            "test-results/TEST-counter_test.tolk.xml",
            "integration/snapshots/test_junit_basic_passing.xml.gen",
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
            "test-results/TEST-counter_test.tolk.xml",
            "integration/snapshots/test_junit_reporter_with_failing_test.xml.gen",
        )
        .assert_snapshot_matches("integration/snapshots/test_junit_with_failing_test.stdout.txt");
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
            "integration/snapshots/test_multiple_reporters_console_teamcity.stdout.txt",
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

            get fun test_wallet_balance() {
                expect(1).toEqual(1);
            }

            get fun test_wallet_utils() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "utils_test",
            r#"
            import "../../lib/testing/expect"

            get fun test_pow2_basic() {
                expect(1).toEqual(1);
            }

            get fun test_pow2_edge() {
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
        .assert_snapshot_matches("integration/snapshots/test_teamcity_multiple_files.stdout.txt");
}

#[test]
fn test_junit_reporter_multiple_files_with_failures() {
    ProjectBuilder::new("multi_file_junit_failures")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "wallet",
            r#"
            import "../../lib/testing/expect"

            get fun test_wallet_balance() {
                expect(1).toEqual(1);
            }

            get fun test_wallet_utils() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "utils",
            r#"
            import "../../lib/testing/expect"

            get fun test_pow2_basic() {
                expect(1).toEqual(1);
            }

            get fun test_pow2_edge() {
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
            "test-results/TEST-wallet_test.tolk.xml",
            "integration/snapshots/test_junit_reporter_multiple_files_with_failures_wallet_test.xml.gen",
        )
        .assert_file_snapshot_matches(
            "test-results/TEST-utils_test.tolk.xml",
            "integration/snapshots/test_junit_reporter_multiple_files_with_failures_utils_test.xml.gen",
        )
        .assert_snapshot_matches(
            "integration/snapshots/test_junit_multiple_files_with_failures.stdout.txt",
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

            get fun test_first() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "second_test",
            r#"
            import "../../lib/testing/expect"

            get fun test_second() {
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
        .assert_file_contains("test-results/junit-results.xml", r#"<testsuites"#)
        .assert_file_contains("test-results/junit-results.xml", r#"<testsuite"#)
        .assert_file_snapshot_matches(
            "test-results/junit-results.xml",
            "integration/snapshots/test_junit_with_merge.xml.gen",
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
        .assert_snapshot_matches("integration/snapshots/test_dot_basic.stdout.txt");
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
        .assert_contains("FAIL")
        .assert_snapshot_matches("integration/snapshots/test_dot_with_failures.stdout.txt");
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

            get fun test_first() {
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

            get fun test_second() {
                expect(2).toEqual(2);
            }

            get fun test_second_fail() {
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
        .assert_snapshot_matches("integration/snapshots/test_dot_multiple_files.stdout.txt");
}
