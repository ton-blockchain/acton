use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn confirm_default_true_with_help_message_uses_default_in_non_interactive_mode() {
    let project = ProjectBuilder::new("aq-stdlib-confirm-empty-input-default")
        .test_file(
            "confirm_empty_input_default",
            r#"
            import "../../lib/prompts"
            import "../../lib/testing/expect"

            get fun `test confirm empty input defaults`() {
                expect(confirm("Abort deployment?", false, "Press enter to keep false.")).toEqual(false);

                val accepted = confirm("Proceed with deployment?", true, "Press enter to accept default.");
                expect(accepted).toEqual(true);
            }
        "#,
        ).build();

    let mut command = project.acton().test();
    command.cmd = command.cmd.stdin("\n\n");

    command
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/confirm_default_true_with_help_message_ignores_empty_input_bug/confirm_default_true_with_help_message_uses_default_in_non_interactive_mode.stdout.txt",
        );
}
