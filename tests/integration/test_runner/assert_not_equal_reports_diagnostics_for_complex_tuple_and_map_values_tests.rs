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
    var balances = map<int32, int32> [];
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

#[test]
fn assert_not_equal_passes_for_rendered_unequal_nullable_structs_and_unions() {
    let source = format!(
        r#"{ASSERT_IMPORTS}
struct NotEqualPoint {{
    x: int
    y: int
}}

struct NotEqualEventPoint {{
    point: NotEqualPoint?
    queryId: int
}}

struct NotEqualEventCode {{
    code: int
}}

struct NotEqualNone {{}}

type NotEqualEvent = NotEqualEventPoint | NotEqualEventCode | NotEqualNone

fun notEqualPoint(x: int, y: int): NotEqualPoint {{
    return NotEqualPoint {{ x, y }};
}}

fun notEqualEventPoint(x: int, y: int, queryId: int): NotEqualEventPoint {{
    return NotEqualEventPoint {{
        point: notEqualPoint(x, y),
        queryId,
    }};
}}

get fun `test ej stdlib assert not equal pass nullable struct value differs`() {{
    val actual: NotEqualPoint? = notEqualPoint(1, 2);
    Assert.notEqual(
        actual,
        notEqualPoint(1, 3),
        "ej Assert.notEqual nullable struct detects rendered difference",
    );
}}

get fun `test ej stdlib assert not equal pass nullable struct differs from null`() {{
    val actual: NotEqualPoint? = notEqualPoint(4, 5);
    Assert.notEqual(
        actual,
        null,
        "ej Assert.notEqual nullable struct detects null difference",
    );
}}

get fun `test ej stdlib assert not equal pass same union variant payload differs`() {{
    val left: NotEqualEvent = notEqualEventPoint(6, 7, 8) as NotEqualEvent;
    val right: NotEqualEvent = notEqualEventPoint(6, 9, 8) as NotEqualEvent;
    Assert.notEqual(
        left,
        right,
        "ej Assert.notEqual same union variant detects payload difference",
    );
}}

get fun `test ej stdlib assert not equal pass different union variants`() {{
    val left: NotEqualEvent = NotEqualEventCode {{ code: 10 }} as NotEqualEvent;
    val right: NotEqualEvent = NotEqualNone {{}} as NotEqualEvent;
    Assert.notEqual(
        left,
        right,
        "ej Assert.notEqual detects different union variants",
    );
}}
"#
    );

    ProjectBuilder::new("ej-stdlib-assert-not-equal-rendered-unequal")
        .test_file("assert_not_equal_rendered_unequal", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(4)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_not_equal_reports_diagnostics_for_complex_tuple_and_map_values/assert_not_equal_passes_for_rendered_unequal_nullable_structs_and_unions.stdout.txt",
        );
}

#[test]
fn assert_not_equal_reports_nullable_struct_matching_non_nullable_struct_as_equal() {
    let source = format!(
        r#"{ASSERT_IMPORTS}
struct NullablePoint {{
    x: int
    y: int
}}

get fun `test ej stdlib assert not equal nullable struct diagnostic`() {{
    val actual: NullablePoint? = NullablePoint {{
        x: 10,
        y: 20,
    }};

    Assert.notEqual(
        actual,
        NullablePoint {{
            x: 10,
            y: 20,
        }},
        "ej Assert.notEqual nullable struct compares by rendered Tolk value",
    );
}}
"#
    );

    ProjectBuilder::new("ej-stdlib-assert-not-equal-nullable-struct")
        .test_file("assert_not_equal_nullable_struct", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("ej Assert.notEqual nullable struct compares by rendered Tolk value")
        .assert_contains("Values are equal but expected to be different")
        .assert_contains("NullablePoint")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_not_equal_reports_diagnostics_for_complex_tuple_and_map_values/assert_not_equal_reports_nullable_struct_matching_non_nullable_struct_as_equal.stdout.txt",
        );
}

#[test]
fn assert_not_equal_reports_union_scalar_matching_plain_scalar_as_equal() {
    let source = format!(
        r#"{ASSERT_IMPORTS}
fun assertUnionScalarNotEqual(actual: int | bool): void {{
    Assert.notEqual(
        actual,
        10,
        "ej Assert.notEqual union scalar compares by rendered Tolk value",
    );
}}

get fun `test ej stdlib assert not equal union scalar diagnostic`() {{
    assertUnionScalarNotEqual(10 as int | bool);
}}
"#
    );

    ProjectBuilder::new("ej-stdlib-assert-not-equal-union-scalar")
        .test_file("assert_not_equal_union_scalar", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("ej Assert.notEqual union scalar compares by rendered Tolk value")
        .assert_contains("Values are equal but expected to be different")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/assert_not_equal_reports_diagnostics_for_complex_tuple_and_map_values/assert_not_equal_reports_union_scalar_matching_plain_scalar_as_equal.stdout.txt",
        );
}
