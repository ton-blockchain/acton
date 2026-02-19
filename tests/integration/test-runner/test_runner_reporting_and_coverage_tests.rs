use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn cli_reporter_overrides_config_reporter() {
    let project = ProjectBuilder::new("b-reporter-override")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-alpha`() {
                expect(1).toEqual(1);
            }

            get fun `test-beta`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .with_test_config(TestConfig {
            reporters: Some(vec!["dot".to_owned()]),
            ..Default::default()
        })
        .build();

    project
        .acton()
        .test()
        .with_reporter("junit")
        .run()
        .success()
        .assert_file_exists("test-results/TEST-test.test.tolk.xml")
        .assert_file_contains(
            "test-results/TEST-test.test.tolk.xml",
            r#"<testcase name="test-alpha""#,
        )
        .assert_not_contains("··");
}

#[test]
fn teamcity_escapes_special_characters_in_test_names() {
    ProjectBuilder::new("b-teamcity-escaping")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-teamcity [special]|quote'`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .with_reporter("console")
        .with_reporter("teamcity")
        .run()
        .success()
        .assert_contains("##teamcity[testStarted")
        .assert_contains("test-teamcity |[special|]||quote|'")
        .assert_contains("##teamcity[testFinished");
}

#[test]
fn junit_merge_creates_only_single_output_file() {
    let project = ProjectBuilder::new("b-junit-merge-only")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "first",
            r#"
            import "../../lib/testing/expect"

            get fun `test-first-suite`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "second",
            r#"
            import "../../lib/testing/expect"

            get fun `test-second-suite`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_reporter("junit")
        .with_junit_merge()
        .run()
        .success()
        .assert_file_exists("test-results/junit-results.xml")
        .assert_file_contains("test-results/junit-results.xml", "<testsuite");

    let first_suite_file = project.path().join("test-results/TEST-first.test.tolk.xml");
    assert!(
        !first_suite_file.exists(),
        "Per-suite file should not exist in junit merge mode: {}",
        first_suite_file.display()
    );

    let second_suite_file = project
        .path()
        .join("test-results/TEST-second.test.tolk.xml");
    assert!(
        !second_suite_file.exists(),
        "Per-suite file should not exist in junit merge mode: {}",
        second_suite_file.display()
    );
}

#[test]
fn coverage_and_junit_reporter_work_together() {
    let project = ProjectBuilder::new("b-coverage-with-junit")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/math",
            r"
            fun mul(a: int, b: int): int {
                return a * b;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/math"

            get fun `test-mul`() {
                expect(mul(3, 7)).toEqual(21);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_reporter("junit")
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("cov-output.txt")
        .run()
        .success()
        .assert_contains("Text coverage file saved in cov-output.txt")
        .assert_file_exists("cov-output.txt")
        .assert_file_contains("cov-output.txt", "Lines:")
        .assert_file_exists("test-results/TEST-test.test.tolk.xml")
        .assert_file_contains("test-results/TEST-test.test.tolk.xml", "<testsuite");
}

#[test]
fn cli_coverage_format_overrides_config_coverage_format() {
    let project = ProjectBuilder::new("b-coverage-config-override")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r"
            fun id(v: int): int {
                return v;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"

            get fun `test-id`() {
                expect(id(5)).toEqual(5);
            }
        "#,
        )
        .with_test_config(TestConfig {
            coverage: Some(true),
            coverage_format: Some("lcov".to_owned()),
            coverage_file: Some("from-config.lcov".to_owned()),
            ..Default::default()
        })
        .build();

    project
        .acton()
        .test()
        .with_coverage_format("text")
        .with_coverage_file("from-cli.txt")
        .run()
        .success()
        .assert_contains("Text coverage file saved in from-cli.txt")
        .assert_file_exists("from-cli.txt")
        .assert_file_contains("from-cli.txt", "Lines:");

    let config_output = project.path().join("from-config.lcov");
    assert!(
        !config_output.exists(),
        "Config coverage output should be overridden by CLI: {}",
        config_output.display()
    );
}
