use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const COMPARISON_PASS_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test less pass`() {
    expect(-3).toBeLess(0);
}

get fun `test greater pass`() {
    expect(42).toBeGreater(7);
}

get fun `test less or equal boundary pass`() {
    expect(10).toBeLessOrEqual(10);
}

get fun `test greater or equal boundary pass`() {
    expect(-5).toBeGreaterOrEqual(-5);
}
"#;

const COMPARISON_FAIL_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test less fail`() {
    expect(5).toBeLess(5);
}

get fun `test greater fail`() {
    expect(5).toBeGreater(6);
}

get fun `test less or equal fail`() {
    expect(8).toBeLessOrEqual(7);
}

get fun `test greater or equal fail`() {
    expect(-2).toBeGreaterOrEqual(-1);
}
"#;

const APPROX_PASS_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test approx abs pass`() {
    expect(1000).toBeApproxEqAbs(995, 5);
}

get fun `test approx abs boundary pass`() {
    expect(-50).toBeApproxEqAbs(-55, 5);
}

get fun `test approx rel pass`() {
    expect(200).toBeApproxEqRel(220, 10);
}

get fun `test approx rel boundary pass`() {
    expect(40).toBeApproxEqRel(44, 10);
}
"#;

const APPROX_FAIL_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test approx abs fail`() {
    expect(10).toBeApproxEqAbs(20, 5);
}

get fun `test approx rel fail`() {
    expect(10).toBeApproxEqRel(20, 50);
}

get fun `test approx rel small threshold fail`() {
    expect(200).toBeApproxEqRel(220, 9);
}
"#;

const APPROX_REL_CUSTOM_PASS_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test parts per million boundary`() {
    expect(1_000_000).toBeApproxEqRel(1_000_001, 1, 1_000_000);
    expect(1_000_000).toBeApproxEqRel(999_999, 1, 1_000_000);
}

get fun `test eighteen decimal precision`() {
    val actual = 1_000_000_000_000_000_000;
    expect(actual).toBeApproxEqRel(actual + 1, 1, actual);
    expect(-actual).toBeApproxEqRel(-actual - 1, 1, actual);
}

get fun `test custom precision floors delta`() {
    expect(1000).toBeApproxEqRel(1099, 9, 100);
    expect(10_000_000).toBeApproxEqRel(10_000_019, 1, 1_000_000);
    expect(10_000_000).toBeApproxEqRel(10_000_009, 0, 1_000_000);
}

get fun `test denominator below one hundred`() {
    expect(100).toBeApproxEqRel(150, 1, 2);
    expect(100).toBeApproxEqRel(200, 1, 1);
}

get fun `test equal values at custom precision`() {
    expect(0).toBeApproxEqRel(0, 0, 1_000_000);
    expect(42).toBeApproxEqRel(42, 0, 1_000_000);
    expect(-42).toBeApproxEqRel(-42, 0, 1_000_000);
}

get fun `test wide intermediate multiplication`() {
    val actual = 1 << 240;
    val difference = 1 << 200;
    expect(actual).toBeApproxEqRel(actual + difference, 1 << 60, 1 << 100);
}
"#;

const APPROX_REL_CUSTOM_FAIL_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test parts per million above boundary`() {
    expect(1_000_000).toBeApproxEqRel(1_000_002, 1, 1_000_000);
}

get fun `test eighteen decimal precision above boundary`() {
    val actual = 1_000_000_000_000_000_000;
    expect(actual).toBeApproxEqRel(actual + 2, 1, actual);
}

get fun `test negative actual above boundary`() {
    expect(-1_000_000).toBeApproxEqRel(-1_000_002, 1, 1_000_000);
}

get fun `test zero actual with custom precision`() {
    expect(0).toBeApproxEqRel(1, 1, 1_000_000);
}

get fun `test zero delta with custom precision`() {
    expect(1_000_000).toBeApproxEqRel(1_000_001, 0, 1_000_000);
}

get fun `test denominator below one hundred above boundary`() {
    expect(100).toBeApproxEqRel(200, 1, 2);
}
"#;

const APPROX_REL_INVALID_DENOMINATOR_TESTS: &str = r#"
import "../../lib/testing/expect"

get fun `test zero denominator even for equal zeros`() {
    expect(0).toBeApproxEqRel(0, 0, 0);
}

get fun `test negative denominator even for equal values`() {
    expect(100).toBeApproxEqRel(100, 10, -100);
}
"#;

#[test]
fn comparison_matchers_pass() {
    ProjectBuilder::new("lib-api-comparison-pass")
        .test_file("comparison", COMPARISON_PASS_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(4)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/comparison_matchers_pass.stdout.txt",
        );
}

#[test]
fn comparison_matchers_fail() {
    ProjectBuilder::new("lib-api-comparison-fail")
        .test_file("comparison", COMPARISON_FAIL_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(4)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/comparison_matchers_fail.stdout.txt",
        );
}

#[test]
fn approx_matchers_pass() {
    ProjectBuilder::new("lib-api-approx-pass")
        .test_file("approx", APPROX_PASS_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(4)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/approx_matchers_pass.stdout.txt",
        );
}

#[test]
fn approx_matchers_fail() {
    ProjectBuilder::new("lib-api-approx-fail")
        .test_file("approx", APPROX_FAIL_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(3)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/approx_matchers_fail.stdout.txt",
        );
}

#[test]
fn approx_relative_custom_denominators_pass() {
    ProjectBuilder::new("lib-api-approx-rel-custom-pass")
        .test_file("approx", APPROX_REL_CUSTOM_PASS_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(6)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/approx_relative_custom_denominators_pass.stdout.txt",
        );
}

#[test]
fn approx_relative_custom_denominators_fail() {
    ProjectBuilder::new("lib-api-approx-rel-custom-fail")
        .test_file("approx", APPROX_REL_CUSTOM_FAIL_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(6)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/approx_relative_custom_denominators_fail.stdout.txt",
        );
}

#[test]
fn approx_relative_rejects_invalid_denominators() {
    ProjectBuilder::new("lib-api-approx-rel-invalid-denominator")
        .test_file("approx", APPROX_REL_INVALID_DENOMINATOR_TESTS)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(2)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_matchers/approx_relative_rejects_invalid_denominators.stdout.txt",
        );
}
