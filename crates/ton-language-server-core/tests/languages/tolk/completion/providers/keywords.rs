use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_keywords_in_expression_contexts() {
    // A general expression position exposes booleans and operator-like keywords.
    CompletionTest::new("fun main(value: int) { val flag = tr<caret>; }")
        .labels(&["true", "false", "lazy", "as", "is", "mutate"])
        .check(expect![[r#"
            label   kind     detail  edit       text
            true    Keyword          0:34-0:36  true
            as      Keyword          0:34-0:36  as 
            false   Keyword          0:34-0:36  false
            is      Keyword          0:34-0:36  is 
            lazy    Keyword          0:34-0:36  lazy 
            mutate  Keyword          0:34-0:36  mutate "#]]);

    // Keywords remain available after an existing expression operand.
    CompletionTest::new("fun main(value: int) { val flag = value as<caret>; }")
        .labels(&["as", "is"])
        .check(expect![[r#"
            label  kind     detail  edit       text
            as     Keyword          0:40-0:42  as 
            is     Keyword          0:40-0:42  is "#]]);

    // A struct field-name slot is owned by field completion, not keyword completion.
    CompletionTest::new(
        "
            struct Foo { value: int }
            fun main() { val foo = Foo { la<caret> }; }
        ",
    )
    .labels(&["lazy"])
    .check(expect!["<none>"]);
}

#[test]
fn applies_keyword_completion_without_duplicating_the_prefix() {
    // A keyword with trailing space replaces the typed prefix exactly once.
    CompletionTest::new("fun main(value: int) { val result = laz<caret>value; }").check_applied(
        "lazy",
        expect!["fun main(value: int) { val result = lazy <caret>; }"],
    );

    // As replaces its typed prefix and leaves one trailing space.
    CompletionTest::new("fun main(value: int) { value as<caret> }")
        .check_applied("as", expect!["fun main(value: int) { value as <caret> }"]);

    // Is replaces its typed prefix and leaves one trailing space.
    CompletionTest::new("fun main(value: int) { value is<caret> }")
        .check_applied("is", expect!["fun main(value: int) { value is <caret> }"]);

    // Mutate replaces its typed prefix and leaves one trailing space.
    CompletionTest::new("fun main(value: int) { mutate<caret> value }").check_applied(
        "mutate",
        expect!["fun main(value: int) { mutate <caret> value }"],
    );

    // Fuzzy selection of as for an asm prefix still replaces the entire prefix once.
    CompletionTest::new("fun main(value: int) { value asm<caret> }")
        .check_applied("as", expect!["fun main(value: int) { value as <caret> }"]);

    // A boolean keyword is inserted without an extra trailing space.
    CompletionTest::new("fun main() { val result = tru<caret>; }")
        .check_applied("true", expect!["fun main() { val result = true<caret>; }"]);
}
