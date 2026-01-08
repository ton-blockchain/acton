use tolkfmt::format_source;
use expect_test::{expect, Expect};

fn check(code: &str, expect: Expect) {
    check_with_width(code, expect, 80)
}

fn check_with_width(code: &str, expect: Expect, width: usize) {
    // unsafe { std::env::set_var("UPDATE_EXPECT", "1") }

    let res = format_source(code, width).unwrap();

    let res = res
        .lines()
        .map(|l| if l.trim().is_empty() { "" } else { l })
        .collect::<Vec<_>>()
        .join("\n");

    expect.assert_eq(&res);
}

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
