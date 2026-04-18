use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn prompts_return_deterministic_fallbacks_in_non_interactive_runner() {
    ProjectBuilder::new("u-stdlib-prompts-deterministic-fallbacks")
        .test_file(
            "prompt_wrappers",
            r#"
            import "../../lib/prompts"
            import "../../lib/testing/expect"

            get fun `test prompt select confirm fallbacks`() {
                expect(prompt("Enter your name:", "Guest")).toEqual("");
                expect(select("Choose network:", ["Mainnet", "Testnet", "Local"])).toEqual("");
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
            "integration/snapshots/test-runner/prompts_return_deterministic_fallbacks_in_non_interactive_runner/prompts_return_deterministic_fallbacks_in_non_interactive_runner.stdout.txt",
        );
}

#[test]
fn confirm_default_true_is_ignored_in_non_interactive_mode_bug() {
    ProjectBuilder::new("u-stdlib-confirm-default-true-bug")
        .test_file(
            "prompt_wrappers_bug",
            r#"
            import "../../lib/prompts"
            import "../../lib/testing/expect"

            get fun `test confirm default true in non interactive mode`() {
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
            "integration/snapshots/test-runner/prompts_return_deterministic_fallbacks_in_non_interactive_runner/confirm_default_true_is_ignored_in_non_interactive_mode_bug.stdout.txt",
        );
}
