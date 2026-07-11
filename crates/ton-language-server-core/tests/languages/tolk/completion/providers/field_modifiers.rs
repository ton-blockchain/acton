use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_only_missing_field_modifiers() {
    // A field without modifiers offers both private and readonly.
    CompletionTest::new("struct Foo { <caret>foo: int }")
        .labels(&["private", "readonly"])
        .check(expect![[r#"
            label     kind     detail  edit       text
            private   Keyword          0:13-0:13  private\s
            readonly  Keyword          0:13-0:13  readonly\s"#]]);

    // An existing private modifier leaves only readonly available.
    CompletionTest::new("struct Foo { private <caret>foo: int }")
        .labels(&["private", "readonly"])
        .check(expect![[r#"
            label     kind     detail  edit       text
            readonly  Keyword          0:21-0:21  readonly\s"#]]);

    // An existing readonly modifier leaves only private available.
    CompletionTest::new("struct Foo { readonly <caret>foo: int }")
        .labels(&["private", "readonly"])
        .check(expect![[r#"
            label    kind     detail  edit       text
            private  Keyword          0:22-0:22  private\s"#]]);

    // Replacing a partially typed modifier still exposes both candidates.
    CompletionTest::new("struct Foo { <caret>readonly foo: int }")
        .labels(&["private", "readonly"])
        .check(expect![[r#"
            label     kind     detail  edit       text
            private   Keyword          0:13-0:13  private\s
            readonly  Keyword          0:13-0:13  readonly\s"#]]);

    // A field with both modifiers has no remaining modifier completion.
    CompletionTest::new("struct Foo { private readonly <caret>foo: int }")
        .labels(&["private", "readonly"])
        .check(expect!["<none>"]);

    // A new line after another field is a valid modifier position.
    CompletionTest::new(
        "
            struct Foo {
                foo: int
                <caret>
            }
        ",
    )
    .labels(&["private", "readonly"])
    .check(expect![[r#"
        label     kind     detail  edit     text
        private   Keyword          2:4-2:4  private\s
        readonly  Keyword          2:4-2:4  readonly\s"#]]);
}

#[test]
fn applies_field_modifier_completion() {
    // Applying private to an unmodified field preserves the field declaration.
    CompletionTest::new("struct Foo { <caret>foo: int }")
        .check_applied("private", expect!["struct Foo { private <caret>foo: int }"]);

    // Applying readonly keeps the field name and inserts the required separating space.
    CompletionTest::new("struct Foo { rea<caret>foo: int }").check_applied(
        "readonly",
        expect!["struct Foo { readonly <caret>foo: int }"],
    );

    // Applying readonly after private preserves the existing modifier.
    CompletionTest::new("struct Foo { private <caret>foo: int }").check_applied(
        "readonly",
        expect!["struct Foo { private readonly <caret>foo: int }"],
    );
}
