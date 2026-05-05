use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXPECT_IMPORTS: &str = r#"
import "../../lib/testing/expect"
"#;

fn wrap_expect_source(test_body: &str) -> String {
    format!("{EXPECT_IMPORTS}\n{test_body}\n")
}

#[test]
fn expect_to_equal_handles_nullable_structs_and_unions_by_rendered_value() {
    let source = wrap_expect_source(
        r"
struct EqPoint {
    x: int
    y: int
}

struct EqBox {
    point: EqPoint?
    tag: int
}

struct EqCode {
    code: int
}

struct EqNone {}

struct EqEventPoint {
    point: EqPoint?
    queryId: int
}

struct EqEventCode {
    code: EqCode
    ok: bool
}

type EqScalar = int | bool
type EqShape = EqPoint | EqCode
type EqEvent = EqEventPoint | EqEventCode | EqNone
type EqPointOrNull = EqPoint | null

struct EqEnvelope {
    event: EqEvent
    owner: EqPoint?
}

fun eqPoint(x: int, y: int): EqPoint {
    return EqPoint { x, y };
}

fun eqEventPoint(x: int, y: int, queryId: int): EqEventPoint {
    return EqEventPoint {
        point: eqPoint(x, y),
        queryId,
    };
}

fun expectScalarEqualsPlainInt(actual: EqScalar): void {
    expect(actual).toEqual(10);
}

fun expectPlainIntEqualsScalar(expected: EqScalar): void {
    expect(10).toEqual(expected);
}

fun expectScalarEqualsPlainBool(actual: EqScalar): void {
    expect(actual).toEqual(true);
}

fun expectShapeEqualsPlainPoint(actual: EqShape): void {
    expect(actual).toEqual(eqPoint(1, 2));
}

fun expectPlainPointEqualsShape(expected: EqShape): void {
    expect(eqPoint(1, 2)).toEqual(expected);
}

fun expectEventEqualsPlainEventPoint(actual: EqEvent): void {
    expect(actual).toEqual(eqEventPoint(3, 4, 77));
}

fun expectEventEqualsPlainNone(actual: EqEvent): void {
    expect(actual).toEqual(EqNone {});
}

fun expectPointOrNullEqualsNull(actual: EqPointOrNull): void {
    expect(actual).toEqual(null);
}

get fun `test fk expect toEqual nullable struct equals plain struct`() {
    val actual: EqPoint? = eqPoint(1, 2);
    expect(actual).toEqual(eqPoint(1, 2));
}

get fun `test fk expect toEqual plain struct equals nullable struct`() {
    val expected: EqPoint? = eqPoint(1, 2);
    expect(eqPoint(1, 2)).toEqual(expected);
}

get fun `test fk expect toEqual nullable struct field equals plain struct`() {
    val actual = EqBox {
        point: eqPoint(5, 6),
        tag: 11,
    };
    expect(actual.point).toEqual(eqPoint(5, 6));
}

get fun `test fk expect toEqual nested nullable struct wrappers match`() {
    expect(
        EqBox {
            point: eqPoint(7, 8),
            tag: 12,
        },
    ).toEqual(EqBox {
        point: eqPoint(7, 8),
        tag: 12,
    });
}

get fun `test fk expect toEqual tuple with nullable struct matches plain struct tuple`() {
    val actualPoint: EqPoint? = eqPoint(9, 10);
    expect((actualPoint, 13)).toEqual((eqPoint(9, 10), 13));
}

get fun `test fk expect toEqual nullable struct null equals null literal`() {
    val actual: EqPoint? = null;
    expect(actual).toEqual(null);
}

get fun `test fk expect toEqual scalar union int equals plain int`() {
    expectScalarEqualsPlainInt(10 as int | bool);
}

get fun `test fk expect toEqual plain int equals scalar union int`() {
    expectPlainIntEqualsScalar(10 as int | bool);
}

get fun `test fk expect toEqual scalar union bool equals plain bool`() {
    expectScalarEqualsPlainBool(true as int | bool);
}

get fun `test fk expect toEqual bool does not collapse to raw tvm minus one`() {
    expectToEndWithExitCode(567);
    expect(true).toEqual(-1);
}

get fun `test fk expect toEqual scalar union bool does not collapse to raw tvm minus one`() {
    expectToEndWithExitCode(567);
    expect(true as int | bool).toEqual(-1);
}

get fun `test fk expect toEqual struct union equals plain struct`() {
    expectShapeEqualsPlainPoint(eqPoint(1, 2) as EqPoint | EqCode);
}

get fun `test fk expect toEqual plain struct equals struct union`() {
    expectPlainPointEqualsShape(eqPoint(1, 2) as EqPoint | EqCode);
}

get fun `test fk expect toEqual complex union event equals plain event struct`() {
    expectEventEqualsPlainEventPoint(eqEventPoint(3, 4, 77) as EqEvent);
}

get fun `test fk expect toEqual complex union event equals same union event`() {
    val left: EqEvent = eqEventPoint(3, 4, 77) as EqEvent;
    val right: EqEvent = eqEventPoint(3, 4, 77) as EqEvent;
    expect(left).toEqual(right);
}

get fun `test fk expect toEqual unit union variant equals empty struct`() {
    expectEventEqualsPlainNone(EqNone {} as EqEvent);
}

get fun `test fk expect toEqual null union variant equals null literal`() {
    expectPointOrNullEqualsNull(null as EqPoint | null);
}

get fun `test fk expect toEqual struct with nested union and nullable fields matches`() {
    val actual = EqEnvelope {
        event: eqEventPoint(21, 22, 23) as EqEvent,
        owner: eqPoint(24, 25),
    };
    expect(actual).toEqual(EqEnvelope {
        event: eqEventPoint(21, 22, 23) as EqEvent,
        owner: eqPoint(24, 25),
    });
}

get fun `test fk expect toEqual nested union field equals plain event struct`() {
    val actual = EqEnvelope {
        event: eqEventPoint(31, 32, 33) as EqEvent,
        owner: null,
    };
    expect(actual.event).toEqual(eqEventPoint(31, 32, 33));
}
",
    );

    ProjectBuilder::new("fk-stdlib-expect-to-equal-rendered-values")
        .test_file("expect_to_equal_rendered_values", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(19)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/expect_to_equal_handles_nullable_structs_and_unions/expect_to_equal_handles_nullable_structs_and_unions_by_rendered_value.stdout.txt",
        );
}
