use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};
use crate::support::snapshots::normalize_output;
use std::fs;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn test_coverage_basic_output() {
    let project = ProjectBuilder::new("coverage-basic")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/math",
            r#"
            fun add(a: int, b: int): int {
                return a + b;
            }
            
            fun isPositive(x: int): bool {
                if (x > 0) {
                    return true;
                } else {
                    return false;
                }
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/math"
            
            get fun `test-coverage-example`() {
                val result = add(1, 2);
                expect(result).toEqual(3);
                
                val positive = isPositive(5);
                expect(positive).toEqual(true);

                val positive2 = isPositive(-10);
                expect(positive2).toEqual(false);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("math.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_basic_output.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_basic_output.txt",
        );
}

#[test]
fn test_coverage_multiple_tests() {
    let project = ProjectBuilder::new("coverage-multiple")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/calculator",
            r#"
            fun multiply(a: int, b: int): int {
                return a * b;
            }
            
            fun divide(a: int, b: int): int {
                if (b == 0) {
                    throw 100;
                }
                return a / b;
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/calculator"
            
            get fun `test-multiply`() {
                val result = multiply(3, 4);
                expect(result).toEqual(12);
            }
            
            get fun `test-divide`() {
                val result = divide(10, 2);
                expect(result).toEqual(5);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains(" COVERAGE ")
        .assert_contains("calculator.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_multiple_tests.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_multiple_tests.txt",
        );
}

#[test]
fn test_coverage_with_failing_tests() {
    let project = ProjectBuilder::new("coverage-with-failures")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/validator",
            r#"
            fun validate(value: int): bool {
                if (value > 0) {
                    return true;
                }
                return false;
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/validator"
            
            get fun `test-passing`() {
                val result = validate(10);
                expect(result).toEqual(true);
            }
            
            get fun `test-failing`() {
                val result = validate(10);
                expect(result).toEqual(false); // This will fail
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .failure()
        .assert_passed(1)
        .assert_failed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("validator.tolk")
        .assert_snapshot_matches(
            "integration/snapshots/test_coverage_with_failing_tests.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_with_failing_tests.txt",
        );
}

#[test]
fn test_coverage_with_filter() {
    let project = ProjectBuilder::new("coverage-filtered")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/helpers",
            r#"
            fun double(x: int): int {
                return x * 2;
            }
            
            fun triple(x: int): int {
                return x * 3;
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/helpers"
            
            get fun `test-unit-double`() {
                val result = double(5);
                expect(result).toEqual(10);
            }
            
            get fun `test-integration-triple`() {
                val result = triple(5);
                expect(result).toEqual(15);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains(" COVERAGE ")
        .assert_contains("helpers.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_with_filter_all.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_with_filter_all.txt",
        );

    project
        .acton()
        .test()
        .filter("test-unit-.*")
        .with_coverage()
        .with_coverage_format("text")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("helpers.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_with_filter.stdout.txt")
        .assert_file_snapshot_matches(
            "coverage.txt",
            "integration/snapshots/test_coverage_with_filter.txt",
        );
}

#[test]
fn test_coverage_lcov_snapshot() {
    let project = ProjectBuilder::new("coverage-lcov-snapshot")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r#"
            fun and(a: bool, b: bool): bool {
                return a && b;
            }
            
            fun or(a: bool, b: bool): bool {
                return a || b;
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"
            
            get fun `test-lcov-snapshot`() {
                val result1 = and(true, true);
                expect(result1).toEqual(true);
                
                val result2 = or(false, true);
                expect(result2).toEqual(true);
            }
        "#,
        )
        .build();

    let lcov_path = project.path().join("lcov.info");

    let output = project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("lcov")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("LCOV file saved in lcov.info");

    let lcov_content = fs::read_to_string(&lcov_path).expect("Should read lcov.info");
    assertion().eq(
        normalize_output(lcov_content.as_str(), project.path()),
        snapbox::file!("snapshots/test_coverage_lcov_snapshot.lcov"),
    )
}

#[test]
fn test_coverage_empty_no_tests() {
    let project = ProjectBuilder::new("coverage-empty")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            
            // No test functions
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .with_coverage()
        .run()
        .success()
        .assert_passed(0);
}

#[test]
fn test_coverage_text_custom_filename() {
    let project = ProjectBuilder::new("coverage-text-custom")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r#"
            fun and(a: bool, b: bool): bool {
                return a && b;
            }

            fun or(a: bool, b: bool): bool {
                return a || b;
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"

            get fun `test-custom-filename`() {
                val result1 = and(true, true);
                expect(result1).toEqual(true);

                val result2 = or(false, true);
                expect(result2).toEqual(true);
            }
        "#,
        )
        .build();

    let output = project
        .acton()
        .test()
        .with_coverage()
        .with_coverage_format("text")
        .with_coverage_file("my-custom-coverage.txt")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("Text coverage file saved in my-custom-coverage.txt")
        .assert_file_exists("my-custom-coverage.txt")
        .assert_file_snapshot_matches(
            "my-custom-coverage.txt",
            "integration/snapshots/test_coverage_text_custom_filename.txt",
        );

    let default_path = project.path().join("coverage.txt");
    assert!(
        !default_path.exists(),
        "Default coverage.txt should not exist when custom filename is specified"
    );
}

#[test]
fn test_coverage_text_custom_filename_from_config() {
    let project = ProjectBuilder::new("coverage-text-custom")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/logic",
            r#"
            fun and(a: bool, b: bool): bool {
                return a && b;
            }

            fun or(a: bool, b: bool): bool {
                return a || b;
            }
        "#,
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/logic"

            get fun `test-custom-filename`() {
                val result1 = and(true, true);
                expect(result1).toEqual(true);

                val result2 = or(false, true);
                expect(result2).toEqual(true);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: Some(true),
            coverage_format: Some("text".to_owned()),
            coverage_file: Some("my-custom-coverage.txt".to_owned()),
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build();

    let output = project.acton().test().run().success();

    output
        .assert_passed(1)
        .assert_contains("Text coverage file saved in my-custom-coverage.txt")
        .assert_file_exists("my-custom-coverage.txt")
        .assert_file_snapshot_matches(
            "my-custom-coverage.txt",
            "integration/snapshots/test_coverage_text_custom_filename.txt",
        );

    let default_path = project.path().join("coverage.txt");
    assert!(
        !default_path.exists(),
        "Default coverage.txt should not exist when custom filename is specified"
    );
}
