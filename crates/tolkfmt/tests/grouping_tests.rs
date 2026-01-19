mod common;

use crate::common::check;
use expect_test::expect;

#[test]
fn test_comment_grouping_for_declarations() {
    check(
        "
            /// doc comment
            /// with text
            fun main() {}

            // comment 3

            /// comment 1
            /// comment 2
            fun bar() {}
            // comment 4
            ",
        expect![[r#"
            /// doc comment
            /// with text
            fun main() {}

            // comment 3

            /// comment 1
            /// comment 2
            fun bar() {}
            // comment 4"#]],
    );
}

#[test]
fn test_comment_grouping_for_statements() {
    check(
        "
            fun main() {
                // comment 1
                // comment 2
                val a = 100;

                // comment 3
                // comment 4

                val b = 200;
            }
            ",
        expect![[r#"
            fun main() {
                // comment 1
                // comment 2
                val a = 100;

                // comment 3
                // comment 4

                val b = 200;
            }"#]],
    );
}

#[test]
fn test_inline_comment_grouping() {
    check(
        "
            fun main() {
                val a = 100; /* comment 1 *//* comment 2 */
            }
            ",
        expect![[r#"
            fun main() {
                val a = 100; /* comment 1 */ /* comment 2 */
            }"#]],
    );
}

#[test]
fn test_trailing_comment_grouping() {
    check(
        "
            fun main() {
                val a = 100;
                // comment 1
                // comment 2
            }
            ",
        expect![[r#"
            fun main() {
                val a = 100;
                // comment 1
                // comment 2
            }"#]],
    );
}

#[test]
fn test_comments_inline_empty_function() {
    check(
        "
            fun main() {
                // comment 1
                // comment 2
            }
            ",
        expect![[r#"
            fun main() {
                // comment 1
                // comment 2
            }"#]],
    );
}

#[test]
fn test_comments_inline_empty_block_statement() {
    check(
        "
            fun main() {
                {
                    // comment 1
                    // comment 2
                }
            }
            ",
        expect![[r#"
            fun main() {
                {
                    // comment 1
                    // comment 2
                }
            }"#]],
    );
}
