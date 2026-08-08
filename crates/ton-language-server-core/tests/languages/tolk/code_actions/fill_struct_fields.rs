use super::support::CodeActionTest;
use expect_test::expect;

#[test]
fn fills_all_fields_and_preserves_indentation() {
    // A one-line empty literal expands into a multiline initializer.
    CodeActionTest::new(
        "
            struct Foo { value: int }
            fun foo() {
                Foo {<caret>};
            }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Foo { value: int }
            fun foo() {
                Foo {
                    value: 0,
                };
            }"]],
    );

    // Deeply nested literals use the indentation of their own line.
    CodeActionTest::new(
        "
            struct Foo { value: int }
            fun foo() {
                {
                    {
                        Foo {<caret>};
                    }
                }
            }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Foo { value: int }
            fun foo() {
                {
                    {
                        Foo {
                            value: 0,
                        };
                    }
                }
            }"]],
    );

    // An already multiline empty body does not gain an extra blank line.
    CodeActionTest::new(
        "
            struct Foo { value: int }
            fun foo() {
                Foo {<caret>
                };
            }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Foo { value: int }
            fun foo() {
                Foo {
                    value: 0,
                };
            }"]],
    );
}

#[test]
fn fills_alias_nullable_and_defaulted_fields() {
    // Aliases use the default value of their underlying type.
    CodeActionTest::new(
        "
            type Int = int
            struct Foo { value: Int }
            fun foo() { Foo {<caret>}; }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            type Int = int
            struct Foo { value: Int }
            fun foo() { Foo {
                value: 0,
            }; }"]],
    );

    // A union without null uses the first declared alternative.
    CodeActionTest::new(
        "
            struct Foo { value: int | bool }
            fun foo() { Foo {<caret>}; }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Foo { value: int | bool }
            fun foo() { Foo {
                value: 0,
            }; }"]],
    );

    // Nullable fields prefer null, while explicit defaults are copied verbatim.
    CodeActionTest::new(
        "
            struct Foo {
                value: int = 10
                other: slice
                optional: slice?
            }
            fun foo() { Foo {<caret>}; }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Foo {
                value: int = 10
                other: slice
                optional: slice?
            }
            fun foo() { Foo {
                value: 10,
                other: createEmptySlice(),
                optional: null,
            }; }"]],
    );
}

#[test]
fn fills_defaults_for_all_supported_type_shapes() {
    CodeActionTest::new(
        "
            struct Other {}
            struct (0x100) Message {}
            enum Color { Red, Blue }
            struct Foo {
                opt: slice?
                integer: int
                coinsValue: coins
                int32Value: int32
                bitsValue: bits32
                bytesValue: bytes1000
                flag: bool
                destination: address
                output: builder
                input: slice
                data: cell
                other: Other
                message: Message
                color: Color
                tensorValue: (int, slice, [int, slice?, builder])
                tupleValue: [int, slice]
                stringValue: string
                unknownValue: SomeUnknownType
            }
            fun foo() { Foo {<caret>}; }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r#"
            struct Other {}
            struct (0x100) Message {}
            enum Color { Red, Blue }
            struct Foo {
                opt: slice?
                integer: int
                coinsValue: coins
                int32Value: int32
                bitsValue: bits32
                bytesValue: bytes1000
                flag: bool
                destination: address
                output: builder
                input: slice
                data: cell
                other: Other
                message: Message
                color: Color
                tensorValue: (int, slice, [int, slice?, builder])
                tupleValue: [int, slice]
                stringValue: string
                unknownValue: SomeUnknownType
            }
            fun foo() { Foo {
                opt: null,
                integer: 0,
                coinsValue: ton("0.1"),
                int32Value: 0,
                bitsValue: createEmptySlice(),
                bytesValue: createEmptySlice(),
                flag: false,
                destination: address(""),
                output: beginCell(),
                input: createEmptySlice(),
                data: createEmptyCell(),
                other: Other {},
                message: Message {},
                color: Red,
                tensorValue: (0, createEmptySlice(), [0, null, beginCell()]),
                tupleValue: [0, createEmptySlice()],
                stringValue: null,
                unknownValue: null,
            }; }"#]],
    );
}

#[test]
fn fills_only_required_fields() {
    // Fields without declaration defaults are required.
    CodeActionTest::new(
        "
            struct Foo {
                value: int = 0
                other: int
                age: int?
                data: cell? = null
            }
            fun foo() { Foo {<caret>}; }
        ",
    )
    .check_applied(
        "Fill required fields...",
        expect![[r"
            struct Foo {
                value: int = 0
                other: int
                age: int?
                data: cell? = null
            }
            fun foo() { Foo {
                other: 0,
                age: null,
            }; }"]],
    );

    // No required-fields action is offered when every field has a default.
    CodeActionTest::new(
        "
            struct Foo { value: int = 0 }
            fun foo() { Foo {<caret>}; }
        ",
    )
    .check_titles(expect!["Fill all fields..."]);
}

#[test]
fn supports_expected_short_literals_and_cell_wrappers() {
    // A short literal obtains its struct type from the call parameter.
    CodeActionTest::new(
        "
            struct Options {
                bounce: bool
                value: coins
            }
            fun send(options: Options) {}
            fun main() { send({<caret>}); }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r#"
            struct Options {
                bounce: bool
                value: coins
            }
            fun send(options: Options) {}
            fun main() { send({
                bounce: false,
                value: ton("0.1"),
            }); }"#]],
    );

    // Expected aliases and generic structs are unwrapped to their struct declaration.
    CodeActionTest::new(
        "
            struct Box<T> { value: T }
            type IntBox = Box<int>
            fun consume(box: IntBox) {}
            fun main() { consume({<caret>}); }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Box<T> { value: T }
            type IntBox = Box<int>
            fun consume(box: IntBox) {}
            fun main() { consume({
                value: null,
            }); }"]],
    );

    // Cell<Struct> fields initialize the inner struct and serialize it.
    CodeActionTest::new(
        "
            struct Inner { bar: int }
            struct Data { foo: Cell<Inner> }
            fun main() { Data {<caret>} }
        ",
    )
    .check_applied(
        "Fill all fields...",
        expect![[r"
            struct Inner { bar: int }
            struct Data { foo: Cell<Inner> }
            fun main() { Data {
                foo: Inner {}.toCell(),
            } }"]],
    );
}

#[test]
fn is_unavailable_for_empty_structs_and_non_empty_literals() {
    CodeActionTest::new("struct Foo {} fun main() { Foo {<caret>} }")
        .check_titles(expect!["<none>"]);

    CodeActionTest::new(
        "
            struct Foo { value: int }
            fun main() { Foo { value: 10, <caret> } }
        ",
    )
    .check_titles(expect!["<none>"]);
}
