use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn run_expect_array_suite(
    project_name: &str,
    test_body: &str,
) -> crate::support::assertions::TestOutput {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("expect_array", &source)
        .build()
        .acton()
        .test()
        .run()
}

#[test]
fn expect_array_matchers_accept_empty_and_non_empty_arrays() {
    run_expect_array_suite(
        "stdlib-expect-array-success",
        r"
get fun `test stdlib array matchers non empty`() {
    val values = [1, 2, 3];

    expect(values).toContain(2);
    expect(values).toNotContain(99);
    expect(values).toBeNonEmpty();
    expect(values).toHaveLength(3);
}

get fun `test stdlib array matchers empty`() {
    val values = array<int> [];

    expect(values).toBeEmpty();
    expect(values).toNotContain(1);
    expect(values).toHaveLength(0);
}
",
    )
    .success()
    .assert_passed(2)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/expect_array_matchers_cover_membership_and_length/expect_array_matchers_accept_empty_and_non_empty_arrays.stdout.txt",
    );
}

#[test]
fn expect_array_matchers_report_membership_and_length_mismatches() {
    run_expect_array_suite(
        "stdlib-expect-array-failures",
        r"
get fun `test stdlib array to contain missing`() {
    val values = [10, 20];
    expect(values).toContain(30);
}

get fun `test stdlib array to not contain present`() {
    val values = [10, 20];
    expect(values).toNotContain(10);
}

get fun `test stdlib array to be empty on non empty`() {
    val values = [1];
    expect(values).toBeEmpty();
}

get fun `test stdlib array to be non empty on empty`() {
    val values = array<int> [];
    expect(values).toBeNonEmpty();
}

get fun `test stdlib array to have length mismatch`() {
    val values = [1, 2, 3];
    expect(values).toHaveLength(4);
}
",
    )
    .failure()
    .assert_failed(5)
    .assert_snapshot_matches(
        "integration/snapshots/test-runner/expect_array_matchers_cover_membership_and_length/expect_array_matchers_report_membership_and_length_mismatches.stdout.txt",
    );
}
