//! Reserved integration test module for subagent AO.
//!
//! Ownership boundary for agent AO:
//! - tests/integration/test-runner/test_runner_stdlib_ao_prompt_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_ao_prompt_tests/**
//! - tests/integration/testdata/test_std_agent_ao/**
//! - tests/support/test_std_agent_ao/** (optional)
//!
//! Required test name prefix:
//! - ao_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn prompt_uses_typed_stdin_when_present_bug() {
    let project = ProjectBuilder::new("ao-stdlib-prompt-typed-stdin")
        .test_file(
            "prompt_stdin_value",
            r#"
            import "../../lib/promts/prompts"
            import "../../lib/testing/expect"

            get fun `test-ao-prompt-typed-stdin`() {
                val name = prompt("Enter your name:", "Guest");
                expect(name).toEqual("");
            }
        "#,
        )
        .build();

    let mut command = project.acton().test().filter("ao-prompt-typed-stdin");
    command.cmd = command.cmd.stdin("Alice\n");

    command
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_ao_prompt_tests/ao_stdlib_prompt_uses_typed_stdin_when_present_bug.stdout.txt",
        );
}
