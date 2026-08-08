use super::support::CodeActionTest;
use expect_test::expect;

#[test]
fn imports_supported_declarations_from_other_files() {
    // Functions are imported before the first declaration.
    CodeActionTest::new(
        "
            fun main() {
                <caret>someFunction();
            }
        ",
    )
    .file("file:///fixture/other.tolk", "fun someFunction() {}")
    .check_applied(
        "Import symbol from other file",
        expect![[r#"
            import "other"

            fun main() {
                someFunction();
            }"#]],
    );

    // Global variables use the same import path calculation.
    CodeActionTest::new("fun main() { <caret>globalVar; }")
        .file("file:///fixture/other.tolk", "global globalVar: int;")
        .check_applied(
            "Import symbol from other file",
            expect![[r#"
                import "other"

                fun main() { globalVar; }"#]],
        );

    // Type aliases can be imported from a type position.
    CodeActionTest::new("fun main(): <caret>Int {}")
        .file("file:///fixture/other.tolk", "type Int = int;")
        .check_applied(
            "Import symbol from other file",
            expect![[r#"
                import "other"

                fun main(): Int {}"#]],
        );

    // Constants and structs are both importable top-level symbols.
    CodeActionTest::new("fun main() { <caret>FOO; }")
        .file("file:///fixture/other.tolk", "const FOO: int = 100;")
        .check_applied(
            "Import symbol from other file",
            expect![[r#"
                import "other"

                fun main() { FOO; }"#]],
        );

    CodeActionTest::new("fun main() { <caret>Foo {}; }")
        .file("file:///fixture/other.tolk", "struct Foo {}")
        .check_applied(
            "Import symbol from other file",
            expect![[r#"
                import "other"

                fun main() { Foo {}; }"#]],
        );
}

#[test]
fn inserts_after_existing_imports_and_version_directive() {
    // New imports remain in the leading import block.
    CodeActionTest::new(
        r#"
            import "some";
            fun main() { <caret>Foo {}; }
        "#,
    )
    .file("file:///fixture/other.tolk", "struct Foo {}")
    .check_applied(
        "Import symbol from other file",
        expect![[r#"
            import "some";
            import "other"

            fun main() { Foo {}; }"#]],
    );

    // A version directive stays before the generated import with one blank line per section.
    CodeActionTest::new(
        "
            tolk 1.0
            fun main() { <caret>Foo {}; }
        ",
    )
    .file("file:///fixture/other.tolk", "struct Foo {}")
    .check_applied(
        "Import symbol from other file",
        expect![[r#"
            tolk 1.0

            import "other"

            fun main() { Foo {}; }"#]],
    );
}

#[test]
fn is_unavailable_for_existing_stdlib_and_ambiguous_symbols() {
    // A same-named local is already visible and must not trigger an import.
    CodeActionTest::new("fun main(<caret>bar: int) { bar; }")
        .file("file:///fixture/other.tolk", "fun bar() {}")
        .check_titles(expect!["<none>"]);

    // Already imported files do not produce a duplicate import.
    CodeActionTest::new(
        r#"
            import "other"
            fun main() { <caret>bar(); }
        "#,
    )
    .file("file:///fixture/other.tolk", "fun bar() {}")
    .check_titles(expect!["<none>"]);

    // Imports with the explicit extension resolve to the same target.
    CodeActionTest::new(
        r#"
            import "other.tolk"
            fun main() { <caret>bar(); }
        "#,
    )
    .file("file:///fixture/other.tolk", "fun bar() {}")
    .check_titles(expect!["<none>"]);

    // Standard-library symbols are visible implicitly.
    CodeActionTest::new("fun main() { <caret>minMax(); }").check_titles(expect!["<none>"]);

    // Ambiguous names cannot choose a deterministic import target.
    CodeActionTest::new("fun main() { <caret>bar(); }")
        .file("file:///fixture/other.tolk", "fun bar() {}")
        .file("file:///fixture/other2.tolk", "fun bar() {}")
        .check_titles(expect!["<none>"]);
}
