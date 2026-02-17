//! Reserved integration test module for subagent AC.
//!
//! Ownership boundary for agent AC:
//! - tests/integration/test_std_agent_ac_tests.rs
//! - tests/integration/snapshots/test_std_agent_ac/**
//! - tests/integration/testdata/test_std_agent_ac/**
//! - tests/support/test_std_agent_ac/** (optional)
//!
//! Required test name prefix:
//! - ac_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
import "../../lib/testing/expect"
"#;

fn wrap_assert_source(test_body: &str) -> String {
    format!("{ASSERT_IMPORTS}\n{test_body}\n")
}

#[test]
fn ac_stdlib_assert_consumes_less_than2_returns_function_result_when_within_limit() {
    let source = wrap_assert_source(
        r#"
get fun `test-ac-consumes-less-than2-returns-result`() {
    val sum = Assert.consumesLessThan2(
        fun(a: int, b: int): int {
            return a + b;
        },
        40,
        2,
        10000
    );
    expect(sum).toEqual(42);
}
"#,
    );

    ProjectBuilder::new("ac-stdlib-assert-consumes-less-than2-pass")
        .test_file("assert_gas", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ac/ac_stdlib_assert_consumes_less_than2_returns_function_result_when_within_limit.stdout.txt",
        );
}

#[test]
fn ac_stdlib_assert_consumes_less_than_reports_human_readable_gas_failure() {
    let source = wrap_assert_source(
        r#"
get fun `test-ac-consumes-less-than-failure`() {
    Assert.consumesLessThan(
        fun() {
            var i = 0;
            while (i < 120) {
                i = i + 1;
            }
        },
        0
    );
}
"#,
    );

    ProjectBuilder::new("ac-stdlib-assert-consumes-less-than-fail")
        .test_file("assert_gas", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Gas consumption was expected to be less than or equal to 0")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ac/ac_stdlib_assert_consumes_less_than_reports_human_readable_gas_failure.stdout.txt",
        );
}

#[test]
fn ac_stdlib_expect_to_equal_decimal_formats_negative_fraction_values_in_failure() {
    let source = wrap_assert_source(
        r#"
get fun `test-ac-decimal-default-message-formatting`() {
    expect(-15).toEqualDecimal(-10, 2);
}
"#,
    );

    ProjectBuilder::new("ac-stdlib-decimal-default-formatting")
        .test_file("assert_decimal", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Actual:   -0.15")
        .assert_contains("Expected: -0.1")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ac/ac_stdlib_expect_to_equal_decimal_formats_negative_fraction_values_in_failure.stdout.txt",
        );
}

#[test]
fn ac_stdlib_assert_equal_decimal_uses_custom_message_and_location_from_arguments() {
    let source = wrap_assert_source(
        r#"
get fun `test-ac-decimal-custom-message-location`() {
    Assert.equalDecimal(
        120,
        121,
        1,
        "custom decimal mismatch from Assert.equalDecimal",
        "tests/custom_decimal.test.tolk:42:7"
    );
}
"#,
    );

    ProjectBuilder::new("ac-stdlib-decimal-custom-message-location")
        .test_file("assert_decimal", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("custom decimal mismatch from Assert.equalDecimal")
        .assert_contains("tests/custom_decimal.test.tolk:42:7")
        .assert_not_contains("toEqualDecimal(expected)")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ac/ac_stdlib_assert_equal_decimal_uses_custom_message_and_location_from_arguments.stdout.txt",
        );
}

#[test]
fn ac_stdlib_assert_not_equal_failure_surfaces_ffi_not_equal_branch_details() {
    let source = wrap_assert_source(
        r#"
get fun `test-ac-assert-not-equal-failure-branch`() {
    Assert.notEqual(7, 7, "numbers should differ");
}
"#,
    );

    ProjectBuilder::new("ac-stdlib-assert-not-equal-failure")
        .test_file("assert_bin", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("numbers should differ")
        .assert_contains("Values are equal but expected to be different")
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_ac/ac_stdlib_assert_not_equal_failure_surfaces_ffi_not_equal_branch_details.stdout.txt",
        );
}
