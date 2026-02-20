use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FMT_TEST_IMPORTS: &str = r#"
import "../../lib/fmt"
import "../../lib/testing/expect"
"#;

fn wrap_fmt_test_source(test_body: &str) -> String {
    format!("{FMT_TEST_IMPORTS}\n{test_body}\n")
}

fn run_fmt_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = wrap_fmt_test_source(test_body);
    ProjectBuilder::new(project_name)
        .test_file("fmt_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn format3_mixed_placeholders_should_follow_template_order_bug() {
    run_fmt_success(
        "ea-stdlib-format3-mixed-placeholder-order-bug",
        r#"
get fun `test-ea-stdlib-format3-mixed-placeholder-order-bug`() {
    val rendered = format3("{} | {:x} | {:ton}", 255, 16, 1500000000);
    expect(rendered).toEqual("255 | 10 | 1.5 TON");
}
"#,
        "integration/snapshots/test-runner/format3_mixed_placeholders_should_follow_template_order_bug/format3_mixed_placeholders_should_follow_template_order_bug.stdout.txt",
    );
}

#[test]
fn format3_escaped_braces_around_placeholder_should_collapse_bug() {
    run_fmt_success(
        "ea-stdlib-format3-escaped-braces-bug",
        r#"
get fun `test-ea-stdlib-format3-escaped-braces-bug`() {
    val rendered = format3("wrap={{{}}} hex={:x} ton={:ton}", "inner", 255, 2500000000);
    expect(rendered).toEqual("wrap={inner} hex=ff ton=2.5 TON");
}
"#,
        "integration/snapshots/test-runner/format3_mixed_placeholders_should_follow_template_order_bug/format3_escaped_braces_around_placeholder_should_collapse_bug.stdout.txt",
    );
}
