use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const MAYBE_IMPORTS: &str = r#"
import "../../lib/tlb/maybe"
import "../../lib/testing/expect"
"#;

fn run_maybe_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_source = format!("{MAYBE_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("maybe_behavior", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_maybe_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_source = format!("{MAYBE_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("maybe_behavior", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=7")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn maybe_some_none_unwrap_and_unwrap_or_follow_documented_semantics() {
    run_maybe_success(
        "aa-stdlib-maybe-some-none-unwrap-semantics",
        r#"
get fun `test-maybe-some-none-unwrap-semantics`() {
    val empty = Maybe<int>.none();
    val present = Maybe<int>.some(17);

    expect(empty).toBeNone();
    expect(present).toBeDefined();

    expect(empty.unwrapOr(99)).toEqual(99);
    expect(present.unwrapOr(99)).toEqual(17);
    expect(present.unwrap()).toEqual(17);

    val nested = Maybe<Maybe<int>>.some(Maybe<int>.none());
    val nestedValue = nested.unwrap();
    expect(nestedValue).toBeNone();
    expect(nestedValue.unwrapOr(123)).toEqual(123);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_maybe_some_none_unwrap_and_unwrap_or_follow_documented_semantics_tests/maybe_some_none_unwrap_and_unwrap_or_follow_documented_semantics.stdout.txt",
    );
}

#[test]
fn maybe_unwrap_on_none_throws_exit_code_7() {
    run_maybe_failure(
        "aa-stdlib-maybe-unwrap-none-exit7",
        r#"
get fun `test-maybe-unwrap-on-none-throws-exit-code-7`() {
    val empty = Maybe<int>.none();
    empty.unwrap();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_maybe_some_none_unwrap_and_unwrap_or_follow_documented_semantics_tests/maybe_unwrap_on_none_throws_exit_code_7.stdout.txt",
    );
}
