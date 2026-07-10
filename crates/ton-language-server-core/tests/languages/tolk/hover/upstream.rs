#![allow(clippy::needless_raw_string_hashes)]

use super::case_tolk_hover;
use expect_test::expect;

#[test]
fn annotations_01_annotations_documentation() {
    // Checks Annotations documentation; ported from annotations.test.
    case_tolk_hover(
        r#"
            @<caret>deprecated("")
            @<caret>inline
            @<caret>inline_ref
            @<caret>noinline
            @<caret>on_bounced_policy("manual")
            @<caret>overflow1023_policy("suppress")
            @<caret>pure
            @<caret>test.skip
            fun foo() {}
            
            struct Message {
                @<caret>abi.clientType(Cell)
                body: cell
            }
        "#,
        expect![[r#"
            Symbol with this annotation is deprecated and should not be used in new code. First string argument is a reason for deprecation as a string literal.
            Function with this annotation will be automatically inlined during compilation
            Function with this annotation will be automatically inlined by reference during compilation
            Function with this annotation will not be inlined even if compiler can inline it
            Defines the policy for handling bounced messages. Right now, only `"manual"` value is supported.
            Defines the policy for handling potential builder overflow. Right now, only `"suppress"` value is supported. See <https://docs.ton.org/v3/documentation/smart-contracts/tolk/tolk-vs-func/pack-to-from-cells#what-if-data-exceeds-1023-bits> for more details
            Function with this annotation has no side effects and can be optimized away by the compiler
            Marks the test as skipped.
            Overrides the client-facing ABI type for a struct field. This is useful when generated wrappers should expose a different representation than the serialized Tolk field type."#]],
    );
}

#[test]
fn auto_return_type_01_function_with_auto_return_type_documentation_void_type() {
    // Checks Function with auto return type documentation: void type; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r"
            fun <caret>foo() {}
        ",
        expect![[r"
            ```tolk
            fun foo(): void
            ```"]],
    );
}

#[test]
fn auto_return_type_02_function_with_auto_return_type_documentation_int_type() {
    // Checks Function with auto return type documentation: int type; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r"
            fun <caret>foo() {
                return 10;
            }
        ",
        expect![[r"
            ```tolk
            fun foo(): int
            ```"]],
    );
}

#[test]
fn auto_return_type_03_function_with_auto_return_type_documentation_union_type() {
    // Checks Function with auto return type documentation: union type; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r#"
            fun <caret>foo(cond: bool) {
                if (cond) {
                    return "hello";
                }
                return 10;
            }
        "#,
        expect![[r"
            ```tolk
            fun foo(cond: bool): string | int
            ```"]],
    );
}

#[test]
fn auto_return_type_04_function_with_auto_return_type_documentation_generic_type_with_null() {
    // Checks Function with auto return type documentation: generic type with null; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r"
            fun <caret>foo<T>(arg: T, cond: bool) {
                if (cond) {
                    return null;
                }
                return arg;
            }
        ",
        expect![[r"
            ```tolk
            fun foo<T>(arg: T, cond: bool): T?
            ```"]],
    );
}

#[test]
fn auto_return_type_05_function_with_auto_return_type_documentation_cyclic_deps() {
    // Checks Function with auto return type documentation: cyclic deps; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r"
            fun <caret>foo(cond: bool) {
                if (cond) {
                    return foo();
                }
                return 10;
            }
        ",
        expect![[r"
            ```tolk
            fun foo(cond: bool): int
            ```"]],
    );
}

#[test]
fn auto_return_type_06_static_method_with_auto_return_type_documentation_void_type() {
    // Checks Static method with auto return type documentation: void type; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r"
            fun int.<caret>foo() {}
        ",
        expect![[r"
            ```tolk
            fun int.foo(): void
            ```"]],
    );
}

#[test]
fn auto_return_type_07_instance_method_with_auto_return_type_documentation_void_type() {
    // Checks Instance method with auto return type documentation: void type; ported from
    // auto-return-type.test.
    case_tolk_hover(
        r"
            fun int.<caret>foo(self) {}
        ",
        expect![[r"
            ```tolk
            fun int.foo(self): void
            ```"]],
    );
}

#[test]
fn basic_01_simple_function_documentation() {
    // Checks Simple function documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun <caret>foo() {}
        ",
        expect![[r"
            ```tolk
            fun foo(): void
            ```"]],
    );
}

#[test]
fn basic_02_simple_backticked_function_documentation() {
    // Checks Simple backticked function documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun <caret>`hello world`() {}
        ",
        expect![[r"
            ```tolk
            fun `hello world`(): void
            ```"]],
    );
}

#[test]
fn basic_03_simple_function_documentation_with_annotation() {
    // Checks Simple function documentation with annotation; ported from basic.test.
    case_tolk_hover(
        r"
            @pure
            fun <caret>foo() {}
        ",
        expect![[r"
            ```tolk
            @pure
            fun foo(): void
            ```"]],
    );
}

#[test]
fn basic_04_generic_function_documentation() {
    // Checks Generic function documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun <caret>foo<TName>() {}
        ",
        expect![[r"
            ```tolk
            fun foo<TName>(): void
            ```"]],
    );
}

#[test]
fn basic_05_function_with_parameters_documentation() {
    // Checks Function with parameters documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun <caret>foo(a: int): slice {}
        ",
        expect![[r"
            ```tolk
            fun foo(a: int): slice
            ```"]],
    );
}

#[test]
fn basic_06_instance_method_documentation() {
    // Checks Instance method documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun Foo.<caret>foo(self, other: int): (slice, bool) {}
        ",
        expect![[r"
            ```tolk
            fun Foo.foo(self, other: int): (slice, bool)
            ```"]],
    );
}

#[test]
fn basic_07_instance_method_with_annotation_documentation() {
    // Checks Instance method with annotation documentation; ported from basic.test.
    case_tolk_hover(
        r"
            @inline
            fun Foo.<caret>foo(self, other: int): (slice, bool) {}
        ",
        expect![[r"
            ```tolk
            @inline
            fun Foo.foo(self, other: int): (slice, bool)
            ```"]],
    );
}

#[test]
fn basic_08_static_method_documentation() {
    // Checks Static method documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun Foo.<caret>foo(other: int): [slice, bool] {}
        ",
        expect![[r"
            ```tolk
            fun Foo.foo(other: int): [slice, bool]
            ```"]],
    );
}

#[test]
fn basic_09_static_method_with_annotations_documentation() {
    // Checks Static method with annotations documentation; ported from basic.test.
    case_tolk_hover(
        r#"
            @pure
            @inline
            @some("hello")
            fun Foo.<caret>foo(other: int): [slice, bool] {}
        "#,
        expect![[r#"
            ```tolk
            @pure
            @inline
            @some("hello")
            fun Foo.foo(other: int): [slice, bool]
            ```"#]],
    );
}

#[test]
fn basic_10_empty_struct_documentation() {
    // Checks Empty struct documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {}
        ",
        expect![[r"
            ```tolk
            struct Foo {}
            ```
            **Size:** 0 bits.
            
            ---"]],
    );
}

#[test]
fn basic_11_struct_documentation_without_body() {
    // Checks Struct documentation without body; ported from basic.test.
    case_tolk_hover(
        r"
            struct <caret>Foo
            struct <caret>Bar
        ",
        expect![[r"
            ```tolk
            struct Foo {}
            ```
            **Size:** 0 bits.
            
            ---
            ```tolk
            struct Bar {}
            ```
            **Size:** 0 bits.
            
            ---"]],
    );
}

#[test]
fn basic_12_struct_documentation() {
    // Checks Struct documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                value: int,
                other: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                value: int
                other: bool
            }
            ```
            **Size:** 258 bits.
            
            ---"]],
    );
}

#[test]
fn basic_13_struct_with_pack_prefix_documentation() {
    // Checks Struct with pack prefix documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct (0x100) <caret>Foo {
                value: int,
                other: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct (0x100) Foo {
                value: int
                other: bool
            }
            ```
            **Size:** 270 bits.
            
            ---"]],
    );
}

#[test]
fn basic_14_struct_with_pack_prefix_and_type_parameters_documentation() {
    // Checks Struct with pack prefix and type parameters documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct (0x100) <caret>Foo<TName, TValue=int> {
                value: TName,
                other: TValue,
            }
        ",
        expect![[r"
            ```tolk
            struct (0x100) Foo<TName, TValue=int> {
                value: TName
                other: TValue
            }
            ```
            **Size:** 12..9999 bits, 0..8 refs.
            
            ---"]],
    );
}

#[test]
fn basic_15_struct_with_comments_inside() {
    // Checks Struct with comments inside; ported from basic.test.
    case_tolk_hover(
        r#"
            /// Options for creating an outgoing message.
            /// Consider [createMessage] for examples.
            struct <caret>CreateMessageOptions<TBody = never> {
                /// whether a message will bounce back on error
                bounce: bool;
                /// message value: attached tons (or tons + extra currencies)
                value: coins | (coins, ExtraCurrenciesMap);
                /// destination is either a provided address, or is auto-calculated by stateInit
                dest: | address             // either just send a message to some address
                      | builder             // ... or a manually constructed builder with a valid address
                      | (int8, uint256)     // ... or to workchain + hash (also known as accountID)
                      | AutoDeployAddress;    // ... or "send to stateInit" aka deploy (address auto-calculated)
                /// body is any serializable object (or just miss this field for empty body)
                body: TBody;
            }
        "#,
        expect![[r"
            ```tolk
            struct CreateMessageOptions<TBody = never> {
                bounce: bool
                value: coins | (coins, ExtraCurrenciesMap)
                dest: | address             // either just send a message to some address
                      | builder             // ... or a manually constructed builder with a valid address
                      | (int8, uint256)     // ... or to workchain + hash (also known as accountID)
                      | AutoDeployAddress
                body: TBody
            }
            ```
            **Size:** 8..9999 bits, 0..9 refs.
            
            ---
            
            Options for creating an outgoing message.
            Consider [createMessage] for examples."]],
    );
}

#[test]
fn basic_16_struct_with_field_with_modifiers_documentation() {
    // Checks Struct with field with modifiers documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct Foo {
                readonly <caret>value: int,
                private <caret>other: bool,
                private readonly <caret>blabla: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            readonly value: int
            ```
            ```tolk
            struct Foo
            private other: bool
            ```
            ```tolk
            struct Foo
            private readonly blabla: bool
            ```"]],
    );
}

#[test]
fn basic_17_struct_field_documentation() {
    // Checks Struct field documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct Foo {
                <caret>value: int,
                other: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```"]],
    );
}

#[test]
fn basic_18_struct_field_with_default_value_documentation() {
    // Checks Struct field with default value documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct Foo {
                <caret>value: int = 100,
                other: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int = 100
            ```"]],
    );
}

#[test]
fn basic_19_struct_field_documentation_with_doc_comment() {
    // Checks Struct field documentation with doc comment; ported from basic.test.
    case_tolk_hover(
        r"
            struct Foo {
                /// some useful field
                <caret>value: int,
                other: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```
            some useful field"]],
    );
}

#[test]
fn basic_20_struct_field_documentation_with_inline_comment() {
    // Checks Struct field documentation with inline comment; ported from basic.test.
    case_tolk_hover(
        r"
            struct Foo {
                <caret>value: int, // some useful field
                other: bool,
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```
            some useful field"]],
    );
}

#[test]
fn basic_21_type_alias_documentation() {
    // Checks Type alias documentation; ported from basic.test.
    case_tolk_hover(
        r"
            type <caret>Int = int;
        ",
        expect![[r"
            ```tolk
            type Int = int
            ```
            **Size:** 257 bits.
            
            ---"]],
    );
}

#[test]
fn basic_22_type_alias_with_type_parameters_documentation() {
    // Checks Type alias with type parameters documentation; ported from basic.test.
    case_tolk_hover(
        r"
            type <caret>Foo<TName, TValue=slice> = TName | TValue;
        ",
        expect![[r"
            ```tolk
            type Foo<TName, TValue=slice> =
                | TName
                | TValue
            ```
            **Size:** 1..9999 bits, 0..4 refs.
            
            ---"]],
    );
}

#[test]
fn basic_23_type_alias_with_union_type_documentation() {
    // Checks Type alias with union type documentation; ported from basic.test.
    case_tolk_hover(
        r"
            type <caret>IntOrStrong = int | string;
        ",
        expect![[r"
            ```tolk
            type IntOrStrong =
                | int
                | string
            ```"]],
    );
}

#[test]
fn basic_24_type_alias_with_union_2_type_documentation() {
    // Checks Type alias with union 2 type documentation; ported from basic.test.
    case_tolk_hover(
        r"
            struct AllowedOpElectorNewStake
            struct AllowedOpElectorRecoverStake
            struct AllowedOpElectorVoteForComplaint
            struct AllowedOpElectorVoteForProposal
            
            type <caret>AllowedOpToElectorContract =
                | AllowedOpElectorNewStake
                | AllowedOpElectorRecoverStake
                | AllowedOpElectorVoteForComplaint
                | AllowedOpElectorVoteForProposal
        ",
        expect![[r"
            ```tolk
            type AllowedOpToElectorContract =
                | AllowedOpElectorNewStake
                | AllowedOpElectorRecoverStake
                | AllowedOpElectorVoteForComplaint
                | AllowedOpElectorVoteForProposal
            ```
            **Size:** 2 bits.
            
            ---"]],
    );
}

#[test]
fn basic_25_type_alias_with_builtin_type() {
    // Checks Type alias with builtin type; ported from basic.test.
    case_tolk_hover(
        r"
            type <caret>int = builtin;
        ",
        expect![[r"
            ```tolk
            type int = builtin
            ```"]],
    );
}

#[test]
fn basic_26_constant_declaration() {
    // Checks Constant declaration; ported from basic.test.
    case_tolk_hover(
        r"
            const <caret>FOO = 100;
        ",
        expect![[r"
            ```tolk
            const FOO: int = 100
            ```"]],
    );
}

#[test]
fn basic_27_constant_declaration_with_type() {
    // Checks Constant declaration with type; ported from basic.test.
    case_tolk_hover(
        r"
            const <caret>FOO: int = 100;
        ",
        expect![[r"
            ```tolk
            const FOO: int = 100
            ```"]],
    );
}

#[test]
fn basic_28_global_variable_declaration() {
    // Checks Global variable declaration; ported from basic.test.
    case_tolk_hover(
        r"
            global <caret>foo: int;
        ",
        expect![[r"
            ```tolk
            global foo: int
            ```"]],
    );
}

#[test]
fn basic_29_local_variable_documentation() {
    // Checks Local variable documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                val <caret>value = 10;
            }
        ",
        expect![[r"
            ```tolk
            val value: int = 10
            ```"]],
    );
}

#[test]
fn basic_30_local_variable_with_typehint_documentation() {
    // Checks Local variable with typehint documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                val <caret>value: int = 10;
            }
        ",
        expect![[r"
            ```tolk
            val value: int = 10
            ```"]],
    );
}

#[test]
fn basic_31_local_mutable_variable_documentation() {
    // Checks Local mutable variable documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                var <caret>value = 10;
            }
        ",
        expect![[r"
            ```tolk
            var value: int = 10
            ```"]],
    );
}

#[test]
fn basic_32_local_tuple_variable_documentation() {
    // Checks Local tuple variable documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                val [<caret>value, other] = [10, 1];
            }
        ",
        expect![[r"
            ```tolk
            val [value, other] = [10, 1]
            ```"]],
    );
}

#[test]
fn basic_33_parameter_documentation() {
    // Checks Parameter documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo(<caret>param: int) {}
        ",
        expect![[r"
            ```tolk
            param: int
            ```"]],
    );
}

#[test]
fn basic_34_mutable_parameter_documentation() {
    // Checks Mutable parameter documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo(mutate <caret>param: int) {}
        ",
        expect![[r"
            ```tolk
            mutate param: int
            ```"]],
    );
}

#[test]
fn basic_35_parameter_with_default_value_documentation() {
    // Checks Parameter with default value documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo(<caret>param: int = 10) {}
        ",
        expect![[r"
            ```tolk
            param: int = 10
            ```"]],
    );
}

#[test]
fn basic_36_catch_variable_documentation() {
    // Checks Catch variable documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                try {} catch (<caret>e) {}
            }
        ",
        expect![[r"
            ```tolk
            catch (e)
            ```"]],
    );
}

#[test]
fn basic_37_second_catch_variable_documentation() {
    // Checks Second catch variable documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                try {} catch (e, <caret>d) {}
            }
        ",
        expect![[r"
            ```tolk
            catch (d)
            ```"]],
    );
}

#[test]
fn basic_38_no_documentation_for_fun() {
    // Checks No documentation for fun; ported from basic.test.
    case_tolk_hover(
        r"
            <caret>fun foo() {}
        ",
        expect![[r"
            no documentation"]],
    );
}

#[test]
fn basic_39_get_method_documentation() {
    // Checks Get method documentation; ported from basic.test.
    case_tolk_hover(
        r"
            get fun <caret>foo() {}
        ",
        expect![[r"
            ```tolk
            get fun foo()
            ```
            Method ID: `0x1af96`"]],
    );
}

#[test]
fn basic_40_get_method_with_explicit_method_id_documentation() {
    // Checks Get method with explicit method id documentation; ported from basic.test.
    case_tolk_hover(
        r"
            @method_id(0x100)
            get fun <caret>foo() {}
        ",
        expect![[r"
            ```tolk
            @method_id(0x100)
            get fun foo()
            ```
            Method ID: `0x100`"]],
    );
}

#[test]
fn basic_41_get_method_documentation_with_comment_and_method_id() {
    // Checks Get method documentation with comment and method id; ported from basic.test.
    case_tolk_hover(
        r"
            /// Some getter with method id 0x100
            @method_id(0x100)
            get fun <caret>foo() {}
        ",
        expect![[r"
            ```tolk
            @method_id(0x100)
            get fun foo()
            ```
            Method ID: `0x100`
            
            Some getter with method id 0x100"]],
    );
}

#[test]
fn basic_42_unresolved_symbol_documentation() {
    // Checks Unresolved symbol documentation; ported from basic.test.
    case_tolk_hover(
        r"
            fun foo() {
                <caret>someUnknownFunction();
            }
        ",
        expect![[r"
            no documentation"]],
    );
}

#[test]
fn contract_01_contract_documentation() {
    // Checks Contract documentation; ported from contract.test.
    case_tolk_hover(
        r#"
            contract <caret>MyContract {
                author: "me",
                version: "1.0.0"
            }
        "#,
        expect![[r#"
            ```tolk
            contract MyContract {
                author: "me"
                version: "1.0.0"
            }
            ```"#]],
    );
}

#[test]
fn contract_02_contract_field_documentation() {
    // Checks Contract field documentation; ported from contract.test.
    case_tolk_hover(
        r#"
            contract MyContract {
                <caret>author: "me",
                version: "1.0.0"
            }
        "#,
        expect![[r"
            ```tolk
            contract MyContract
            author
            ```
            Author of the contract."]],
    );
}

#[test]
fn enums_01_enum_documentation() {
    // Checks Enum documentation; ported from enums.test.
    case_tolk_hover(
        r"
            enum <caret>Color {
                Red = 10,
                Blue = 200 + 100,
                Green,
            }
        ",
        expect![[r"
            ```tolk
            enum Color {
                Red = 10
                Blue = 200 + 100
                Green
            }
            ```"]],
    );
}

#[test]
fn enums_02_enum_with_backed_type_documentation() {
    // Checks Enum with backed type documentation; ported from enums.test.
    case_tolk_hover(
        r"
            enum <caret>Color: uint8 {
                Red = 10,
                Blue = 200 + 100,
                Green,
            }
        ",
        expect![[r"
            ```tolk
            enum Color: uint8 {
                Red = 10
                Blue = 200 + 100
                Green
            }
            ```"]],
    );
}

#[test]
fn enums_03_enum_member_documentation() {
    // Checks Enum member documentation; ported from enums.test.
    case_tolk_hover(
        r"
            enum Color {
                <caret>Red = 10,
                <caret>Blue = 200 + 100,
                <caret>Green,
            }
        ",
        expect![[r"
            ```tolk
            enum Color
            Red = 10
            ```
            ```tolk
            enum Color
            Blue = 200 + 100
            ```
            ```tolk
            enum Color
            Green
            ```"]],
    );
}

#[test]
fn exit_codes_01_exit_code_documentation_for_throw() {
    // Checks Exit code documentation for throw; ported from exit-codes.test.
    case_tolk_hover(
        r"
            fun foo() {
                throw <caret>1;
            }
        ",
        expect![[r"
            Alternative successful execution exit code. Reserved, but doesn’t occur.
            
            **Phase**: Compute phase
            
            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes"]],
    );
}

#[test]
fn exit_codes_02_exit_code_documentation_for_assert() {
    // Checks Exit code documentation for assert; ported from exit-codes.test.
    case_tolk_hover(
        r"
            fun foo() {
                assert (true) throw <caret>5;
            }
        ",
        expect![[r"
            Range check error — some integer is out of its expected range.
            
            **Phase**: Compute phase
            
            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes"]],
    );
}

#[test]
fn exit_codes_03_exit_code_documentation_for_assert_2() {
    // Checks Exit code documentation for assert 2; ported from exit-codes.test.
    case_tolk_hover(
        r"
            fun foo() {
                assert(true, <caret>10);
            }
        ",
        expect![[r"
            Dictionary error.
            
            **Phase**: Compute phase
            
            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes"]],
    );
}

#[test]
fn exit_codes_04_exit_code_documentation_for_assert_condition() {
    // Checks Exit code documentation for assert condition; ported from exit-codes.test.
    case_tolk_hover(
        r"
            fun foo() {
                assert(<caret>10, 10);
            }
        ",
        expect![[r"
            no documentation"]],
    );
}

#[test]
fn fields_01_inline_field_documentation_without_comma() {
    // Checks Inline field documentation without comma; ported from fields.test.
    case_tolk_hover(
        r"
            struct Foo {
                <caret>value: int // comment here
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```
            comment here"]],
    );
}

#[test]
fn fields_02_inline_field_documentation_with_comma() {
    // Checks Inline field documentation with comma; ported from fields.test.
    case_tolk_hover(
        r"
            struct Foo {
                <caret>value: int, // comment here
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```
            comment here"]],
    );
}

#[test]
fn fields_03_inline_field_documentation_with_several_comments() {
    // Checks Inline field documentation with several comments; ported from fields.test.
    case_tolk_hover(
        r"
            struct Foo {
                <caret>value: int, /* comment */ /* comment2 */
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```
            comment"]],
    );
}

#[test]
fn fields_04_inline_field_documentation_with_plain_documentation() {
    // Checks Inline field documentation with plain documentation; ported from fields.test.
    case_tolk_hover(
        r"
            struct Foo {
                // documentation here
                <caret>value: int, // comment here
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            value: int
            ```
            documentation here"]],
    );
}

#[test]
fn fields_05_field_with_single_modifier_documentation() {
    // Checks Field with single modifier documentation; ported from fields.test.
    case_tolk_hover(
        r"
            struct Foo {
                readonly <caret>value: int
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            readonly value: int
            ```"]],
    );
}

#[test]
fn fields_06_field_with_several_modifiers_documentation() {
    // Checks Field with several modifiers documentation; ported from fields.test.
    case_tolk_hover(
        r"
            struct Foo {
                private readonly <caret>value: int
            }
        ",
        expect![[r"
            ```tolk
            struct Foo
            private readonly value: int
            ```"]],
    );
}

#[test]
fn imports_01_import_path_from_stdlib_documentation() {
    // Checks Import path from stdlib documentation; ported from imports.test.
    case_tolk_hover(
        r#"
            import "<caret>@stdlib/common"
        "#,
        expect![[r#"
            ```tolk
            import "/__tolk_stdlib__/common.tolk"
            ```"#]],
    );
}

#[test]
fn imports_02_import_path_documentation() {
    // Checks Import path documentation; ported from imports.test.
    case_tolk_hover(
        r#"
            import "<caret>./test.tolk"
        "#,
        expect![[r#"
            ```tolk
            import "/fixture/test.tolk"
            ```"#]],
    );
}

#[test]
fn map_01_variable_declaration() {
    // Checks Variable declaration; ported from map.test.
    case_tolk_hover(
        r#"
            import "@stdlib/tvm-dicts";
            
            struct Map<K, <caret>V> {
                data: <caret>dict,
            }
            
            fun emptyMap<K, V>() {
                return Map<K, <caret>V> {
                    <caret>data: null
                };
            }
            
            fun Map<K, V>.set(self,
                <caret>key: K,
                value: <caret>V) {}
            
            fun Map<int32, int>.set(mutate self, key: int, value: int) {
                self.<caret>data.iDictSetBuilder(32, key,
                    <caret>beginCell()
                    .<caret>storeInt(value, 257));
            }
            
            fun Map<int32, int>.has(mutate self, key: int) {
                return <caret>self.data.
                    <caret>iDictGet(32, key).1;
            }
            
            fun main() {
                var map = <caret>emptyMap<int32, int>();
                map.<caret>set(1, 10);
            
                if (<caret>map
                    .<caret>has(1)) {
                    return;
                }
            
                throw <caret>2;
            }
        "#,
        expect![[r#"
            ```tolk
            struct Map
            V
            ```
            ```tolk
            type dict = cell?
            ```
            **Size:** 1 bit, 0..1 refs.

            ---

            Think of it as "a map with unknown keys and unknown values".
            Prefer using `map<K, V>`, not `dict`.
            ```tolk
            fun emptyMap
            V
            ```
            ```tolk
            struct Map
            data: dict
            ```
            ```tolk
            key: K
            ```
            ```tolk
            fun Map<K, V>.set
            V
            ```
            ```tolk
            struct Map
            data: dict
            ```
            ```tolk
            @pure
            fun beginCell(): builder
            ```
            Creates a new empty builder.
            ```tolk
            @pure
            fun builder.storeInt(mutate self, x: int, len: int): self
            ```
            Stores a signed len-bit integer into a builder (`0 ≤ len ≤ 257`).
            ```tolk
            mutate self: Map<int32, int>
            ```
            ```tolk
            @pure
            fun dict.iDictGet(self, keyLen: int, key: int): (slice?, bool)
            ```
            ```tolk
            fun emptyMap<K, V>(): Map<K, V>
            ```
            ```tolk
            fun Map<int32, int>.set(mutate self, key: int, value: int): void
            ```
            ```tolk
            var map: Map<int32, int> = emptyMap<int32, int>()
            ```
            ```tolk
            fun Map<int32, int>.has(mutate self, key: int): bool
            ```
            Stack underflow.

            **Phase**: Compute phase

            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes"#]],
    );
}

#[test]
fn size_of_01_size_of_struct_without_prefix() {
    // Checks Size of: struct without prefix; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                a: uint32
                b: int32
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: uint32
                b: int32
            }
            ```
            **Size:** 64 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_02_size_of_struct_with_0x1_prefix() {
    // Checks Size of: struct with 0x1 prefix; ported from size-of.test.
    case_tolk_hover(
        r"
            struct (0x1) <caret>Foo { // 4 + 32 + 32
                a: uint32
                b: int32
            }
        ",
        expect![[r"
            ```tolk
            struct (0x1) Foo {
                a: uint32
                b: int32
            }
            ```
            **Size:** 68 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_03_size_of_struct_with_0x7e8764ef_prefix() {
    // Checks Size of: struct with 0x7e8764ef prefix; ported from size-of.test.
    case_tolk_hover(
        r"
            struct (0x7e8764ef) <caret>Foo { // 32 + 32 + 32
                a: uint32
                b: int32
            }
        ",
        expect![[r"
            ```tolk
            struct (0x7e8764ef) Foo {
                a: uint32
                b: int32
            }
            ```
            **Size:** 96 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_04_size_of_primitive_types() {
    // Checks Size of: primitive types; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                a: uint32
                b: int32
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: uint32
                b: int32
            }
            ```
            **Size:** 64 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_05_size_of_optional_primitive_type() {
    // Checks Size of: optional primitive type; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                a: uint32?
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: uint32?
            }
            ```
            **Size:** 33 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_06_size_of_int32_and_coins_type() {
    // Checks Size of: int32 and coins type; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                a: int32
                b: coins
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: coins
            }
            ```
            **Size:** 36..156 bits, 0 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_07_size_of_int32_and_cell_uint64() {
    // Checks Size of: int32 and Cell<uint64>; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 0
                a: int32
                b: Cell<uint64>
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: Cell<uint64>
            }
            ```
            **Size:** 32 bits, 1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_08_size_of_int32_and_raw_cell() {
    // Checks Size of: int32 and raw cell; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 0
                a: int32
                b: cell
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: cell
            }
            ```
            **Size:** 32 bits, 1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_09_size_of_int32_and_builder() {
    // Checks Size of: int32 and builder; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 0
                a: int32
                b: builder
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: builder
            }
            ```
            **Size:** 32..9999 bits, 0..4 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_10_size_of_int32_and_bits32() {
    // Checks Size of: int32 and bits32; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 32
                a: int32
                b: bits32
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: bits32
            }
            ```
            **Size:** 64 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_11_size_of_int32_and_bytes32() {
    // Checks Size of: int32 and bytes32; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 32 * 8
                a: int32
                b: bytes32
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: bytes32
            }
            ```
            **Size:** 288 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_12_size_of_int32_uint64_and_bool() {
    // Checks Size of: int32, uint64? and bool; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 64 + 1 + 1
                a: int32
                b: uint64?
                c: bool
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32
                b: uint64?
                c: bool
            }
            ```
            **Size:** 98 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_13_size_of_address() {
    // Checks Size of: address; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 2..267
                a: address
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: address
            }
            ```
            **Size:** 2..267 bits, 0 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_14_size_of_inner_struct() {
    // Checks Size of: inner struct; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32
                a: Bar
            }
            
            struct Bar {
                b: int32
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: Bar
            }
            ```
            **Size:** 32 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_15_size_of_alias_to_int32() {
    // Checks Size of: alias to int32; ported from size-of.test.
    case_tolk_hover(
        r"
            type MyInt = int32
            
            struct <caret>Foo { // 32
                a: MyInt
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: MyInt
            }
            ```
            **Size:** 32 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_16_size_of_tensor_type() {
    // Checks Size of: tensor type; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 1
                a: (int32, bool)
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: (int32, bool)
            }
            ```
            **Size:** 33 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_17_size_of_tensor_type_with_cell() {
    // Checks Size of: tensor type with cell; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 1 + 1 ref
                a: (int32, bool, cell)
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: (int32, bool, cell)
            }
            ```
            **Size:** 33 bits, 1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_18_size_of_tuple_type() {
    // Checks Size of: tuple type; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 1
                a: (int32, bool)
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: (int32, bool)
            }
            ```
            **Size:** 33 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_19_size_of_tuple_type_with_cell() {
    // Checks Size of: tuple type with cell; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 32 + 1 + 1 ref
                a: [int32, bool, cell]
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: [int32, bool, cell]
            }
            ```
            **Size:** 33 bits, 1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_20_size_of_maybe_int32() {
    // Checks Size of: maybe int32; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 1 + 32
                a: int32?
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32?
            }
            ```
            **Size:** 33 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_21_size_of_maybe_cell() {
    // Checks Size of: maybe cell; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 1 + 1 ref
                a: cell?
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: cell?
            }
            ```
            **Size:** 1 bit, 0..1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_22_size_of_maybe_cell_int32() {
    // Checks Size of: maybe Cell<int32>; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 1 + 1 ref
                a: Cell<int32>?
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: Cell<int32>?
            }
            ```
            **Size:** 1 bit, 0..1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_23_size_of_either_int32_int64() {
    // Checks Size of: Either int32 | int64; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo { // 1 + max(32, 64)
                a: int32 | int64
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32 | int64
            }
            ```
            **Size:** 33..65 bits, 0 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_24_size_of_either_structs() {
    // Checks Size of: Either structs; ported from size-of.test.
    case_tolk_hover(
        r"
            struct (0x7e8764ef) <caret>IncreaseCounter {
                queryId: uint64
                increaseBy: uint32
            }
            
            struct (0x3a) <caret>ResetCounter {
                queryId: uint64
                action: Cell<SwapRequest>
            }
            
            struct SwapRequest {
              receiver: int32?
            }
            
            struct <caret>Foo { // 1 + max(32, 64)
                a: IncreaseCounter | ResetCounter
            }
        ",
        expect![[r"
            ```tolk
            struct (0x7e8764ef) IncreaseCounter {
                queryId: uint64
                increaseBy: uint32
            }
            ```
            **Size:** 128 bits.
            
            ---
            ```tolk
            struct (0x3a) ResetCounter {
                queryId: uint64
                action: Cell<SwapRequest>
            }
            ```
            **Size:** 72 bits, 1 refs.
            
            ---
            ```tolk
            struct Foo {
                a: IncreaseCounter | ResetCounter
            }
            ```
            **Size:** 72..128 bits, 0..1 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_25_size_of_union_of_primitive_types() {
    // Checks Size of: Union of primitive types; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                a: int32 | int64 | bool
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32 | int64 | bool
            }
            ```
            **Size:** 3..66 bits, 0 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_26_size_of_union_of_primitive_types_2() {
    // Checks Size of: Union of primitive types 2; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                a: int32 | int64 | bool | int32 | int11
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                a: int32 | int64 | bool | int32 | int11
            }
            ```
            **Size:** 3..66 bits, 0 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_27_size_of_with_remainingbitsandrefs() {
    // Checks Size of: With RemainingBitsAndRefs; ported from size-of.test.
    case_tolk_hover(
        r"
            type ForwardPayloadRemainder = RemainingBitsAndRefs
            
            struct (0x0f8a7ea5) <caret>AskToTransfer {
                queryId: uint64
                jettonAmount: coins
                transferRecipient: address
                sendExcessesTo: address
                customPayload: cell?
                forwardTonAmount: coins
                forwardPayload: ForwardPayloadRemainder
            }
        ",
        expect![[r"
            ```tolk
            struct (0x0f8a7ea5) AskToTransfer {
                queryId: uint64
                jettonAmount: coins
                transferRecipient: address
                sendExcessesTo: address
                customPayload: cell?
                forwardTonAmount: coins
                forwardPayload: ForwardPayloadRemainder
            }
            ```
            **Size:** 109..9999 bits, 0..5 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_28_size_of_generic_struct_with_t_uint32() {
    // Checks Size of: generic struct with T=uint32; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                bar: Bar<uint32>
            }
            
            struct <caret>Bar<T> {
                a: T
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                bar: Bar<uint32>
            }
            ```
            **Size:** 32 bits.
            
            ---
            ```tolk
            struct Bar<T> {
                a: T
            }
            ```
            **Size:** 0..9999 bits, 0..4 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_29_size_of_generic_struct_with_t_cell() {
    // Checks Size of: generic struct with T=cell; ported from size-of.test.
    case_tolk_hover(
        r"
            struct <caret>Foo {
                bar: Bar<cell>
            }
            
            struct <caret>Bar<T> {
                a: T
            }
        ",
        expect![[r"
            ```tolk
            struct Foo {
                bar: Bar<cell>
            }
            ```
            **Size:** 0 bits, 1 refs.
            
            ---
            ```tolk
            struct Bar<T> {
                a: T
            }
            ```
            **Size:** 0..9999 bits, 0..4 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_30_size_of_alias_for_int32() {
    // Checks Size of: Alias for int32; ported from size-of.test.
    case_tolk_hover(
        r"
            type <caret>MyInt = int32
        ",
        expect![[r"
            ```tolk
            type MyInt = int32
            ```
            **Size:** 32 bits.
            
            ---"]],
    );
}

#[test]
fn size_of_31_size_of_alias_for_int32_int64() {
    // Checks Size of: Alias for int32 | int64; ported from size-of.test.
    case_tolk_hover(
        r"
            type <caret>MyInt = int32 | int64
        ",
        expect![[r"
            ```tolk
            type MyInt =
                | int32
                | int64
            ```
            **Size:** 33..65 bits, 0 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_32_size_of_alias_for_tensor_int32_int64_cell() {
    // Checks Size of: Alias for tensor (int32 | int64, cell); ported from size-of.test.
    case_tolk_hover(
        r"
            type <caret>MyInt = (int32 | int64, cell)
        ",
        expect![[r"
            ```tolk
            type MyInt = (int32 | int64, cell)
            ```
            **Size:** 33..65 bits, 1 ref.
            
            ---"]],
    );
}

#[test]
fn size_of_33_size_of_generic_alias() {
    // Checks Size of: Generic alias; ported from size-of.test.
    case_tolk_hover(
        r"
            type <caret>Root = Foo<int32>
            
            type <caret>Foo<T> = T
        ",
        expect![[r"
            ```tolk
            type Root = Foo<int32>
            ```
            **Size:** 32 bits.
            
            ---
            ```tolk
            type Foo<T> = T
            ```
            **Size:** 0..9999 bits, 0..4 refs.
            
            ---"]],
    );
}

#[test]
fn size_of_34_size_of_alias_for_generic_struct_with_t_uint32() {
    // Checks Size of: Alias for generic struct with T=uint32; ported from size-of.test.
    case_tolk_hover(
        r"
            type <caret>Root = Foo<int32>
            
            type <caret>Foo<T> = Bar<T>
            
            struct <caret>Bar<T> {
                a: T
            }
        ",
        expect![[r"
            ```tolk
            type Root = Foo<int32>
            ```
            **Size:** 32 bits.
            
            ---
            ```tolk
            type Foo<T> = Bar<T>
            ```
            **Size:** 0..9999 bits, 0..4 refs.
            
            ---
            ```tolk
            struct Bar<T> {
                a: T
            }
            ```
            **Size:** 0..9999 bits, 0..4 refs.
            
            ---"]],
    );
}

#[test]
fn tlb_01_struct_field_int32_tlb_type() {
    // Checks Struct field int32 tlb type; ported from tlb.test.
    case_tolk_hover(
        r"
            type intN = builtin;
            
            struct Foo {
                value: <caret>int32,
            }
        ",
        expect![[r"
            ```tolk
            type int32 = builtin
            ```
            
            - **Range**: -2^31 to 2^31 - 1
            - **Size**: 32 bits = 4 bytes
            - **TL-B**: int32"]],
    );
}

#[test]
fn tlb_02_struct_field_uint32_tlb_type() {
    // Checks Struct field uint32 tlb type; ported from tlb.test.
    case_tolk_hover(
        r"
            type uintN = builtin;
            
            struct Foo {
                value: <caret>uint32,
            }
        ",
        expect![[r"
            ```tolk
            type uint32 = builtin
            ```
            
            - **Range**: 0 to 4,294,967,295 (2^32 - 1)
            - **Size**: 32 bits = 4 bytes
            - **TL-B**: uint32"]],
    );
}

#[test]
fn tlb_03_struct_field_int24_tlb_type() {
    // Checks Struct field int24 tlb type; ported from tlb.test.
    case_tolk_hover(
        r"
            type intN = builtin;
            
            struct Foo {
                value: <caret>int24,
            }
        ",
        expect![[r"
            ```tolk
            type int24 = builtin
            ```
            
            - **Range**: -2^23 to 2^23 - 1
            - **Size**: 24 bits
            - **TL-B**: int24
            
            Arbitrary bit-width signed integer type"]],
    );
}

#[test]
fn tlb_04_struct_field_uint244_tlb_type() {
    // Checks Struct field uint244 tlb type; ported from tlb.test.
    case_tolk_hover(
        r"
            type uintN = builtin;
            
            struct Foo {
                value: <caret>uint244,
            }
        ",
        expect![[r"
            ```tolk
            type uint244 = builtin
            ```
            
            - **Range**: 0 to 2^244 - 1
            - **Size**: 244 bits
            - **TL-B**: uint244
            
            Arbitrary bit-width unsigned integer type"]],
    );
}

#[test]
fn tlb_05_struct_field_uint9999_tlb_type() {
    // Checks Struct field uint9999 tlb type; ported from tlb.test.
    case_tolk_hover(
        r"
            type uintN = builtin;
            
            struct Foo {
                value: <caret>uint9999,
            }
        ",
        expect![[r"
            ```tolk
            type uint9999 = builtin
            ```"]],
    );
}

#[test]
fn type_parameters_01_function_type_parameter_documentation() {
    // Checks Function type parameter documentation; ported from type-parameters.test.
    case_tolk_hover(
        r"
            fun foo<TName>(): <caret>TName {}
        ",
        expect![[r"
            ```tolk
            fun foo
            TName
            ```"]],
    );
}

#[test]
fn type_parameters_02_struct_type_parameter_documentation() {
    // Checks Struct type parameter documentation; ported from type-parameters.test.
    case_tolk_hover(
        r"
            struct Generic<TName> {
                field: <caret>TName,
            }
        ",
        expect![[r"
            ```tolk
            struct Generic
            TName
            ```"]],
    );
}

#[test]
fn type_parameters_03_struct_type_parameter_with_default_type_documentation() {
    // Checks Struct type parameter with default type documentation; ported from
    // type-parameters.test.
    case_tolk_hover(
        r"
            struct Generic<TName = int> {
                field: <caret>TName,
            }
        ",
        expect![[r"
            ```tolk
            struct Generic
            TName = int
            ```"]],
    );
}

#[test]
fn type_parameters_04_type_alias_type_parameter_with_default_type_documentation() {
    // Checks Type alias type parameter with default type documentation; ported from
    // type-parameters.test.
    case_tolk_hover(
        r"
            type Generic<TName> = Foo<TNa<caret>me>;
        ",
        expect![[r"
            ```tolk
            type Generic
            TName
            ```"]],
    );
}

#[test]
fn type_parameters_05_static_method_type_parameter_documentation() {
    // Checks Static method type parameter documentation; ported from type-parameters.test.
    case_tolk_hover(
        r"
            fun int.foo<TName>(): <caret>TName {}
        ",
        expect![[r"
            ```tolk
            fun int.foo
            TName
            ```"]],
    );
}

#[test]
fn type_parameters_06_instance_method_type_parameter_documentation() {
    // Checks Instance method type parameter documentation; ported from type-parameters.test.
    case_tolk_hover(
        r"
            fun int.foo<TName>(self): <caret>TName {}
        ",
        expect![[r"
            ```tolk
            fun int.foo
            TName
            ```"]],
    );
}

#[test]
fn type_parameters_07_receiver_type_parameters() {
    // Checks Receiver type parameters; ported from type-parameters.test.
    case_tolk_hover(
        r"
            struct Foo<T> {}
            
            fun Foo<<caret>TName>.foo(): TName {}
        ",
        expect![[r"
            ```tolk
            fun Foo<TName>.foo
            TName
            ```"]],
    );
}
