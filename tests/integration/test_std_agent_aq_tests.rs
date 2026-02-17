//! Reserved integration test module for subagent AQ.
//!
//! Ownership boundary for agent AQ:
//! - tests/integration/test_std_agent_aq_tests.rs
//! - tests/integration/snapshots/test_std_agent_aq/**
//! - tests/integration/testdata/test_std_agent_aq/**
//! - tests/support/test_std_agent_aq/** (optional)
//!
//! Required test name prefix:
//! - aq_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn aq_stdlib_confirm_default_true_with_help_message_ignores_empty_input_bug() {
    let mut command = ProjectBuilder::new("aq-stdlib-confirm-empty-input-default-bug")
        .test_file(
            "confirm_empty_input_bug",
            r#"
            import "../../lib/promts/prompts"
            import "../../lib/testing/expect"

            get fun `test-confirm-empty-input-defaults`() {
                expect(confirm("Abort deployment?", false, "Press enter to keep false.")).toEqual(false);

                val accepted = confirm("Proceed with deployment?", true, "Press enter to accept default.");
                // BUG: confirm() ignores default=true when input is empty; expected true, got false.
                expect(accepted).toEqual(true);
            }
        "#,
        )
        .build()
        .acton()
        .test();

    command.cmd = command.cmd.stdin("\n\n");

    command
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_aq/aq_stdlib_confirm_default_true_with_help_message_ignores_empty_input_bug.stdout.txt",
        );
}
