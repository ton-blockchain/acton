//! Reserved integration test module for subagent AD.
//!
//! Ownership boundary for agent AD:
//! - tests/integration/test_std_agent_ad_tests.rs
//! - tests/integration/snapshots/test_std_agent_ad/**
//! - tests/integration/testdata/test_std_agent_ad/**
//! - tests/support/test_std_agent_ad/** (optional)
//!
//! Required test name prefix:
//! - ad_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/tlb/maybe"
"#;

fn run_expect_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("expect_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_expect_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("expect_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ad_stdlib_expect_comparison_helpers_accept_ordered_values() {
    run_expect_success(
        "ad-stdlib-expect-comparison-helpers",
        r#"
get fun `test-ad-stdlib-comparison-helpers`() {
    expect(-10).toBeLess(-9);
    expect(15).toBeGreater(14);
    expect(42).toBeLessOrEqual(42);
    expect(42).toBeGreaterOrEqual(42);
    expect(7).toBeGreaterOrEqual(1);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_comparison_helpers_accept_ordered_values.stdout.txt",
    );
}

#[test]
fn ad_stdlib_expect_approx_helpers_accept_boundary_deltas() {
    run_expect_success(
        "ad-stdlib-expect-approx-boundary-deltas",
        r#"
get fun `test-ad-stdlib-approx-boundary-deltas`() {
    expect(1000).toBeApproxEqAbs(1007, 7);
    expect(1000).toBeApproxEqAbs(993, 7);

    expect(1000).toBeApproxEqRel(1089, 9);
    expect(1000).toBeApproxEqRel(1099, 9);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_approx_helpers_accept_boundary_deltas.stdout.txt",
    );
}

#[test]
fn ad_stdlib_expect_maybe_helpers_handle_some_and_none() {
    run_expect_success(
        "ad-stdlib-expect-maybe-helpers",
        r#"
get fun `test-ad-stdlib-maybe-helpers`() {
    val noneValue = Maybe<int>.none();
    val someValue = Maybe<int>.some(21);

    expect(noneValue).toBeNone();
    expect(someValue).toBeDefined();
    expect(noneValue.unwrapOr(99)).toEqual(99);
    expect(someValue.unwrapOr(99)).toEqual(21);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_maybe_helpers_handle_some_and_none.stdout.txt",
    );
}

#[test]
fn ad_stdlib_expect_tuple_have_length_reports_expected_size() {
    run_expect_success(
        "ad-stdlib-expect-tuple-have-length",
        r#"
get fun `test-ad-stdlib-tuple-have-length`() {
    var values = createEmptyTuple();
    values.push(10);
    values.push(20);
    values.push(30);

    expect(values).toHaveLength(3);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_tuple_have_length_reports_expected_size.stdout.txt",
    );
}

#[test]
fn ad_stdlib_expect_map_helpers_cover_key_value_and_length_checks() {
    run_expect_success(
        "ad-stdlib-expect-map-helpers",
        r#"
get fun `test-ad-stdlib-map-helpers`() {
    var balances = createEmptyMap<int32, int32>();
    balances.set(1, 100);
    balances.set(2, 250);

    expect(balances).toBeNonEmpty();
    expect(balances).toContainKey(1);
    expect(balances).toNotContainKey(3);
    expect(balances).toContainValue(250);
    expect(balances).toNotContainValue(999);
    expect(balances).toHaveLength(2);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_map_helpers_cover_key_value_and_length_checks.stdout.txt",
    );
}

#[test]
fn ad_stdlib_expect_tuple_contain_should_compile_and_assert_at_runtime_bug() {
    run_expect_success(
        "ad-stdlib-expect-tuple-contain-compile-bug",
        r#"
get fun `test-ad-stdlib-tuple-contain-compile-bug`() {
    var values = createEmptyTuple();
    values.push(1);
    expect(values).toContain(1);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_tuple_contain_should_compile_and_assert_at_runtime_bug.stdout.txt",
    );
}

#[test]
fn ad_stdlib_expect_rel_approx_with_negative_actual_should_fail_but_passes_bug() {
    run_expect_failure(
        "ad-stdlib-expect-rel-approx-negative-actual-bug",
        r#"
get fun `test-ad-stdlib-rel-approx-negative-actual-bug`() {
    expectToEndWithExitCode(567);
    // BUG: toBeApproxEqRel uses signed actual value in denominator; expected ASSERTION_FAILED (567), got success (0).
    expect(-100).toBeApproxEqRel(100, 10);
}
"#,
        "integration/snapshots/test_std_agent_ad/ad_stdlib_expect_rel_approx_with_negative_actual_should_fail_but_passes_bug.stdout.txt",
    );
}
