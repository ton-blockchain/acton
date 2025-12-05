use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;

#[test]
fn test_basic_fixture_passing() {
    FixtureProject::load("basic")
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches("integration/snapshots/test_basic_fixture_passing.stdout.txt");
}

#[test]
fn test_basic_fixture_with_failing_contract() {
    FixtureProject::load("basic")
        .with_contract_slot(1) // Enable "throw 10;" in contract
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(2)
        .assert_contains("exit_code=10")
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_failing_contract.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_failing_contract_and_backtrace_full() {
    FixtureProject::load("basic")
        .with_contract_slot(1) // Enable "throw 10;" in contract
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(2)
        .assert_contains("exit_code=10")
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_failing_contract_and_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_gas_limit() {
    FixtureProject::load("basic")
        .with_test_slot(1) // Enable gas_limit annotation
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Gas limit exceeded")
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_gas_limit.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_expect_failure() {
    FixtureProject::load("basic")
        .with_test_slot(2) // Enable expect failure
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_expect_failure.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_exit_code_mismatch() {
    FixtureProject::load("basic")
        .with_test_slot(3) // Enable exit code expectation
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Expected exit_code=")
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_exit_code_mismatch.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_throw_in_test() {
    FixtureProject::load("basic")
        .with_test_slot(4) // Enable throw in test
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=9")
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_throw_in_test.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_throw_in_test_and_backtrace_full() {
    FixtureProject::load("basic")
        .with_test_slot(4) // Enable throw in test
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=9")
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_throw_in_test_and_backtrace_full.stdout.txt",
        );
}

#[test]
fn test_basic_fixture_with_debug_output() {
    FixtureProject::load("basic")
        .with_contract_slot(2) // Enable debug.printString
        .with_test_slot(5) // Enable println in test
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(2).assert_snapshot_matches("integration/snapshots/test_basic_fixture_with_debug_output.stdout.txt")
        // .assert_contains("Hello World") // TODO
    ;
}

#[test]
fn test_basic_fixture_with_stderr_output() {
    FixtureProject::load("basic")
        .with_test_slot(6) // Enable eprintln in test
        .acton()
        .test()
        .with_backtrace("full")
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test_basic_fixture_with_stderr_output.stdout.txt",
        );
}

#[test]
fn test_compilation_error_fixture() {
    FixtureProject::load("with_compilation_error")
        .with_contract_slot(1)
        .acton()
        .test()
        .run()
        .failure()
        .assert_contains("field `body2` doesn't exist in type `InMessage`")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compilation_error_fixture.stderr.txt",
        );
}
