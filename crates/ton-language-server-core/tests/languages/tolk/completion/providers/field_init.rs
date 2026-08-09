use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_struct_to_cell_for_cell_struct_fields_only() {
    // A Cell<Struct> field value offers Struct{}.toCell().
    CompletionTest::new(
        r#"
            struct Inner { value: int }
            struct Outer { inner: Cell<Inner> }
            fun main() {
                val outer = Outer { inner: <caret> };
            }
        "#,
    )
    .labels(&["Inner {}.toCell()"])
    .check(expect![[r#"
        label              kind     detail  edit       text
        Inner {}.toCell()  Snippet          3:31-3:31  Inner {$0}.toCell()"#]]);

    // Non-Cell fields do not receive the conversion snippet.
    CompletionTest::new(
        "
            struct Outer { value: int }
            fun main() { val outer = Outer { value: <caret> }; }
        ",
    )
    .labels(&["Inner {}.toCell()"])
    .check(expect!["<none>"]);
}

#[test]
fn applies_struct_to_cell_field_initializer() {
    // Applying the conversion snippet puts the caret inside the nested struct literal.
    CompletionTest::new(
        "
            struct Inner { value: int }
            struct Outer { inner: Cell<Inner> }
            fun main() { val outer = Outer { inner: <caret> }; }
        ",
    )
    .check_applied(
        "Inner {}.toCell()",
        expect![[r#"
            struct Inner { value: int }
            struct Outer { inner: Cell<Inner> }
            fun main() { val outer = Outer { inner: Inner {<caret>}.toCell() }; }"#]],
    );
}
