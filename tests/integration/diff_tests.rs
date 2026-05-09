use crate::support::assertions::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn test_diff_for_numbers() {
    let project = ProjectBuilder::new("diff-numbers")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            get fun `test diff`() {
                expect(10).toEqual(20)
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_numbers.stdout.txt");
}

#[test]
fn test_diff_for_tensors() {
    let project = ProjectBuilder::new("diff-tensors")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            get fun `test diff`() {
                expect((10, 20)).toEqual((10, 30))
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_tensors.stdout.txt");
}

#[test]
fn test_diff_for_bools() {
    let project = ProjectBuilder::new("diff-bools")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            get fun `test diff`() {
                expect(true).toEqual(false)
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_bools.stdout.txt");
}

#[test]
fn test_diff_for_strings() {
    let project = ProjectBuilder::new("diff-strings")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            get fun `test diff`() {
                expect("hello").toEqual("world")
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_strings.stdout.txt");
}

#[test]
fn test_diff_for_nullables() {
    let project = ProjectBuilder::new("diff-nullables")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            get fun `test diff`() {
                expect(10).toEqual(null)
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_nullables.stdout.txt");
}

#[test]
fn test_diff_for_structs() {
    let project = ProjectBuilder::new("diff-structs")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            struct Point {
                x: int,
                y: int
            }

            get fun `test diff`() {
                expect(Point{x: 1, y: 2}).toEqual(Point{x: 1, y: 3})
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_structs.stdout.txt");
}

#[test]
fn test_diff_for_nested_structs() {
    let project = ProjectBuilder::new("diff-nested-structs")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            struct Line {
                start: Point
                end: Point
            }

            struct Point {
                x: int
                y: int
            }

            get fun `test diff`() {
                expect(Line { start: Point{ x: 1, y: 2 }, end: Point{ x: 1, y: 3 } }).toEqual(Line { start: Point{ x: 2, y: 2 }, end: Point{ x: 2, y: 3 } })
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/diff/test_diff_for_nested_structs.stdout.txt",
        );
}

#[test]
fn test_diff_for_structs_with_nullable_struct_and_union_fields() {
    let project = ProjectBuilder::new("diff-structs-nullable-union-fields")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            struct Point {
                x: int
                y: int
            }

            struct LeftChoice {
                point: Point?
                code: int
            }

            struct RightChoice {
                value: int
            }

            struct EmptyChoice {}

            type Choice = LeftChoice | RightChoice | EmptyChoice

            struct Record {
                id: int
                samePoint: Point?
                changedPoint: Point?
                missingPoint: Point?
                sameChoice: Choice
                changedChoice: Choice
                sameInt: int
                changedInt: int
            }

            fun point(x: int, y: int): Point {
                return Point { x, y };
            }

            fun leftChoice(x: int, y: int, code: int): LeftChoice {
                return LeftChoice {
                    point: point(x, y),
                    code,
                };
            }

            get fun `test diff nullable struct and same union variant fields`() {
                expect(Record {
                    id: 1,
                    samePoint: point(10, 20),
                    changedPoint: point(30, 40),
                    missingPoint: null,
                    sameChoice: EmptyChoice {} as Choice,
                    changedChoice: leftChoice(5, 6, 7) as Choice,
                    sameInt: 99,
                    changedInt: 100,
                }).toEqual(Record {
                    id: 1,
                    samePoint: point(10, 20),
                    changedPoint: point(30, 41),
                    missingPoint: point(50, 60),
                    sameChoice: EmptyChoice {} as Choice,
                    changedChoice: leftChoice(5, 9, 7) as Choice,
                    sameInt: 99,
                    changedInt: 101,
                })
            }

            get fun `test diff nullable struct and different union variant fields`() {
                expect(Record {
                    id: 2,
                    samePoint: null,
                    changedPoint: point(70, 80),
                    missingPoint: point(90, 100),
                    sameChoice: EmptyChoice {} as Choice,
                    changedChoice: RightChoice { value: 7 } as Choice,
                    sameInt: 199,
                    changedInt: 200,
                }).toEqual(Record {
                    id: 2,
                    samePoint: null,
                    changedPoint: null,
                    missingPoint: point(90, 101),
                    sameChoice: EmptyChoice {} as Choice,
                    changedChoice: EmptyChoice {} as Choice,
                    sameInt: 199,
                    changedInt: 201,
                })
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/diff/test_diff_for_structs_with_nullable_struct_and_union_fields.stdout.txt",
        );
}

#[test]
fn test_diff_for_top_level_unions() {
    let project = ProjectBuilder::new("diff-top-level-unions")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            struct Point {
                x: int
                y: int
            }

            struct LeftChoice {
                point: Point
                code: int
            }

            struct RightChoice {
                value: int
            }

            struct EmptyChoice {}

            type Choice = LeftChoice | RightChoice | EmptyChoice

            fun point(x: int, y: int): Point {
                return Point { x, y };
            }

            fun leftChoice(x: int, y: int, code: int): LeftChoice {
                return LeftChoice {
                    point: point(x, y),
                    code,
                };
            }

            get fun `test diff same top level union variant`() {
                expect(leftChoice(5, 6, 7) as Choice).toEqual(leftChoice(5, 9, 7) as Choice)
            }

            get fun `test diff different top level union variants`() {
                expect(RightChoice { value: 7 } as Choice).toEqual(EmptyChoice {} as Choice)
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/diff/test_diff_for_top_level_unions.stdout.txt",
        );
}

#[test]
fn test_diff_for_addresses() {
    let project = ProjectBuilder::new("diff-addresses")
        .contract("simple", "fun main() {}")
        .test_file(
            "simple",
            r#"
            import "../../lib/testing/expect"

            get fun `test diff`() {
                val addr1 = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
                val addr2 = address("EQD__________________________________________0vo");
                expect(addr1).toEqual(addr2);
            }
        "#,
        )
        .build();

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/diff/test_diff_for_addresses.stdout.txt");
}
