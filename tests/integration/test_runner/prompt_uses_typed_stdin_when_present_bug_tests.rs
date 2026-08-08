use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn prompt_ignores_typed_stdin_and_uses_default_in_non_interactive_mode() {
    let project = ProjectBuilder::new("ao-stdlib-prompt-non-interactive-default")
        .test_file(
            "prompt_stdin_value",
            r#"
            import "../../lib/prompts"
            import "../../lib/testing/expect"

            get fun `test ao prompt typed stdin`() {
                val name = prompt("Enter your name:", "type your name", "Bob");
                expect(name).toEqual("Bob");
            }
        "#,
        )
        .build();

    let mut command = project.acton().test().filter("ao prompt typed stdin");
    command.cmd = command.cmd.stdin("Alice\n");

    command
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/prompt_uses_typed_stdin_when_present_bug/prompt_ignores_typed_stdin_and_uses_default_in_non_interactive_mode.stdout.txt",
        );
}
