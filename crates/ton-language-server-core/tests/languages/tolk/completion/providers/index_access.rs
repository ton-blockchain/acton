use super::support::CompletionTest;
use expect_test::expect;

#[test]
fn completes_tuple_and_tensor_indexes_but_not_empty_tuple() {
    // Tuple members are exposed as numeric indexes after a dot.
    CompletionTest::new("fun main() { val value = (1, 2, 3); value.<caret>; }")
        .labels(&["0", "1", "2"])
        .trigger_character(".")
        .check(expect![[r#"
            label  kind   detail  edit       text
            0      Field          0:42-0:42  0
            1      Field          0:42-0:42  1
            2      Field          0:42-0:42  2"#]]);

    // A typed tuple exposes the same numeric indexes as an inferred tensor.
    CompletionTest::new("fun main() { val value: [int, int, int] = [1, 2, 3]; value.<caret>; }")
        .labels(&["0", "1", "2"])
        .trigger_character(".")
        .check(expect![[r#"
        label  kind   detail  edit       text
        0      Field          0:59-0:59  0
        1      Field          0:59-0:59  1
        2      Field          0:59-0:59  2"#]]);

    // An empty tuple has no numeric index completion.
    CompletionTest::new("fun main() { val value = []; value.<caret>; }")
        .labels(&["0"])
        .trigger_character(".")
        .check(expect!["<none>"]);
}

#[test]
fn applies_tuple_index_completion_after_the_dot() {
    // Applying an index item leaves the receiver and dot unchanged.
    CompletionTest::new("fun main() { val value = (1, 2); value.<caret>; }")
        .trigger_character(".")
        .check_applied(
            "1",
            expect!["fun main() { val value = (1, 2); value.1<caret>; }"],
        );
}
