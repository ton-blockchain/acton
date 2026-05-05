use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
"#;

fn wrap_assert_source(test_body: &str) -> String {
    format!("{ASSERT_IMPORTS}\n{test_body}\n")
}

#[test]
fn assert_equal_passes_for_identical_tuple_values() {
    let source = wrap_assert_source(
        r#"
get fun `test ek stdlib assert equal pass identical tuple`() {
    var left = [];
    left.push(10);
    left.push("ok");

    var right = [];
    right.push(10);
    right.push("ok");

    Assert.equal(left, right, "ek Assert.equal pass tuple");
}
"#,
    );

    ProjectBuilder::new("ek-stdlib-assert-equal-pass-identical-tuple")
        .test_file("assert_equal_pass", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_equal_passes_for_identical_tuple_values/assert_equal_passes_for_identical_tuple_values.stdout.txt",
        );
}

#[test]
fn assert_equal_passes_for_nullable_struct_matching_non_nullable_struct() {
    let source = wrap_assert_source(
        r#"
struct NullablePoint {
    x: int
    y: int
}

get fun `test ek stdlib assert equal pass nullable struct`() {
    val actual: NullablePoint? = NullablePoint {
        x: 10,
        y: 20,
    };

    Assert.equal(
        actual,
        NullablePoint {
            x: 10,
            y: 20,
        },
        "ek Assert.equal nullable struct compares by rendered Tolk value",
    );
}
"#,
    );

    ProjectBuilder::new("ek-stdlib-assert-equal-pass-nullable-struct")
        .test_file("assert_equal_nullable_struct", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_equal_passes_for_identical_tuple_values/assert_equal_passes_for_nullable_struct_matching_non_nullable_struct.stdout.txt",
        );
}

#[test]
fn assert_equal_passes_for_union_scalar_matching_plain_scalar() {
    let source = wrap_assert_source(
        r#"
fun assertUnionScalarEqual(actual: int | bool): void {
    Assert.equal(
        actual,
        10,
        "ek Assert.equal union scalar compares by rendered Tolk value",
    );
}

get fun `test ek stdlib assert equal pass union scalar`() {
    assertUnionScalarEqual(10 as int | bool);
}
"#,
    );

    ProjectBuilder::new("ek-stdlib-assert-equal-pass-union-scalar")
        .test_file("assert_equal_union_scalar", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_equal_passes_for_identical_tuple_values/assert_equal_passes_for_union_scalar_matching_plain_scalar.stdout.txt",
        );
}

#[test]
fn assert_equal_reports_actual_and_expected_on_failure() {
    let source = wrap_assert_source(
        r#"
get fun `test ek stdlib assert equal failure diagnostics`() {
    Assert.equal(42, 41, "ek Assert.equal mismatch diagnostic");
}
"#,
    );

    ProjectBuilder::new("ek-stdlib-assert-equal-failure-diagnostics")
        .test_file("assert_equal_failure", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("ek Assert.equal mismatch diagnostic")
        .assert_contains("(")
        .assert_contains("42")
        .assert_contains("41")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_equal_passes_for_identical_tuple_values/assert_equal_reports_actual_and_expected_on_failure.stdout.txt",
        );
}
