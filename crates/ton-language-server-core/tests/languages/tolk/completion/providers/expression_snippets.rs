use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_match_in_expression_context() {
    // Match is available as an expression initializer.
    CompletionTest::new("fun main() { val result = mat<caret>; }")
        .labels(&["match"])
        .check(expect![[r#"
            label  kind     detail  edit       text
            match  Snippet          0:26-0:29  match (${1:condition}) {\n\t$0\n}"#]]);
}

#[test]
fn applies_match_expression_snippet() {
    // Selecting match expands the expression and activates its condition tab stop.
    CompletionTest::new("fun main() { val result = mat<caret>; }").check_applied(
        "match",
        expect![[r#"
                fun main() { val result = match (condition<caret>) {

                }; }"#]],
    );
}

#[test]
fn applies_match_snippet_as_a_statement() {
    // Match completion is also applicable directly in a statement position.
    CompletionTest::new("fun main() { mat<caret> }").check_applied(
        "match",
        expect![[r#"
            fun main() { match (condition<caret>) {

            } }"#]],
    );
}
