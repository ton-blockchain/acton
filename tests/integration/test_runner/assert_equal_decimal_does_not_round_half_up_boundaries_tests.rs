use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
"#;

fn wrap_assert_source(test_body: &str) -> String {
    format!("{ASSERT_IMPORTS}\n{test_body}\n")
}

#[test]
fn assert_equal_decimal_does_not_round_half_up_boundaries() {
    let source = wrap_assert_source(
        r"
get fun `test-eg-assert-equal-decimal-no-round-half-up`() {
    Assert.equalDecimal(149, 150, 2);
}
",
    );

    ProjectBuilder::new("eg-stdlib-assert-equal-decimal-no-round-half-up")
        .test_file("assert_equal_decimal_rounding", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Actual:   1.49")
        .assert_contains("Expected: 1.5")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_equal_decimal_does_not_round_half_up_boundaries/assert_equal_decimal_does_not_round_half_up_boundaries.stdout.txt",
        );
}

#[test]
fn assert_equal_decimal_formats_negative_small_values_with_leading_zero() {
    let source = wrap_assert_source(
        r"
get fun `test-eg-assert-equal-decimal-negative-leading-zero`() {
    Assert.equalDecimal(-5, -6, 2);
}
",
    );

    ProjectBuilder::new("eg-stdlib-assert-equal-decimal-negative-leading-zero")
        .test_file("assert_equal_decimal_signed", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Actual:   -0.05")
        .assert_contains("Expected: -0.06")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_equal_decimal_does_not_round_half_up_boundaries/assert_equal_decimal_formats_negative_small_values_with_leading_zero.stdout.txt",
        );
}
