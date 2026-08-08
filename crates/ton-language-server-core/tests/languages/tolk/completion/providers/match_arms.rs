use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_missing_union_enum_and_else_arms() {
    // An empty union match offers every type branch, else, and the fill-all action.
    CompletionTest::new(
        "
            struct Foo {}
            struct Bar {}
            fun main(value: Foo | Bar) {
                match (value) {
                    <caret>
                }
            }
        ",
    )
    .labels(&["Foo", "Bar", "else", "Fill all cases..."])
    .check(expect![[r#"
        label              kind     detail  edit     text
        Bar                Event     => {}  4:8-4:8  Bar => {\n\t$0\n}
        Fill all cases...  Snippet          4:8-4:8  Bar => {$0}\nFoo => {}\nelse => {}
        Foo                Event     => {}  4:8-4:8  Foo => {\n\t$0\n}
        else               Event     => {}  4:8-4:8  else => {\n\t$0\n}"#]]);

    // A filled enum arm is removed while the remaining enum member and else stay available.
    CompletionTest::new(
        "
            enum Mode { First, Second }
            fun main(value: Mode) {
                match (value) {
                    Mode.First => {}
                    <caret>
                }
            }
        ",
    )
    .labels(&["Mode.First", "Mode.Second", "else"])
    .check(expect![[r#"
        label        kind   detail  edit     text
        Mode.Second  Event   => {}  4:8-4:8  Mode.Second => {\n\t$0\n}
        else         Event   => {}  4:8-4:8  else => {\n\t$0\n}"#]]);
}

#[test]
fn excludes_existing_union_and_else_arms() {
    // Only unhandled union variants are offered after several existing arms.
    CompletionTest::new(
        "
            struct Foo {}
            struct Bar {}
            struct Baz {}
            fun main(value: Foo | Bar | Baz) {
                match (value) {
                    Foo => {}
                    Bar => {}
                    <caret>
                }
            }
        ",
    )
    .labels(&["Foo", "Bar", "Baz", "else", "Fill all cases..."])
    .check(expect![[r#"
        label  kind   detail  edit     text
        Baz    Event   => {}  7:8-7:8  Baz => {\n\t$0\n}
        else   Event   => {}  7:8-7:8  else => {\n\t$0\n}"#]]);

    // Once all typed and else arms exist, the provider returns no candidates.
    CompletionTest::new(
        "
            struct Foo {}
            fun main(value: Foo) {
                match (value) {
                    Foo => {}
                    else => {}
                    <caret>
                }
            }
        ",
    )
    .labels(&["Foo", "else", "Fill all cases..."])
    .check(expect!["<none>"]);

    // An existing else arm suppresses another else candidate in a value match.
    CompletionTest::new(
        "
            const FOO = 100
            fun main(value: int) {
                match (value) {
                    FOO => {}
                    else => {}
                    els<caret>
                }
            }
        ",
    )
    .labels(&["else"])
    .check(expect!["<none>"]);
}

#[test]
fn completes_value_and_enum_match_arms() {
    // A non-type match reuses ordinary visible value completion as an arm pattern.
    CompletionTest::new(
        "
            const FOO = 100
            fun main(value: int) {
                match (value) {
                    FO<caret>
                }
            }
        ",
    )
    .labels(&["FOO"])
    .check(expect![[r#"
        label  kind      detail       edit      text
        FOO    Constant  : int = 100  3:8-3:10  FOO$1 => {$0}"#]]);

    // Enum members are qualified and an already filled member is excluded.
    CompletionTest::new(
        "
            enum Color { Red = 10, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Red => {}
                    Blue<caret>
                }
            }
        ",
    )
    .labels(&["Color.Red", "Color.Blue"])
    .check(expect![[r#"
        label       kind   detail  edit      text
        Color.Blue  Event   => {}  4:8-4:12  Color.Blue => {\n\t$0\n}"#]]);
}

#[test]
fn applies_match_arm_and_fill_all_completions() {
    // Selecting one union variant inserts a complete arm and selects its body.
    CompletionTest::new(
        "
            fun main(value: int | slice) {
                match (value) {
                    <caret>
                }
            }
        ",
    )
    .check_applied(
        "int",
        expect![[r#"
            fun main(value: int | slice) {
                match (value) {
                    int => {
            	<caret>
            }
                }
            }"#]],
    );

    // Fill-all inserts every missing typed case followed by else.
    CompletionTest::new(
        "
            fun main(value: int | slice) {
                match (value) {
                    <caret>
                }
            }
        ",
    )
    .check_applied(
        "Fill all cases...",
        expect![[r#"
            fun main(value: int | slice) {
                match (value) {
                    int => {<caret>}
            slice => {}
            else => {}
                }
            }"#]],
    );
}

#[test]
fn applies_value_struct_and_else_match_arms() {
    // A visible value becomes a complete value-pattern arm.
    CompletionTest::new(
        "
            fun main(value: int) {
                match (value) {
                    val<caret>
                }
            }
        ",
    )
    .check_applied(
        "value",
        expect![[r#"
            fun main(value: int) {
                match (value) {
                    value<caret> => {}
                }
            }"#]],
    );

    // A struct union variant is inserted after an existing arm.
    CompletionTest::new(
        "
            struct Foo {}
            struct Bar {}
            fun main(value: Foo | Bar) {
                match (value) {
                    Foo => {}
                    <caret>
                }
            }
        ",
    )
    .check_applied(
        "Bar",
        expect![[r#"
            struct Foo {}
            struct Bar {}
            fun main(value: Foo | Bar) {
                match (value) {
                    Foo => {}
                    Bar => {
            	<caret>
            }
                }
            }"#]],
    );

    // A struct union variant can be inserted before an existing arm.
    CompletionTest::new(
        "
            struct Foo {}
            struct Bar {}
            fun main(value: Foo | Bar) {
                match (value) {
                    <caret>
                    Foo => {}
                }
            }
        ",
    )
    .check_applied(
        "Bar",
        expect![[r#"
            struct Foo {}
            struct Bar {}
            fun main(value: Foo | Bar) {
                match (value) {
                    Bar<caret>
                    Foo => {}
                }
            }"#]],
    );

    // Else completion works for a type match.
    CompletionTest::new(
        "
            fun main(value: int | slice) {
                match (value) {
                    els<caret>
                }
            }
        ",
    )
    .check_applied(
        "else",
        expect![[r#"
            fun main(value: int | slice) {
                match (value) {
                    else => {
            	<caret>
            }
                }
            }"#]],
    );

    // Else completion also works for a value match.
    CompletionTest::new(
        "
            fun main() {
                match (10) {
                    els<caret>
                }
            }
        ",
    )
    .check_applied(
        "else",
        expect![[r#"
            fun main() {
                match (10) {
                    else => {
            	<caret>
            }
                }
            }"#]],
    );

    // Fill-all works for a single struct type as well as a union.
    CompletionTest::new(
        "
            struct Foo {}
            fun main(value: Foo) {
                match (value) {
                    Fill<caret>
                }
            }
        ",
    )
    .check_applied(
        "Fill all cases...",
        expect![[r#"
            struct Foo {}
            fun main(value: Foo) {
                match (value) {
                    Foo => {<caret>}
            else => {}
                }
            }"#]],
    );

    // A value named Fill remains an ordinary value arm and does not trigger fill-all.
    CompletionTest::new(
        "
            const Fill = 100
            fun main(value: int) {
                match (value) {
                    Fil<caret>
                }
            }
        ",
    )
    .check_applied(
        "Fill",
        expect![[r#"
            const Fill = 100
            fun main(value: int) {
                match (value) {
                    Fill<caret> => {}
                }
            }"#]],
    );
}

#[test]
fn applies_enum_match_arms_in_every_qualified_form() {
    // A bare enum-member prefix expands to a qualified arm.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Red<caret>
                }
            }
        ",
    )
    .check_applied(
        "Color.Red",
        expect![[r#"
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Red => {
            	<caret>
            }
                }
            }"#]],
    );

    // An already handled enum member is skipped when inserting the next member.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Red => {}
                    Blue<caret>
                }
            }
        ",
    )
    .check_applied(
        "Color.Blue",
        expect![[r#"
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Red => {}
                    Color.Blue => {
            	<caret>
            }
                }
            }"#]],
    );

    // Completion after Color. inserts only the remaining member name segment.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.<caret>
                }
            }
        ",
    )
    .trigger_character(".")
    .check_applied(
        "Color.Blue",
        expect![[r#"
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Color.Blue => {
            	<caret>
            }
                }
            }"#]],
    );

    // Else completion remains available after a filled enum member.
    CompletionTest::new(
        "
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Red => {}
                    els<caret>
                }
            }
        ",
    )
    .check_applied(
        "else",
        expect![[r#"
            enum Color { Red, Blue }
            fun main(color: Color) {
                match (color) {
                    Color.Red => {}
                    else => {
            	<caret>
            }
                }
            }"#]],
    );
}
