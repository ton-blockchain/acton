use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn discovery_supports_all_name_prefixes() {
    ProjectBuilder::new("a-discovery-prefixes")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "naming",
            r#"
            import "../../lib/testing/expect"

            get fun `test-dash-case`() {
                expect(1).toEqual(1);
            }

            get fun test_underscore_case() {
                expect(2).toEqual(2);
            }

            get fun `test space case`() {
                expect(3).toEqual(3);
            }

            get fun helper_not_discovered() {
                expect(1).toEqual(2); // Must not be discovered by test runner
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(3)
        .assert_contains("dash-case")
        .assert_contains("underscore_case")
        .assert_contains("space case")
        .assert_not_contains("helper_not_discovered");
}

#[test]
fn exclude_has_priority_over_include() {
    let project = ProjectBuilder::new("a-include-exclude-priority")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/unit/only-unit.test.tolk",
            r#"
            import "../../../lib/testing/expect"

            get fun `test unit selected`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .raw_file(
            "tests/integration/should-be-excluded.test.tolk",
            r#"
            import "../../../lib/testing/expect"

            get fun `test integration excluded`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .include_pattern("tests/**/*.test.tolk")
        .exclude_pattern("tests/integration/**")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("unit selected")
        .assert_not_contains("integration excluded");
}

#[test]
fn cli_include_overrides_config_include() {
    let project = ProjectBuilder::new("a-cli-include-overrides-config")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/unit/unit-only.test.tolk",
            r#"
            import "../../../lib/testing/expect"

            get fun `test from cli include`() {
                expect(10).toEqual(10);
            }
        "#,
        )
        .raw_file(
            "tests/integration/integration-only.test.tolk",
            r#"
            import "../../../lib/testing/expect"

            get fun `test from config include`() {
                expect(20).toEqual(20);
            }
        "#,
        )
        .with_test_config(TestConfig {
            include_patterns: Some(vec!["tests/integration/**".to_string()]),
            ..Default::default()
        })
        .build();

    project
        .acton()
        .test()
        .include_pattern("tests/unit/**")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("from cli include")
        .assert_not_contains("from config include");
}

#[test]
fn cli_exclude_overrides_config_exclude() {
    let project = ProjectBuilder::new("a-cli-exclude-overrides-config")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/unit/unit-only.test.tolk",
            r#"
            import "../../../lib/testing/expect"

            get fun `test from cli exclude`() {
                expect(100).toEqual(100);
            }
        "#,
        )
        .raw_file(
            "tests/integration/integration-only.test.tolk",
            r#"
            import "../../../lib/testing/expect"

            get fun `test from config exclude`() {
                expect(200).toEqual(200);
            }
        "#,
        )
        .with_test_config(TestConfig {
            exclude_patterns: Some(vec!["tests/unit/**".to_string()]),
            ..Default::default()
        })
        .build();

    project
        .acton()
        .test()
        .exclude_pattern("tests/integration/**")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("from cli exclude")
        .assert_not_contains("from config exclude");
}

#[test]
fn fail_fast_stops_after_first_failure_with_todo_and_skip() {
    ProjectBuilder::new("a-fail-fast-with-todo-skip")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "flow",
            r#"
            import "../../lib/testing/expect"

            @test("todo")
            get fun `test todo before failure`() {
                expect(1).toEqual(2); // Must not run
            }

            @test("skip")
            get fun `test skip before failure`() {
                expect(1).toEqual(2); // Must not run
            }

            get fun `test first failure`() {
                expect(1).toEqual(2);
            }

            get fun `test should not run`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .fail_fast()
        .run()
        .failure()
        .assert_failed(1)
        .assert_skipped(1)
        .assert_todo(1)
        .assert_contains("first failure")
        .assert_not_contains("should not run");
}
