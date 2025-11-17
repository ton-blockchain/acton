use crate::common::assertion;
use crate::support::snapshots::normalize_output;
use crate::support::{ProjectBuilder, TestOutputExt};
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
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("math.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_basic_output.stdout.txt");
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
        .run()
        .success()
        .assert_passed(2)
        .assert_contains(" COVERAGE ")
        .assert_contains("calculator.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_multiple_tests.stdout.txt");
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
        .run()
        .failure()
        .assert_passed(1)
        .assert_failed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("validator.tolk")
        .assert_snapshot_matches(
            "integration/snapshots/test_coverage_with_failing_tests.stdout.txt",
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
        .run()
        .success()
        .assert_passed(2)
        .assert_contains(" COVERAGE ")
        .assert_contains("helpers.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_with_filter_all.stdout.txt");

    project
        .acton()
        .test()
        .filter("test-unit-.*")
        .with_coverage()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("helpers.tolk")
        .assert_snapshot_matches("integration/snapshots/test_coverage_with_filter.stdout.txt");
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

    // Clean up any existing lcov.info
    let lcov_path = project.path().join("lcov.info");
    let _ = fs::remove_file(&lcov_path);

    let output = project
        .acton()
        .test()
        .with_coverage_format("lcov")
        .run()
        .success();

    output
        .assert_passed(1)
        .assert_contains("LCOV file saved in lcov.info");

    let lcov_content = fs::read_to_string(&lcov_path).expect("Should read lcov.info");
    assertion().eq(
        normalize_output(lcov_content.as_str(), &project.path().to_path_buf()),
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
