//! Reserved integration test module for subagent M.
//!
//! Ownership boundary for agent M:
//! - tests/integration/test_lib_agent_m_tests.rs
//! - tests/integration/snapshots/test_lib_agent_m/**
//! - tests/integration/testdata/test_lib_agent_m/**
//! - tests/support/test_lib_agent_m/** (optional)
//!
//! Required test name prefix:
//! - m_lib_api_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn m_lib_api_expect_to_end_with_exit_code_marks_controlled_throw_as_pass() {
    ProjectBuilder::new("m-lib-api-expected-throw-pass")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "expect_exit_code",
            r#"
            import "../../lib/testing/expect"

            get fun `test-expect-exit-code-pass`() {
                expectToEndWithExitCode(77);
                throw 77;
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_not_contains("Expected exit_code=")
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_m/m_lib_api_expect_to_end_with_exit_code_marks_controlled_throw_as_pass.stdout.txt",
        );
}

#[test]
fn m_lib_api_expect_to_end_with_exit_code_reports_mismatched_throw() {
    ProjectBuilder::new("m-lib-api-exit-code-mismatch")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "expect_exit_code",
            r#"
            import "../../lib/testing/expect"

            get fun `test-expect-exit-code-mismatch`() {
                expectToEndWithExitCode(42);
                throw 99;
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Expected exit_code=42, got=99")
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_m/m_lib_api_expect_to_end_with_exit_code_reports_mismatched_throw.stdout.txt",
        );
}

#[test]
fn m_lib_api_expect_to_end_with_exit_code_overrides_fail_with_annotation() {
    ProjectBuilder::new("m-lib-api-override-annotation")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "expect_exit_code",
            r#"
            import "../../lib/testing/expect"

            @test({ fail_with: 12 })
            get fun `test-exit-code-dynamic-overrides-annotation`() {
                expectToEndWithExitCode(24);
                throw 24;
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_not_contains("Expected exit_code=12")
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_m/m_lib_api_expect_to_end_with_exit_code_overrides_fail_with_annotation.stdout.txt",
        );
}

#[test]
fn m_lib_api_expect_to_end_with_exit_code_last_call_wins() {
    ProjectBuilder::new("m-lib-api-last-exit-code-wins")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "expect_exit_code",
            r#"
            import "../../lib/testing/expect"

            get fun `test-exit-code-last-call-wins`() {
                expectToEndWithExitCode(5);
                expectToEndWithExitCode(9);
                throw 9;
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_not_contains("Expected exit_code=5")
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_m/m_lib_api_expect_to_end_with_exit_code_last_call_wins.stdout.txt",
        );
}

#[test]
fn m_lib_api_expect_to_end_with_exit_code_conditional_path_without_expect_fails_with_raw_exit_code() {
    ProjectBuilder::new("m-lib-api-conditional-expect")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "expect_exit_code",
            r#"
            import "../../lib/testing/expect"

            get fun `test-exit-code-conditional-expect`() {
                val shouldExpect = false;
                if (shouldExpect) {
                    expectToEndWithExitCode(17);
                }
                throw 17;
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=17")
        .assert_not_contains("Expected exit_code=")
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_m/m_lib_api_expect_to_end_with_exit_code_conditional_path_without_expect_fails_with_raw_exit_code.stdout.txt",
        );
}
