use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const PROMPTS_IMPORTS: &str = r#"
import "../../lib/prompts"
import "../../lib/testing/expect"
"#;

fn run_select_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{PROMPTS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("prompt_select", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn select_multiple_options_returns_default_option_in_non_interactive_mode() {
    run_select_success(
        "ap-stdlib-select-multiple-options-default",
        r#"
get fun `test ap stdlib select multiple options fallback`() {
    val selected = select("Choose network:", ["Mainnet", "Testnet", "Local"]);
    expect(selected).toEqual("Mainnet");
}
"#,
        "integration/snapshots/test-runner/select_multiple_options_return_empty_string_in_non_interactive_mode/select_multiple_options_returns_default_option_in_non_interactive_mode.stdout.txt",
    );
}

#[test]
fn select_honors_starting_cursor_index_zero_in_non_interactive_mode() {
    run_select_success(
        "ap-stdlib-select-starting-cursor-index-zero",
        r#"
get fun `test ap stdlib select starting cursor index zero bug`() {
    val selected = select("Choose deployment profile:", ["Safe", "Fast", "Experimental"]);
    expect(selected).toEqual("Safe");
}
"#,
        "integration/snapshots/test-runner/select_multiple_options_return_empty_string_in_non_interactive_mode/select_honors_starting_cursor_index_zero_in_non_interactive_mode.stdout.txt",
    );
}
