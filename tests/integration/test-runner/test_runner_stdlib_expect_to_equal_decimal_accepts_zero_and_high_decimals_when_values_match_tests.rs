use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn wrap_expect_source(test_body: &str) -> String {
    format!("{EXPECT_IMPORTS}\n{test_body}\n")
}

#[test]
fn expect_to_equal_decimal_accepts_zero_and_high_decimals_when_values_match() {
    let source = wrap_expect_source(
        r#"
get fun `test-ef-decimal-equality-zero-and-high-precision`() {
    expect(123).toEqualDecimal(123, 0);
    expect(42).toEqualDecimal(42, 18);
    expect(-42).toEqualDecimal(-42, 18);
}
"#,
    );

    ProjectBuilder::new("ef-stdlib-decimal-equality-zero-high-precision")
        .test_file("expect_decimal_edges", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_expect_to_equal_decimal_accepts_zero_and_high_decimals_when_values_match_tests/expect_to_equal_decimal_accepts_zero_and_high_decimals_when_values_match.stdout.txt",
        );
}

#[test]
fn expect_to_equal_decimal_formats_zero_and_high_decimal_failures() {
    let source = wrap_expect_source(
        r#"
get fun `test-ef-decimal-failure-format-zero-decimals`() {
    expect(123).toEqualDecimal(124, 0);
}

get fun `test-ef-decimal-failure-format-high-decimals`() {
    expect(-15).toEqualDecimal(-16, 18);
}
"#,
    );

    ProjectBuilder::new("ef-stdlib-decimal-failure-format-zero-high")
        .test_file("expect_decimal_edges", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(2)
        .assert_contains("Actual:   123.0")
        .assert_contains("Expected: 124.0")
        .assert_contains("Actual:   -0.000000000000000015")
        .assert_contains("Expected: -0.000000000000000016")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_expect_to_equal_decimal_accepts_zero_and_high_decimals_when_values_match_tests/expect_to_equal_decimal_formats_zero_and_high_decimal_failures.stdout.txt",
        );
}
