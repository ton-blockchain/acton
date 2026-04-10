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
