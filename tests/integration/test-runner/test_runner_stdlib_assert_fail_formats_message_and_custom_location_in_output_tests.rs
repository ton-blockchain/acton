use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
import "../../lib/testing/expect"
"#;

fn wrap_assert_source(test_body: &str) -> String {
    format!("{ASSERT_IMPORTS}\n{test_body}\n")
}

#[test]
fn assert_fail_formats_message_and_custom_location_in_output() {
    let source = wrap_assert_source(
        r#"
get fun `test-bv-assert-fail-custom-location`() {
    Assert.fail(
        "bv explicit fail message",
        "tests/custom_assert_fail_output.test.tolk:42:7"
    );
}
"#,
    );

    ProjectBuilder::new("bv-stdlib-assert-fail-custom-location")
        .test_file("assert_fail_formatting", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Error: bv explicit fail message")
        .assert_contains("at tests/custom_assert_fail_output.test.tolk:42:7")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_assert_fail_formats_message_and_custom_location_in_output_tests/assert_fail_formats_message_and_custom_location_in_output.stdout.txt",
        );
}

#[test]
fn assert_fail_honors_expected_exit_code_path_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bv_assert_fail_expected_exit_path.test.tolk";
    let source = wrap_assert_source(
        r#"
get fun `test-bv-assert-fail-expected-exit`() {
    expectToEndWithExitCode(567);
    Assert.fail(
        "bv expected exit path message",
        "tests/bv_assert_fail_expected_exit_path.test.tolk:9:5"
    );
}
"#,
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write BV fixture assert.fail expected-exit test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_not_contains("Error: bv expected exit path message")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_assert_fail_formats_message_and_custom_location_in_output_tests/assert_fail_honors_expected_exit_code_path_in_fixture_project.stdout.txt",
        );
}
