use super::providers::support::CompletionTest;
use expect_test::expect;

fn check_disabled(source: &str) {
    CompletionTest::new(source).check(expect!["<none>"]);
}

#[test]
fn disables_completion_in_line_comments() {
    check_disabled("// com<caret>");
    check_disabled("fun main() { // com<caret>\n}");
    check_disabled("fun main() {} // com<caret>");
}

#[test]
fn disables_completion_in_documentation_line_comments() {
    check_disabled("/// doc<caret>\nfun main() {}");
    check_disabled("fun main() {\n    /// doc<caret>\n}");
}

#[test]
fn disables_completion_in_block_comments() {
    check_disabled("/* comment <caret> text */\nfun main() {}");
    check_disabled("fun main() { /* comment <caret> text */ }");
    check_disabled("fun main() {} /* comment <caret> text */");
}

#[test]
fn disables_completion_in_documentation_block_comments() {
    check_disabled("/** doc<caret>umentation */\nfun main() {}");
    check_disabled("fun main() { /** doc<caret>umentation */ }");
}

#[test]
fn disables_completion_on_later_lines_of_block_comments() {
    check_disabled(
        r#"
            /* first line
             * second <caret>line
             * third line */
            fun main() {}
        "#,
    );
}

#[test]
fn disables_completion_in_comments_containing_unicode() {
    check_disabled("// комментарий <caret>здесь");
    check_disabled("/* комментарий <caret>здесь */");
}

#[test]
fn disables_completion_immediately_after_line_comment_text() {
    check_disabled("//<caret>");
    check_disabled("// comment<caret>");
}

#[test]
fn disables_triggered_completion_in_comments() {
    CompletionTest::new("// value.<caret>")
        .trigger_character(".")
        .check(expect!["<none>"]);
    CompletionTest::new("/* value.<caret> */")
        .trigger_character(".")
        .check(expect!["<none>"]);
}

#[test]
fn keeps_completion_before_a_comment() {
    CompletionTest::new("fun main() { val value = <caret>/* comment */; }")
        .labels(&["true", "false"])
        .check(expect![[r#"
            label  kind     detail  edit       text
            false  Keyword          0:25-0:25  false
            true   Keyword          0:25-0:25  true"#]]);
}

#[test]
fn keeps_completion_after_a_closed_block_comment() {
    CompletionTest::new("fun main() { val value = /* comment */ <caret>; }")
        .labels(&["true", "false"])
        .check(expect![[r#"
            label  kind     detail  edit       text
            false  Keyword          0:39-0:39  false
            true   Keyword          0:39-0:39  true"#]]);
}

#[test]
fn keeps_completion_on_the_line_after_a_comment() {
    CompletionTest::new("// comment\nfun main() { val value = <caret>; }")
        .labels(&["true", "false"])
        .check(expect![[r#"
            label  kind     detail  edit       text
            false  Keyword          1:25-1:25  false
            true   Keyword          1:25-1:25  true"#]]);
}
