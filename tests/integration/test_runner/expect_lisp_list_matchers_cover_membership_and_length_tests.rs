use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn run_expect_lisp_list_suite(
    project_name: &str,
    test_body: &str,
) -> crate::support::assertions::TestOutput {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("expect_lisp_list", &source)
        .build()
        .acton()
        .test()
        .run()
}

#[test]
fn expect_lisp_list_matchers_report_membership_and_length_mismatches() {
    run_expect_lisp_list_suite(
        "stdlib-expect-lisp-list-failures",
        r"
fun filledList(): lisp_list<int> {
    return [1, 2, 3];
}

get fun `test stdlib lisp list to contain missing`() {
    expect(filledList()).toContain(99);
}

get fun `test stdlib lisp list to not contain present`() {
    expect(filledList()).toNotContain(2);
}

get fun `test stdlib lisp list to be empty on non empty`() {
    expect(filledList()).toBeEmpty();
}

get fun `test stdlib lisp list to be non empty on empty`() {
    val values = lisp_list<int> [];
    expect(values).toBeNonEmpty();
}

get fun `test stdlib lisp list to have length mismatch`() {
    expect(filledList()).toHaveLength(4);
}
",
    )
    .failure()
    .assert_failed(5)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/expect_lisp_list_matchers_cover_membership_and_length/expect_lisp_list_matchers_report_membership_and_length_mismatches.stdout.txt",
    );
}
