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
        expect![[r"
            /// doc comment
            /// with text
            fun main() {}

            // comment 3

            /// comment 1
            /// comment 2
            fun bar() {}
            // comment 4"]],
    );
}

#[test]
fn test_comments_around_function_annotations_are_preserved() {
    check(
        r#"
            /// Stores a little-endian signed 32-bit integer (TL `int` on the wire) via STILE4.
            @pure
            // check-disable-next-line asm-function-missing-safety-comment
            fun builder.storeI32le(mutate self, x: int): self
                asm(x self) "STILE4"
            "#,
        expect![[r#"
            /// Stores a little-endian signed 32-bit integer (TL `int` on the wire) via STILE4.
            @pure
            // check-disable-next-line asm-function-missing-safety-comment
            fun builder.storeI32le(mutate self, x: int): self
                asm(x self) "STILE4""#]],
    );
}

#[test]
fn test_comment_group_after_function_annotations_is_preserved() {
    check(
        r#"
            @pure
            // SAFETY: the assembly only reads its arguments
            // check-disable-next-line asm-function-missing-safety-comment
            fun addOne(x: int): int asm "INC"
            "#,
        expect![[r#"
            @pure
            // SAFETY: the assembly only reads its arguments
            // check-disable-next-line asm-function-missing-safety-comment
            fun addOne(x: int): int
                asm "INC""#]],
    );
}

#[test]
fn test_comment_between_annotations_and_method_is_preserved() {
    check(
        r"
            @pure
            // method comment
            fun int.abs(): int { return self; }
            ",
        expect![[r"
            @pure
            // method comment
            fun int.abs(): int {
                return self;
            }"]],
    );
}

#[test]
fn test_comment_between_annotations_and_get_method_is_preserved() {
    check(
        r"
            @pure
            // get method comment
            get value(): int { return 42; }
            ",
        expect![[r"
            @pure
            // get method comment
            get fun value(): int {
                return 42;
            }"]],
    );
}

#[test]
fn test_comment_between_annotations_and_global_is_preserved() {
    check(
        r"
            @deprecated
            // global comment
            global counter: int
            ",
        expect![[r"
            @deprecated
            // global comment
            global counter: int"]],
    );
}

#[test]
fn test_comment_between_annotations_and_constant_is_preserved() {
    check(
        r"
            @deprecated
            // constant comment
            const ANSWER = 42
            ",
        expect![[r"
            @deprecated
            // constant comment
            const ANSWER = 42"]],
    );
}

#[test]
fn test_comment_between_annotations_and_type_alias_is_preserved() {
    check(
        r"
            @deprecated
            // type alias comment
            type Amount = int
            ",
        expect![[r"
            @deprecated
            // type alias comment
            type Amount = int"]],
    );
}

#[test]
fn test_comment_between_annotations_and_struct_is_preserved() {
    check(
        r"
            @deprecated
            // struct comment
            struct (0x10) Message { value: int }
            ",
        expect![[r"
            @deprecated
            // struct comment
            struct (0x10) Message {
                value: int
            }"]],
    );
}

#[test]
fn test_comment_between_annotations_and_struct_field_is_preserved() {
    check(
        r"
            struct Message {
                @deprecated
                // field comment
                readonly value: int
            }
            ",
        expect![[r"
            struct Message {
                @deprecated
                // field comment
                readonly value: int
            }"]],
    );
}

#[test]
fn test_comment_between_annotations_and_enum_is_preserved() {
    check(
        r"
            @deprecated
            // enum comment
            enum Mode { First, Second }
            ",
        expect![[r"
            @deprecated
            // enum comment
            enum Mode {
                First
                Second
            }"]],
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
        expect![[r"
            fun main() {
                // comment 1
                // comment 2
                val a = 100;

                // comment 3
                // comment 4

                val b = 200;
            }"]],
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
        expect![[r"
            fun main() {
                val a = 100; /* comment 1 */ /* comment 2 */
            }"]],
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
        expect![[r"
            fun main() {
                val a = 100;
                // comment 1
                // comment 2
            }"]],
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
        expect![[r"
            fun main() {
                // comment 1
                // comment 2
            }"]],
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
        expect![[r"
            fun main() {
                {
                    // comment 1
                    // comment 2
                }
            }"]],
    );
}
