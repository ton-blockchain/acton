use expect_test::expect;

use super::support::CodeActionTest;

#[test]
fn applies_linter_fix_for_the_selected_diagnostic() {
    CodeActionTest::new("fun <caret>BadName() {}")
        .check_applied("rename to camelCase: badName", expect!["fun badName() {}"]);
}

#[test]
fn does_not_offer_linter_fixes_outside_the_requested_range() {
    CodeActionTest::new("fun BadName() {}\nfun <caret>goodName() {}")
        .check_titles(expect!["<none>"]);
}
