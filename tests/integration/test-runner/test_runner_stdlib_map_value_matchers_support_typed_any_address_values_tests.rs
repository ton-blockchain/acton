use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("ed_map_value_matchers", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

fn run_failure(project_name: &str, test_body: &str, snapshot_path: &str, failed_count: usize) {
    let source = format!("{EXPECT_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("ed_map_value_matchers", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(failed_count)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn map_value_matchers_support_typed_any_address_values() {
    run_success(
        "ed-stdlib-map-value-matchers-typed-any-address-success",
        r#"
get fun `test-ed-stdlib-map-value-matchers-typed-any-address-success`() {
    val alice = address("0:00000000000000000000000000000000000000000000000000000000000000AA") as any_address;
    val bob = address("0:00000000000000000000000000000000000000000000000000000000000000BB") as any_address;
    val carol = address("0:00000000000000000000000000000000000000000000000000000000000000CC") as any_address;

    var balances = createEmptyMap<int32, any_address>();
    balances.set(1, alice);
    balances.set(2, bob);

    expect(balances).toContainValue(alice);
    expect(balances).toContainValue(bob);
    expect(balances).toNotContainValue(carol);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_map_value_matchers_support_typed_any_address_values_tests/map_value_matchers_support_typed_any_address_values.stdout.txt",
    );
}

#[test]
fn map_value_matchers_report_typed_any_address_mismatches() {
    run_failure(
        "ed-stdlib-map-value-matchers-typed-any-address-mismatch-reporting",
        r#"
get fun `test-ed-stdlib-map-to-contain-value-reports-missing-typed-address`() {
    val alice = address("0:00000000000000000000000000000000000000000000000000000000000000AA") as any_address;
    val missing = address("0:00000000000000000000000000000000000000000000000000000000000000DD") as any_address;

    var balances = createEmptyMap<int32, any_address>();
    balances.set(1, alice);

    expect(balances).toContainValue(missing);
}

get fun `test-ed-stdlib-map-to-not-contain-value-reports-present-typed-address`() {
    val alice = address("0:00000000000000000000000000000000000000000000000000000000000000AA") as any_address;

    var balances = createEmptyMap<int32, any_address>();
    balances.set(1, alice);

    expect(balances).toNotContainValue(alice);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_map_value_matchers_support_typed_any_address_values_tests/map_value_matchers_report_typed_any_address_mismatches.stdout.txt",
        2,
    );
}
