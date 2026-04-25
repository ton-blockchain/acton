use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn prompts_return_deterministic_fallbacks_in_non_interactive_runner() {
    ProjectBuilder::new("u-stdlib-prompts-deterministic-fallbacks")
        .test_file(
            "prompt_wrappers",
            r#"
            import "../../lib/prompts"
            import "../../lib/fmt"
            import "../../lib/testing/expect"

            get fun `test prompt select confirm fallbacks`() {
                expect(prompt("Enter your name:", "Guest")).toEqual("");
                expect(prompt("Enter your name:", "type your name", "John")).toEqual("John");
                expect(select("Choose network:", ["Mainnet", "Testnet", "Local"])).toEqual("");
                expect(confirm("Proceed with deployment?", false, "Safe default should be false.")).toEqual(false);
                expect(promptInt("Enter retry count:", "42")).toEqual(42);

                val defaultAddress = parseAddress("0:0000000000000000000000000000000000000000000000000000000000000000");
                expect(promptAddress("Enter recipient:", defaultAddress)).toEqual(defaultAddress);
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

#[cfg(unix)]
#[test]
fn prompt_int_and_address_validate_interactive_input_until_corrected() {
    use expectrl::Eof;
    use std::time::Duration;

    let project = ProjectBuilder::new("stdlib-prompts-typed-validation")
        .script_file(
            "typed_prompts",
            r#"
            import "../../lib/prompts"
            import "../../lib/io"

            fun main() {
                val count = promptInt("Enter retry count", "1");
                println("count={}", count);

                val recipient = promptAddress("Enter recipient");
                println("recipient={}", recipient);
            }
        "#,
        )
        .build();

    let mut session = project
        .acton()
        .script("scripts/typed_prompts.tolk")
        .spawn_pty()
        .set_expect_timeout(Some(Duration::from_secs(10)));

    session.expect("Enter retry count");
    session.send_line("abc", "failed to send invalid integer");
    session.expect("Enter a valid integer");
    session.send_line("\u{15}7", "failed to correct integer");
    session.expect("count=7");

    session.expect("Enter recipient");
    session.send_line("not-an-address", "failed to send invalid address");
    session.expect("Enter a valid TON address");
    session.send_line(
        "\u{15}0:0000000000000000000000000000000000000000000000000000000000000000",
        "failed to correct address",
    );
    session.expect("recipient=");
    session.expect(Eof);
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
