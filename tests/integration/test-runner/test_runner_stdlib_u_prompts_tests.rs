//! Reserved integration test module for subagent U.
//!
//! Ownership boundary for agent U:
//! - tests/integration/test-runner/test_runner_stdlib_u_prompts_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_u_prompts_tests/**
//! - tests/integration/testdata/test_std_agent_u/**
//! - tests/support/test_std_agent_u/** (optional)
//!
//! Required test name prefix:
//! - u_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn prompts_return_deterministic_fallbacks_in_non_interactive_runner() {
    ProjectBuilder::new("u-stdlib-prompts-deterministic-fallbacks")
        .test_file(
            "prompt_wrappers",
            r#"
            import "../../lib/promts/prompts"
            import "../../lib/testing/expect"

            get fun `test-prompt-select-confirm-fallbacks`() {
                expect(prompt("Enter your name:", "Guest")).toEqual("");
                expect(select("Choose network:", ["Mainnet", "Testnet", "Local"] as tuple)).toEqual("");
                expect(confirm("Proceed with deployment?", false, "Safe default should be false.")).toEqual(false);
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
            "integration/snapshots/test-runner/test_runner_stdlib_u_prompts_tests/u_stdlib_prompts_return_deterministic_fallbacks_in_non_interactive_runner.stdout.txt",
        );
}

#[test]
fn confirm_default_true_is_ignored_in_non_interactive_mode_bug() {
    ProjectBuilder::new("u-stdlib-confirm-default-true-bug")
        .test_file(
            "prompt_wrappers_bug",
            r#"
            import "../../lib/promts/prompts"
            import "../../lib/testing/expect"

            get fun `test-confirm-default-true-in-non-interactive-mode`() {
                val answer = confirm("Proceed with deployment?", true, "Press enter to accept default.");
                expect(answer).toEqual(false);
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
            "integration/snapshots/test-runner/test_runner_stdlib_u_prompts_tests/u_stdlib_confirm_default_true_is_ignored_in_non_interactive_mode_bug.stdout.txt",
        );
}
