use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const PROMPTS_IMPORTS: &str = r#"
import "../../lib/promts/prompts"
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
fn select_multiple_options_return_empty_string_in_non_interactive_mode() {
    run_select_success(
        "ap-stdlib-select-multiple-options-fallback",
        r#"
get fun `test ap stdlib select multiple options fallback`() {
    val selected = select("Choose network:", ["Mainnet", "Testnet", "Local"] as tuple);
    expect(selected).toEqual("");
}
"#,
        "integration/snapshots/test-runner/select_multiple_options_return_empty_string_in_non_interactive_mode/select_multiple_options_return_empty_string_in_non_interactive_mode.stdout.txt",
    );
}

#[test]
fn select_does_not_honor_starting_cursor_index_zero_in_non_interactive_mode_bug() {
    run_select_success(
        "ap-stdlib-select-starting-cursor-index-zero-bug",
        r#"
get fun `test ap stdlib select starting cursor index zero bug`() {
    val selected = select("Choose deployment profile:", ["Safe", "Fast", "Experimental"] as tuple);
    expect(selected).toEqual("");
}
"#,
        "integration/snapshots/test-runner/select_multiple_options_return_empty_string_in_non_interactive_mode/select_does_not_honor_starting_cursor_index_zero_in_non_interactive_mode_bug.stdout.txt",
    );
}
