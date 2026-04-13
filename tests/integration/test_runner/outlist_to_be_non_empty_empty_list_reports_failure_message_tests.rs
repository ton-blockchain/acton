use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const OUTLIST_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/testing/outlist_expect"
"#;

fn run_outlist_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{OUTLIST_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("outlist_non_empty", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("expect(actual).toBeNonEmpty(expected): array is empty")
        .assert_contains("0")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn outlist_to_be_non_empty_empty_list_reports_failure_message() {
    run_outlist_failure(
        "cd-stdlib-outlist-to-be-non-empty-empty-list",
        r"
get fun `test cd stdlib outlist to be non empty empty list`() {
    val out_actions = [];
    expect(out_actions).toBeNonEmpty();
}
",
        "integration/snapshots/test-runner/outlist_to_be_non_empty_empty_list_reports_failure_message/outlist_to_be_non_empty_empty_list_reports_failure_message.stdout.txt",
    );
}
