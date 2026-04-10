use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const ASSERT_IMPORTS: &str = r#"
import "../../lib/testing/assert"
"#;

#[test]
fn assert_not_equal_reports_diagnostics_for_complex_tuple_and_map_values() {
    let source = format!(
        r#"{ASSERT_IMPORTS}
get fun `test ej stdlib assert not equal complex tuple diagnostic`() {{
    var nested = [];
    nested.push(22);
    nested.push(33);

    var payload = [];
    payload.push(11);
    payload.push(nested);
    payload.push("alpha");

    Assert.notEqual(payload, payload, "ej tuple/map notEqual diagnostic tuple");
}}

get fun `test ej stdlib assert not equal complex map diagnostic`() {{
    var balances = createEmptyMap<int32, int32>();
    balances.set(7, 70);
    balances.set(11, 110);

    Assert.notEqual(balances, balances, "ej tuple/map notEqual diagnostic map");
}}
"#
    );

    ProjectBuilder::new("ej-stdlib-assert-not-equal-complex-tuple-map")
        .test_file("assert_not_equal_complex", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(2)
        .assert_contains("ej tuple/map notEqual diagnostic tuple")
        .assert_contains("ej tuple/map notEqual diagnostic map")
        .assert_contains("Values are equal but expected to be different")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_not_equal_reports_diagnostics_for_complex_tuple_and_map_values/assert_not_equal_reports_diagnostics_for_complex_tuple_and_map_values.stdout.txt",
        );
}
