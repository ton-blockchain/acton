mod common;

use crate::common::check;
use expect_test::expect;

#[test]
fn test_comment_only_file_is_preserved() {
    check(
        "// A module placeholder\n// More context\n",
        expect![[r"
            // A module placeholder
            // More context"]],
    );
    check(
        "/* A module placeholder */",
        expect!["/* A module placeholder */"],
    );
}

#[test]
fn test_comments_between_control_flow_branches_are_preserved() {
    check(
        r"
            fun test() {
                if (ready) {
                    work();
                } /* alternative */ else {
                    fallback();
                }
                try {
                    work();
                } // handler
                catch (error) {
                    fallback();
                }
                do {
                    work();
                } // condition
                while (ready);
            }
        ",
        expect![[r"
            fun test() {
                if (ready) {
                    work();
                } /* alternative */ else {
                    fallback();
                }
                try {
                    work();
                } // handler
                catch (error) {
                    fallback();
                }
                do {
                    work();
                } // condition
                while (ready);
            }"]],
    );
}

#[test]
fn test_comments_inside_empty_lists_are_preserved() {
    check(
        r"
            fun test(/* parameters */) {
                foo(/* arguments */);
                val a = Foo { /* fields */ };
                val b = [ /* tuple */ ];
                val c = ( /* unit */ );
                val d = match (value) { /* cases */ };
                val callback = fun(/* lambda parameters */) {};
            }
        ",
        expect![[r"
            fun test(
                /* parameters */
            ) {
                foo(
                    /* arguments */
                );
                val a = Foo {
                    /* fields */
                };
                val b = [
                    /* tuple */
                ];
                val c = (
                    /* unit */
                );
                val d = match (value) {
                    /* cases */
                };
                val callback = fun(
                    /* lambda parameters */
                ) {};
            }"]],
    );
}

#[test]
fn test_comments_inside_empty_type_and_annotation_lists_are_preserved() {
    check(
        r"
            type Unit = (/* tensor type */);
            @custom(/* annotation */)
            fun test</* type parameters */>() {}
        ",
        expect![[r"
            type Unit = (
                /* tensor type */
            )

            @custom(
                /* annotation */
            )
            fun test<
                /* type parameters */
            >() {}"]],
    );
}

#[test]
fn test_comments_in_single_generic_arguments_preserve_closing_brackets() {
    check(
        r"
            type Values = array<
                int // scalar
            >;
            fun test() {
                identity<
                    int // scalar
                >(42);
                value.convert<
                    // Result type
                    int
                >();
            }
        ",
        expect![[r"
            type Values = array<
                int, // scalar
            >

            fun test() {
                identity<
                    int, // scalar
                >(42);
                value.convert<
                    // Result type
                    int,
                >();
            }"]],
    );
}

#[test]
fn test_comments_on_tuple_elements_are_printed_once() {
    check(
        r#"
            fun test() {
                val pair = (true, // enabled
                    false);
                val tuple = ["hello", // greeting
                    null];
                val refs = (value, // selected
                    42);
            }
        "#,
        expect![[r#"
            fun test() {
                val pair = (
                    true, // enabled
                    false,
                );
                val tuple = [
                    "hello", // greeting
                    null,
                ];
                val refs = (
                    value, // selected
                    42,
                );
            }"#]],
    );
}

#[test]
fn test_comments_on_type_list_elements_are_printed_once() {
    check(
        r"
            type Pair = (int, // first
                bool);
            type Tuple = [int, // first
                bool];
            type Map = map<int, // key
                slice>;
            fun test() {
                convert<int, // source
                    slice>();
            }
        ",
        expect![[r"
            type Pair = (
                int, // first
                bool,
            )

            type Tuple = [
                int, // first
                bool,
            ]

            type Map = map<
                int, // key
                slice,
            >

            fun test() {
                convert<
                    int, // source
                    slice,
                >();
            }"]],
    );
}

#[test]
fn test_comments_on_annotation_arguments_are_printed_once() {
    check(
        r#"
            @custom("hello", // greeting
                true)
            fun test() {}
        "#,
        expect![[r#"
            @custom(
                "hello", // greeting
                true,
            )
            fun test() {}"#]],
    );
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
