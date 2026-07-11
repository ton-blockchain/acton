use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_throw_and_assert_snippets_in_statements() {
    // Statement positions expose both error-control snippets.
    CompletionTest::new("fun main() { th<caret> }")
        .labels(&["throw", "assert"])
        .check(expect![[r#"
            label   kind     detail                   edit       text
            throw   Keyword   EXIT_CODE               0:13-0:15  throw ${1:5};$0
            assert  Keyword   (cond) throw EXIT_CODE  0:13-0:15  assert (${1:cond}) throw ${2:5};$0"#]]);
}

#[test]
fn applies_throw_and_assert_snippets() {
    // Throw completion expands an editable exit code.
    CompletionTest::new("fun main() { thro<caret> }")
        .check_applied("throw", expect!["fun main() { throw 5<caret>; }"]);

    // Assert completion expands condition and exit-code tab stops.
    CompletionTest::new("fun main() { asse<caret> }").check_applied(
        "assert",
        expect!["fun main() { assert (cond<caret>) throw 5; }"],
    );
}
