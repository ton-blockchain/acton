#![allow(clippy::needless_raw_string_hashes)]

use super::{case_tolk_inlay_hints, full_document_range};
use expect_test::expect;

#[test]
fn auto_return_type_01_function_with_single_return_and_explicit_type() {
    // Ported from auto-return-type.test: Function with single return and explicit type.
    case_tolk_inlay_hints(
        r"
            fun some(): int {
                return 10;
            }
            
            fun test(): void {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(): int {
                return 10;
            }

            fun test(): void {
                val res/* : int */ = some();
            }"#]],
    );
}

#[test]
fn auto_return_type_02_function_with_single_return() {
    // Ported from auto-return-type.test: Function with single return.
    case_tolk_inlay_hints(
        r"
            fun some() {
                return 10;
            }
            
            fun test() {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some()/* : int */ {
                return 10;
            }

            fun test()/* : void */ {
                val res/* : int */ = some();
            }"#]],
    );
}

#[test]
fn auto_return_type_03_function_with_several_returns_with_single_type() {
    // Ported from auto-return-type.test: Function with several returns with single type.
    case_tolk_inlay_hints(
        r"
            fun some(cond: bool) {
                if (cond) {
                    return 20;
                } else {
                    return 30;
                }
                return 10;
            }
            
            fun test() {
                val res = some(true);
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(cond: bool)/* : int */ {
                if (cond) {
                    return 20;
                } else {
                    return 30;
                }
                return 10;
            }

            fun test()/* : void */ {
                val res/* : int */ = some(/* cond: */true);
            }"#]],
    );
}

#[test]
fn auto_return_type_04_function_with_null_and_non_null_returns() {
    // Ported from auto-return-type.test: Function with null and non null returns.
    case_tolk_inlay_hints(
        r"
            fun some(cond: bool) {
                if (cond) {
                    return null;
                }
                return 10;
            }
            
            fun test() {
                val res = some(true);
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(cond: bool)/* : int? */ {
                if (cond) {
                    return null;
                }
                return 10;
            }

            fun test()/* : void */ {
                val res/* : int? */ = some(/* cond: */true);
            }"#]],
    );
}

#[test]
fn auto_return_type_05_function_with_nullable_ternary_return() {
    // Ported from auto-return-type.test: Function with nullable ternary return.
    case_tolk_inlay_hints(
        r"
            fun some(cond: bool) {
                return cond ? null : 10;
            }
            
            fun test() {
                val res = some(true);
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(cond: bool)/* : int? */ {
                return cond ? null : 10;
            }

            fun test()/* : void */ {
                val res/* : int? */ = some(/* cond: */true);
            }"#]],
    );
}

#[test]
fn auto_return_type_06_static_method_with_single_return() {
    // Ported from auto-return-type.test: Static method with single return.
    case_tolk_inlay_hints(
        r"
            fun int.some() {
                return 10;
            }
            
            fun test() {
                val res = int.some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun int.some()/* : int */ {
                return 10;
            }

            fun test()/* : void */ {
                val res/* : int */ = int.some();
            }"#]],
    );
}

#[test]
fn auto_return_type_07_instance_method_with_single_return() {
    // Ported from auto-return-type.test: Instance method with single return.
    case_tolk_inlay_hints(
        r"
            fun int.some(self) {
                return 10;
            }
            
            fun test() {
                val res = 10.some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun int.some(self)/* : int */ {
                return 10;
            }

            fun test()/* : void */ {
                val res/* : int */ = 10.some();
            }"#]],
    );
}

#[test]
fn auto_return_type_08_get_method_with_single_return() {
    // Ported from auto-return-type.test: Get method with single return.
    case_tolk_inlay_hints(
        r"
            get fun some() {
                return 10;
            }
            
            fun test() {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            get/* (0x12f86) */ fun some()/* : int */ {
                return 10;
            }

            fun test()/* : void */ {
                val res/* : int */ = some();
            }"#]],
    );
}

#[test]
fn auto_return_type_09_function_with_single_tuple_return() {
    // Ported from auto-return-type.test: Function with single tuple return.
    case_tolk_inlay_hints(
        r"
            fun some() {
                return [10, true, 1000];
            }
            
            fun test() {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some()/* : array<int | bool> */ {
                return [10, true, 1000];
            }

            fun test()/* : void */ {
                val res/* : array<int | bool> */ = some();
            }"#]],
    );
}

#[test]
fn auto_return_type_10_function_with_several_boolean_return() {
    // Ported from auto-return-type.test: Function with several boolean return.
    case_tolk_inlay_hints(
        r"
            fun some(some: bool) {
                if (some) { return false; }
                return true;
            }
            
            fun test() {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(some: bool)/* : bool */ {
                if (some) { return false; }
                return true;
            }

            fun test()/* : void */ {
                val res/* : bool */ = some();
            }"#]],
    );
}

#[test]
fn auto_return_type_11_function_with_several_boolean_return_2() {
    // Ported from auto-return-type.test: Function with several boolean return 2.
    case_tolk_inlay_hints(
        r"
            fun some(some: bool) {
                if (some) { return true; }
                return true;
            }
            
            fun test() {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(some: bool)/* : bool */ {
                if (some) { return true; }
                return true;
            }

            fun test()/* : void */ {
                val res/* : bool */ = some();
            }"#]],
    );
}

#[test]
fn auto_return_type_12_function_with_several_boolean_return_3() {
    // Ported from auto-return-type.test: Function with several boolean return 3.
    case_tolk_inlay_hints(
        r"
            fun some(some: bool) {
                if (some) { return false; }
                return false;
            }
            
            fun test() {
                val res = some();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun some(some: bool)/* : bool */ {
                if (some) { return false; }
                return false;
            }

            fun test()/* : void */ {
                val res/* : bool */ = some();
            }"#]],
    );
}

#[test]
fn basic_evaluation_01_basic_constant_evaluation_literals() {
    // Ported from basic-evaluation.test: Basic constant evaluation, literals.
    case_tolk_inlay_hints(
        r"
            const INT_CONST = 42;
            const HEX_CONST = 0xFF;
            const BIN_CONST = 0b1010;
            const BOOL_CONST = true;
            const NULL_CONST = null;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const INT_CONST/* : int */ = 42;
            const HEX_CONST/* : int */ = 0xFF;
            const BIN_CONST/* : int */ = 0b1010;
            const BOOL_CONST/* : bool */ = true;
            const NULL_CONST/* : null */ = null;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn basic_evaluation_02_binary_operations_evaluation() {
    // Ported from basic-evaluation.test: Binary operations evaluation.
    case_tolk_inlay_hints(
        r"
            const ADD_CONST = 10 + 5;
            const SUB_CONST = 20 - 8;
            const MUL_CONST = 6 * 7;
            const DIV_CONST = 100 / 4;
            const MOD_CONST = 17 % 5;
            const SHIFT_LEFT = 1 << 3;
            const SHIFT_RIGHT = 16 >> 2;
            const BIT_AND = 0xFF & 0x0F;
            const BIT_OR = 0xF0 | 0x0F;
            const BIT_XOR = 0xFF ^ 0xAA;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const ADD_CONST/* : int */ = 10 + 5/* = 15 (0xF) */;
            const SUB_CONST/* : int */ = 20 - 8/* = 12 (0xC) */;
            const MUL_CONST/* : int */ = 6 * 7/* = 42 (0x2A) */;
            const DIV_CONST/* : int */ = 100 / 4/* = 25 (0x19) */;
            const MOD_CONST/* : int */ = 17 % 5/* = 2 (0x2) */;
            const SHIFT_LEFT/* : int */ = 1 << 3/* = 8 (0x8) */;
            const SHIFT_RIGHT/* : int */ = 16 >> 2/* = 4 (0x4) */;
            const BIT_AND/* : int */ = 0xFF & 0x0F/* = 15 (0xF) */;
            const BIT_OR/* : int */ = 0xF0 | 0x0F/* = 255 (0xFF) */;
            const BIT_XOR/* : int */ = 0xFF ^ 0xAA/* = 85 (0x55) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn basic_evaluation_03_unary_operations_evaluation() {
    // Ported from basic-evaluation.test: Unary operations evaluation.
    case_tolk_inlay_hints(
        r"
            const NEG_CONST = -42;
            const POS_CONST = +100;
            const NOT_CONST = !true;
            const NOT_FALSE = !false;
            const BIT_NOT = ~0xFF;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const NEG_CONST/* : int */ = -42/* = 0x-2A */;
            const POS_CONST/* : int */ = +100/* = 100 (0x64) */;
            const NOT_CONST/* : bool */ = !true/* = false */;
            const NOT_FALSE/* : bool */ = !false/* = true */;
            const BIT_NOT/* : int */ = ~0xFF/* = 0x-100 */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn basic_evaluation_04_reference_evaluation() {
    // Ported from basic-evaluation.test: Reference evaluation.
    case_tolk_inlay_hints(
        r"
            const BASE_CONST = 10;
            const REF_CONST = BASE_CONST;
            const EXPR_CONST = BASE_CONST * 2;
            const CHAIN_CONST = REF_CONST + 5;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const BASE_CONST/* : int */ = 10;
            const REF_CONST/* : int */ = BASE_CONST/* = 10 (0xA) */;
            const EXPR_CONST/* : int */ = BASE_CONST * 2/* = 20 (0x14) */;
            const CHAIN_CONST/* : int */ = REF_CONST + 5/* = 15 (0xF) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn basic_evaluation_05_complex_expressions_evaluation() {
    // Ported from basic-evaluation.test: Complex expressions evaluation.
    case_tolk_inlay_hints(
        r"
            const COMPLEX1 = (10 + 5) * 2;
            const COMPLEX2 = 100 / (4 + 1);
            const COMPLEX3 = (1 << 4) | (1 << 2);
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const COMPLEX1/* : int */ = (10 + 5) * 2/* = 30 (0x1E) */;
            const COMPLEX2/* : int */ = 100 / (4 + 1)/* = 20 (0x14) */;
            const COMPLEX3/* : int */ = (1 << 4) | (1 << 2)/* = 20 (0x14) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn basic_evaluation_06_circular_dependency_handling() {
    // Ported from basic-evaluation.test: Circular dependency handling.
    case_tolk_inlay_hints(
        r"
            const CIRC_A = CIRC_B + 1;
            const CIRC_B = CIRC_A - 1;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const CIRC_A/* : int */ = CIRC_B + 1;
            const CIRC_B/* : int */ = CIRC_A - 1;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn calls_01_function_without_parameters_call() {
    // Ported from calls.test: Function without parameters call.
    case_tolk_inlay_hints(
        r"
            fun foo() {}
            
            fun test() {
                foo(); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo()/* : void */ {}

            fun test()/* : void */ {
                foo(); // no hints here
            }"#]],
    );
}

#[test]
fn calls_02_function_with_parameter_call() {
    // Ported from calls.test: Function with parameter call.
    case_tolk_inlay_hints(
        r"
            fun foo(value: int) {}
            
            fun test() {
                foo(100);
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo(value: int)/* : void */ {}

            fun test()/* : void */ {
                foo(/* value: */100);
            }"#]],
    );
}

#[test]
fn calls_03_function_with_parameters_call() {
    // Ported from calls.test: Function with parameters call.
    case_tolk_inlay_hints(
        r#"
            fun foo(value: int, other: string) {}
            
            fun test() {
                foo(100, "hello");
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun foo(value: int, other: string)/* : void */ {}

            fun test()/* : void */ {
                foo(/* value: */100, /* other: */"hello");
            }"#]],
    );
}

#[test]
fn calls_04_function_with_parameters_and_too_much_arguments_call() {
    // Ported from calls.test: Function with parameters and too much arguments call.
    case_tolk_inlay_hints(
        r#"
            fun foo(value: int, other: string) {}
            
            fun test() {
                foo(100, "hello", 20, 30, 40);
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun foo(value: int, other: string)/* : void */ {}

            fun test()/* : void */ {
                foo(/* value: */100, /* other: */"hello", 20, 30, 40);
            }"#]],
    );
}

#[test]
fn calls_05_function_with_parameters_and_too_less_arguments_call() {
    // Ported from calls.test: Function with parameters and too less arguments call.
    case_tolk_inlay_hints(
        r"
            fun foo(value: int, other: string) {}
            
            fun test() {
                foo(); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo(value: int, other: string)/* : void */ {}

            fun test()/* : void */ {
                foo(); // no hints here
            }"#]],
    );
}

#[test]
fn calls_06_don_t_show_hints_for_full_struct_instance_argument() {
    // Ported from calls.test: Don't show hints for full struct instance argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun foo(options: Bar) {}
            
            fun test() {
                foo(Bar {}); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun foo(options: Bar)/* : void */ {}

            fun test()/* : void */ {
                foo(Bar {}); // no hints here
            }"#]],
    );
}

#[test]
fn calls_07_show_hints_for_short_struct_instance_argument() {
    // Ported from calls.test: Show hints for short struct instance argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun foo(options: Bar) {}
            
            fun test() {
                foo({});
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun foo(options: Bar)/* : void */ {}

            fun test()/* : void */ {
                foo(/* options: */{});
            }"#]],
    );
}

#[test]
fn calls_08_don_t_show_hints_for_same_name_argument() {
    // Ported from calls.test: Don't show hints for same name argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun foo(options: Bar) {}
            
            fun test() {
                val options = Bar{};
                foo(options); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun foo(options: Bar)/* : void */ {}

            fun test()/* : void */ {
                val options/* : Bar */ = Bar{};
                foo(options); // no hints here
            }"#]],
    );
}

#[test]
fn calls_09_show_hints_for_different_name_argument() {
    // Ported from calls.test: Show hints for different name argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun foo(options: Bar) {}
            
            fun test() {
                val ownOptions = Bar{};
                foo(ownOptions);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun foo(options: Bar)/* : void */ {}

            fun test()/* : void */ {
                val ownOptions/* : Bar */ = Bar{};
                foo(/* options: */ownOptions);
            }"#]],
    );
}

#[test]
fn calls_10_don_t_show_hints_for_same_name_field_argument() {
    // Ported from calls.test: Don't show hints for same name field argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun foo(options: Bar) {}
            
            struct Data {
                options: Bar,
            }
            
            fun test(data: Data) {
                foo(data.options); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun foo(options: Bar)/* : void */ {}

            struct Data {
                options: Bar,
            }

            fun test(data: Data)/* : void */ {
                foo(data.options); // no hints here
            }"#]],
    );
}

#[test]
fn calls_11_show_hints_for_different_name_field_argument() {
    // Ported from calls.test: Show hints for different name field argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun foo(options: Bar) {}
            
            struct Data {
                ownOptions: Bar,
            }
            
            fun test(data: Data) {
                foo(data.ownOptions);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun foo(options: Bar)/* : void */ {}

            struct Data {
                ownOptions: Bar,
            }

            fun test(data: Data)/* : void */ {
                foo(/* options: */data.ownOptions);
            }"#]],
    );
}

#[test]
fn calls_12_don_t_show_hints_for_same_name_call_argument() {
    // Ported from calls.test: Don't show hints for same name call argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun options(): Bar {}
            
            fun foo(options: Bar) {}
            
            fun test() {
                foo(options()); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun options(): Bar {}

            fun foo(options: Bar)/* : void */ {}

            fun test()/* : void */ {
                foo(options()); // no hints here
            }"#]],
    );
}

#[test]
fn calls_13_show_hints_for_different_name_call_argument() {
    // Ported from calls.test: Show hints for different name call argument.
    case_tolk_inlay_hints(
        r"
            struct Bar {
                value: int,
            }
            
            fun getOptions(): Bar {}
            
            fun foo(options: Bar) {}
            
            fun test() {
                foo(getOptions());
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Bar {
                value: int,
            }

            fun getOptions(): Bar {}

            fun foo(options: Bar)/* : void */ {}

            fun test()/* : void */ {
                foo(/* options: */getOptions());
            }"#]],
    );
}

#[test]
fn calls_14_static_method_with_parameter_call() {
    // Ported from calls.test: Static method with parameter call.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun Foo.foo(value: int) {}
            
            fun test() {
                Foo.foo(100);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun Foo.foo(value: int)/* : void */ {}

            fun test()/* : void */ {
                Foo.foo(/* value: */100);
            }"#]],
    );
}

#[test]
fn calls_15_instance_method_with_parameter_call() {
    // Ported from calls.test: Instance method with parameter call.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun Foo.foo(self, value: int) {}
            
            fun test() {
                val foo: Foo = {};
                foo.foo(100);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun Foo.foo(self, value: int)/* : void */ {}

            fun test()/* : void */ {
                val foo: Foo = {};
                foo.foo(/* value: */100);
            }"#]],
    );
}

#[test]
fn calls_16_ton_function_call() {
    // Ported from calls.test: ton() function call.
    case_tolk_inlay_hints(
        r#"
            fun test() {
                ton("1.5"); // no hints here
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun test()/* : void */ {
                ton("1.5"); // no hints here
            }"#]],
    );
}

#[test]
fn calls_17_no_hint_for_single_letter_param() {
    // Ported from calls.test: No hint for single letter param.
    case_tolk_inlay_hints(
        r"
            fun bar(a: int) {}
            
            fun test() {
                bar(10); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun bar(a: int)/* : void */ {}

            fun test()/* : void */ {
                bar(10); // no hints here
            }"#]],
    );
}

#[test]
fn calls_18_no_hint_for_calling_variable() {
    // Ported from calls.test: No hint for calling variable.
    case_tolk_inlay_hints(
        r"
            fun test() {
                val cb: (int) -> int;
                cb(10); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun test()/* : void */ {
                val cb: (int) -> int;
                cb(10); // no hints here
            }"#]],
    );
}

#[test]
fn calls_19_no_hint_for_function_without_parameters() {
    // Ported from calls.test: No hint for function without parameters.
    case_tolk_inlay_hints(
        r"
            fun bar() {}
            
            fun test() {
                bar(); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun bar()/* : void */ {}

            fun test()/* : void */ {
                bar(); // no hints here
            }"#]],
    );
}

#[test]
fn calls_20_no_hint_for_function_without_parameters_but_with_arguments() {
    // Ported from calls.test: No hint for function without parameters but with arguments.
    case_tolk_inlay_hints(
        r"
            fun bar() {}
            
            fun test() {
                bar(10, 20); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun bar()/* : void */ {}

            fun test()/* : void */ {
                bar(10, 20); // no hints here
            }"#]],
    );
}

#[test]
fn calls_21_no_hint_for_function_function_call() {
    // Ported from calls.test: No hint for function function call.
    case_tolk_inlay_hints(
        r"
            fun bar(): (int) -> int {}
            
            fun test() {
                bar()(); // no hints here
            }
        ",
        full_document_range(),
        expect![[r#"
            fun bar(): (int) -> int {}

            fun test()/* : void */ {
                bar()(); // no hints here
            }"#]],
    );
}

#[test]
fn compile_time_functions_01_compile_time_functions_evaluation_crc_functions() {
    // Ported from compile-time-functions.test: Compile-time functions evaluation, CRC functions.
    case_tolk_inlay_hints(
        r#"
            const CRC32_TEST = stringCrc32("some_str");
            const CRC16_TEST = stringCrc16("some_str");
            const CRC32_HELLO = stringCrc32("hello");
            const CRC16_HELLO = stringCrc16("hello");
            
            fun main() {
                return 0;
            }
        "#,
        full_document_range(),
        expect![[r#"
            const CRC32_TEST/* : int */ = stringCrc32("some_str")/* = 4013618352 (0xEF3AF4B0) */;
            const CRC16_TEST/* : int */ = stringCrc16("some_str")/* = 53407 (0xD09F) */;
            const CRC32_HELLO/* : int */ = stringCrc32("hello")/* = 907060870 (0x3610A686) */;
            const CRC16_HELLO/* : int */ = stringCrc16("hello")/* = 50018 (0xC362) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn compile_time_functions_02_compile_time_functions_evaluation_sha256_functions() {
    // Ported from compile-time-functions.test: Compile-time functions evaluation, SHA256 functions.
    case_tolk_inlay_hints(
        r#"
            const SHA256_TEST = stringSha256("some_crypto_key");
            const SHA256_32_TEST = stringSha256_32("some_crypto_key");
            const SHA256_HELLO = stringSha256("hello");
            const SHA256_32_HELLO = stringSha256_32("hello");
            
            fun main() {
                return 0;
            }
        "#,
        full_document_range(),
        expect![[r#"
            const SHA256_TEST/* : int */ = stringSha256("some_crypto_key")/* = 0x1C30C3FA846E4D85FB39C4A1C791F66A66DA7DE5D1ED24FCA94208F7F6D3CB21 */;
            const SHA256_32_TEST/* : int */ = stringSha256_32("some_crypto_key")/* = 472957946 (0x1C30C3FA) */;
            const SHA256_HELLO/* : int */ = stringSha256("hello")/* = 0x2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824 */;
            const SHA256_32_HELLO/* : int */ = stringSha256_32("hello")/* = 754077114 (0x2CF24DBA) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn compile_time_functions_03_compile_time_functions_evaluation_stringtobase256() {
    // Ported from compile-time-functions.test: Compile-time functions evaluation, stringToBase256.
    case_tolk_inlay_hints(
        r#"
            const BASE256_AB = stringToBase256("AB");
            const BASE256_HELLO = stringToBase256("hello");
            const BASE256_A = stringToBase256("A");
            
            fun main() {
                return 0;
            }
        "#,
        full_document_range(),
        expect![[r#"
            const BASE256_AB/* : int */ = stringToBase256("AB")/* = 16706 (0x4142) */;
            const BASE256_HELLO/* : int */ = stringToBase256("hello")/* = 0x68656C6C6F */;
            const BASE256_A/* : int */ = stringToBase256("A")/* = 65 (0x41) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn compile_time_functions_04_invalid_compile_time_function_calls() {
    // Ported from compile-time-functions.test: Invalid compile-time function calls.
    case_tolk_inlay_hints(
        r"
            const INVALID_ARG = stringCrc32(42); // Should not evaluate
            const NON_CONST_STR = someFunction();
            const INVALID_REF = stringCrc32(NON_CONST_STR); // Should not evaluate
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const INVALID_ARG/* : int */ = stringCrc32(42); // Should not evaluate
            const NON_CONST_STR = someFunction();
            const INVALID_REF/* : int */ = stringCrc32(NON_CONST_STR); // Should not evaluate

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn constant_values_01_constant_value_inlay_hints_basic() {
    // Ported from constant-values.test: Constant value inlay hints, basic.
    case_tolk_inlay_hints(
        r"
            const SIMPLE_CONST = 42 + 8;
            const HEX_CONST = 0xFF & 0x0F;
            const BOOL_CONST = !false;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const SIMPLE_CONST/* : int */ = 42 + 8/* = 50 (0x32) */;
            const HEX_CONST/* : int */ = 0xFF & 0x0F/* = 15 (0xF) */;
            const BOOL_CONST/* : bool */ = !false/* = true */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn constant_values_02_constant_value_inlay_hints_references() {
    // Ported from constant-values.test: Constant value inlay hints, references.
    case_tolk_inlay_hints(
        r"
            const BASE = 10;
            const DERIVED = BASE * 3;
            const COMPLEX = (BASE + DERIVED) / 2;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const BASE/* : int */ = 10;
            const DERIVED/* : int */ = BASE * 3/* = 30 (0x1E) */;
            const COMPLEX/* : int */ = (BASE + DERIVED) / 2/* = 20 (0x14) */;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn constant_values_03_constant_value_inlay_hints_circular_dependency() {
    // Ported from constant-values.test: Constant value inlay hints, circular dependency.
    case_tolk_inlay_hints(
        r"
            const CIRC_A = CIRC_B + 1;
            const CIRC_B = CIRC_A, 1;
            
            fun main() {
                return 0;
            }
        ",
        full_document_range(),
        expect![[r#"
            const CIRC_A/* : int */ = CIRC_B + 1;
            const CIRC_B = CIRC_A, 1;

            fun main()/* : int */ {
                return 0;
            }"#]],
    );
}

#[test]
fn enum_values_01_enum_member_value_inlay_hints_sequential_and_explicit_values() {
    // Ported from enum-values.test: Enum member value inlay hints, sequential and explicit values.
    case_tolk_inlay_hints(
        r"
            enum Color {
                Red,
                Green = 5,
                Blue,
                Negative = -1,
                Next,
                Hex = 0x10,
                Truthy = 10 > 0,
                Falsy = 10 < 0,
                AfterFalsy,
            }
        ",
        full_document_range(),
        expect![[r#"
            enum Color {
                Red/* = 0 */,
                Green = 5,
                Blue/* = 6 */,
                Negative = -1,
                Next/* = 0 */,
                Hex = 0x10/* = 16 */,
                Truthy = 10 > 0/* = -1 */,
                Falsy = 10 < 0/* = 0 */,
                AfterFalsy/* = 1 */,
            }"#]],
    );
}

#[test]
fn enum_values_02_enum_member_value_inlay_hints_constant_expressions_and_other_enum_member() {
    // Ported from enum-values.test: Enum member value inlay hints, constant expressions and other
    // enum members.
    case_tolk_inlay_hints(
        r"
            const BASE = 10;
            
            enum Other {
                Item = 3,
            }
            
            enum Color {
                Red = BASE + 1,
                Green,
                Blue = Other.Item + 2,
                Yellow,
            }
        ",
        full_document_range(),
        expect![[r#"
            const BASE/* : int */ = 10;

            enum Other {
                Item = 3,
            }

            enum Color {
                Red = BASE + 1/* = 11 */,
                Green/* = 12 */,
                Blue = Other.Item + 2/* = 5 */,
                Yellow/* = 6 */,
            }"#]],
    );
}

#[test]
fn enum_values_03_enum_member_value_inlay_hints_unsupported_initializers_stop_sequence() {
    // Ported from enum-values.test: Enum member value inlay hints, unsupported initializers stop
    // sequence.
    case_tolk_inlay_hints(
        r"
            fun notConst(): int {
                return 10;
            }
            
            enum Broken {
                A = 1,
                B = notConst(),
                C,
                D = 4,
                E,
            }
        ",
        full_document_range(),
        expect![[r#"
            fun notConst(): int {
                return 10;
            }

            enum Broken {
                A = 1,
                B = notConst(),
                C,
                D = 4,
                E/* = 5 */,
            }"#]],
    );
}

#[test]
fn enum_values_04_enum_member_value_inlay_hints_same_enum_references_stop_sequence() {
    // Ported from enum-values.test: Enum member value inlay hints, same enum references stop
    // sequence.
    case_tolk_inlay_hints(
        r"
            enum Codes {
                Base = 100,
                Next = Codes.Base + 1,
                After,
                Reset = 7,
                Last,
            }
        ",
        full_document_range(),
        expect![[r#"
            enum Codes {
                Base = 100,
                Next = Codes.Base + 1,
                After,
                Reset = 7,
                Last/* = 8 */,
            }"#]],
    );
}

#[test]
fn from_cell_slice_01_struct_fromcell_call() {
    // Ported from from-cell-slice.test: Struct fromCell call.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun some(cell: cell) {
                return Foo.fromCell(cell);
            }
            
            fun test(cell: cell) {
                val res = some(cell);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun some(cell: cell)/* : Foo */ {
                return Foo.fromCell(/* packedCell: */cell);
            }

            fun test(cell: cell)/* : void */ {
                val res/* : Foo */ = some(cell);
            }"#]],
    );
}

#[test]
fn from_cell_slice_02_struct_fromslice_call() {
    // Ported from from-cell-slice.test: Struct fromSlice call.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun some(slice: slice) {
                return Foo.fromSlice(slice);
            }
            
            fun test(slice: slice) {
                val res = some(slice);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun some(slice: slice)/* : Foo */ {
                return Foo.fromSlice(/* rawSlice: */slice);
            }

            fun test(slice: slice)/* : void */ {
                val res/* : Foo */ = some(slice);
            }"#]],
    );
}

#[test]
fn from_cell_slice_03_struct_fromslice_call_with_lazy() {
    // Ported from from-cell-slice.test: Struct fromSlice call with lazy.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun test(slice: slice) {
                val res = Foo.fromSlice(slice);
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun test(slice: slice)/* : void */ {
                val res/* : Foo */ = Foo.fromSlice(/* rawSlice: */slice);
            }"#]],
    );
}

#[test]
fn lambdas_01_lambda_param_type() {
    // Ported from lambdas.test: Lambda param type.
    case_tolk_inlay_hints(
        r"
            fun measure(cb: (int) -> int) {}
            
            fun foo() {
                measure(fun (x) {
                })
            }
        ",
        full_document_range(),
        expect![[r#"
            fun measure(cb: (int) -> int)/* : void */ {}

            fun foo()/* : void */ {
                measure(/* cb: */fun (x/* : int */) {
                })
            }"#]],
    );
}

#[test]
fn lambdas_02_lambda_param_types() {
    // Ported from lambdas.test: Lambda param types.
    case_tolk_inlay_hints(
        r"
            fun measure(cb: (int, slice) -> int) {}
            
            fun foo() {
                measure(fun (
                    x,
                    y,
                ) {
                })
            }
        ",
        full_document_range(),
        expect![[r#"
            fun measure(cb: (int, slice) -> int)/* : void */ {}

            fun foo()/* : void */ {
                measure(/* cb: */fun (
                    x/* : int */,
                    y/* : slice */,
                ) {
                })
            }"#]],
    );
}

#[test]
fn lambdas_03_lambda_with_generic_param_type() {
    // Ported from lambdas.test: Lambda with generic param type.
    case_tolk_inlay_hints(
        r"
            fun measure<T>(cb: (T) -> void): T {}
            
            fun foo() {
                val a = measure(fun (a: int) {
                })
            }
        ",
        full_document_range(),
        expect![[r#"
            fun measure<T>(cb: (T) -> void): T {}

            fun foo()/* : void */ {
                val a/* : int */ = measure(/* cb: */fun (a: int) {
                })
            }"#]],
    );
}

#[test]
fn method_id_01_get_method_id_hint() {
    // Ported from method-id.test: Get method id hint.
    case_tolk_inlay_hints(
        r"
            get fun data(): int {
                return 0
            }
        ",
        full_document_range(),
        expect![[r#"
            get/* (0x18762) */ fun data(): int {
                return 0
            }"#]],
    );
}

#[test]
fn method_id_02_get_method_id_hint_with_explicit_annotation() {
    // Ported from method-id.test: Get method id hint with explicit annotation.
    case_tolk_inlay_hints(
        r"
            @method_id(0x100)
            get fun data(): int {
                return 0
            }
        ",
        full_document_range(),
        expect![[r#"
            @method_id(0x100)
            get fun data(): int {
                return 0
            }"#]],
    );
}

#[test]
fn method_id_03_get_method_id_hint_with_explicit_empty_annotation() {
    // Ported from method-id.test: Get method id hint with explicit empty annotation.
    case_tolk_inlay_hints(
        r"
            @method_id()
            get fun data(): int {
                return 0
            }
        ",
        full_document_range(),
        expect![[r#"
            @method_id()
            get fun data(): int {
                return 0
            }"#]],
    );
}

#[test]
fn method_id_04_get_method_id_hint_with_explicit_different_annotation() {
    // Ported from method-id.test: Get method id hint with explicit different annotation.
    case_tolk_inlay_hints(
        r"
            @foo()
            get fun data(): int {
                return 0
            }
        ",
        full_document_range(),
        expect![[r#"
            @foo()
            get/* (0x18762) */ fun data(): int {
                return 0
            }"#]],
    );
}

#[test]
fn method_id_05_get_method_id_hint_for_test_get_method() {
    // Ported from method-id.test: Get method id hint for test get method.
    case_tolk_inlay_hints(
        r"
            get fun `test foo`(): int {
                return 0
            }
            get fun `test-foo`(): int {
                return 0
            }
            get fun `test_foo`(): int {
                return 0
            }
        ",
        full_document_range(),
        expect![[r#"
            get fun `test foo`(): int {
                return 0
            }
            get fun `test-foo`(): int {
                return 0
            }
            get fun `test_foo`(): int {
                return 0
            }"#]],
    );
}

#[test]
fn types_01_variable_type() {
    // Ported from types.test: Variable type.
    case_tolk_inlay_hints(
        r"
            fun test() {
                val some = 10;
            }
        ",
        full_document_range(),
        expect![[r#"
            fun test()/* : void */ {
                val some/* : int */ = 10;
            }"#]],
    );
}

#[test]
fn types_02_variable_type_for_struct_init() {
    // Ported from types.test: Variable type for struct init.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun test() {
                val some = Foo {}; // no type hint
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun test()/* : void */ {
                val some/* : Foo */ = Foo {}; // no type hint
            }"#]],
    );
}

#[test]
fn types_03_variable_type_for_struct_fromcell() {
    // Ported from types.test: Variable type for struct fromCell.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun test() {
                val some = Foo.fromCell(); // no type hint
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun test()/* : void */ {
                val some/* : Foo */ = Foo.fromCell(); // no type hint
            }"#]],
    );
}

#[test]
fn types_04_variable_type_for_struct_fromslice() {
    // Ported from types.test: Variable type for struct fromSlice.
    case_tolk_inlay_hints(
        r"
            struct Foo {}
            
            fun test() {
                val some = Foo.fromSlice(); // no type hint
            }
        ",
        full_document_range(),
        expect![[r#"
            struct Foo {}

            fun test()/* : void */ {
                val some/* : Foo */ = Foo.fromSlice(); // no type hint
            }"#]],
    );
}

#[test]
fn types_05_constant_type() {
    // Ported from types.test: Constant type.
    case_tolk_inlay_hints(
        r"
            const FOO = 10;
            
            fun test() {
            }
        ",
        full_document_range(),
        expect![[r#"
            const FOO/* : int */ = 10;

            fun test()/* : void */ {
            }"#]],
    );
}

#[test]
fn vars_01_empty_test() {
    // Ported from vars.test: Empty test.
    case_tolk_inlay_hints("", full_document_range(), expect![[""]]);
}

#[test]
fn vars_02_tensor_variable_declaration() {
    // Ported from vars.test: Tensor variable declaration.
    case_tolk_inlay_hints(
        r#"
            fun main() {
                val (
                    first,
                    second,
                    third
                ) = (100, true, "");
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                val (
                    first/* : int */,
                    second/* : bool */,
                    third/* : string */
                ) = (100, true, "");
            }"#]],
    );
}

#[test]
fn vars_03_tensor_variable_declaration_with_trailing_comma() {
    // Ported from vars.test: Tensor variable declaration with trailing comma.
    case_tolk_inlay_hints(
        r#"
            fun main() {
                val (
                    first,
                    second,
                    third,
                ) = (100, true, "");
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                val (
                    first/* : int */,
                    second/* : bool */,
                    third/* : string */,
                ) = (100, true, "");
            }"#]],
    );
}

#[test]
fn vars_04_tuple_variable_declaration() {
    // Ported from vars.test: Tuple variable declaration.
    case_tolk_inlay_hints(
        r#"
            fun main() {
                val [
                    first,
                    second,
                    third
                ] = [100, true, ""];
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                val [
                    first/* : int */,
                    second/* : bool */,
                    third/* : string */
                ] = [100, true, ""];
            }"#]],
    );
}

#[test]
fn vars_05_tuple_variable_declaration_with_trailing_comma() {
    // Ported from vars.test: Tuple variable declaration with trailing comma.
    case_tolk_inlay_hints(
        r#"
            fun main() {
                val [
                    first,
                    second,
                    third,
                ] = [100, true, ""];
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                val [
                    first/* : int */,
                    second/* : bool */,
                    third/* : string */,
                ] = [100, true, ""];
            }"#]],
    );
}

#[test]
fn vars_06_tensor_variable_declaration_with_function_init() {
    // Ported from vars.test: Tensor variable declaration with function init.
    case_tolk_inlay_hints(
        r"
            fun foo(): (int, slice, bool) {}
            
            fun main() {
                val (
                    first,
                    second,
                    third
                ) = foo();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo(): (int, slice, bool) {}

            fun main()/* : void */ {
                val (
                    first/* : int */,
                    second/* : slice */,
                    third/* : bool */
                ) = foo();
            }"#]],
    );
}

#[test]
fn vars_07_tuple_variable_declaration_with_function_init() {
    // Ported from vars.test: Tuple variable declaration with function init.
    case_tolk_inlay_hints(
        r"
            fun foo(): [int, slice, bool] {}
            
            fun main() {
                val [
                    first,
                    second,
                    third
                ] = foo();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo(): [int, slice, bool] {}

            fun main()/* : void */ {
                val [
                    first/* : int */,
                    second/* : slice */,
                    third/* : bool */
                ] = foo();
            }"#]],
    );
}

#[test]
fn vars_08_nested_tensor_and_tuple_variable_declaration_with_function_init() {
    // Ported from vars.test: Nested tensor and tuple variable declaration with function init.
    case_tolk_inlay_hints(
        r"
            fun foo(): (int, [[[[slice]]]], [bool, int]) {}
            
            fun main() {
                val (first,
                    [[[[second]]]],
                    [
                        third,
                        fourth
                    ]
                ) = foo();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo(): (int, [[[[slice]]]], [bool, int]) {}

            fun main()/* : void */ {
                val (first/* : int */,
                    [[[[second/* : slice */]]]],
                    [
                        third/* : bool */,
                        fourth/* : int */
                    ]
                ) = foo();
            }"#]],
    );
}

#[test]
fn vars_09_tensor_variable_declaration_with_variable_function_call() {
    // Ported from vars.test: Tensor variable declaration with variable function call.
    case_tolk_inlay_hints(
        r"
            fun foo(): (() -> (int, slice, bool)) {}
            
            fun main() {
                val getter = foo();
            
                val (
                    first,
                    second,
                    third
                ) = getter();
            }
        ",
        full_document_range(),
        expect![[r#"
            fun foo(): (() -> (int, slice, bool)) {}

            fun main()/* : void */ {
                val getter/* : () -> (int, slice, bool) */ = foo();

                val (
                    first/* : int */,
                    second/* : slice */,
                    third/* : bool */
                ) = getter();
            }"#]],
    );
}

#[test]
fn vars_10_try_catch_variable_hint() {
    // Ported from vars.test: Try catch variable hint.
    case_tolk_inlay_hints(
        r"
            fun main() {
                try {} catch (e) {}
            }
        ",
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                try {} catch (e/* : int */) {}
            }"#]],
    );
}

#[test]
fn vars_11_try_catch_variable_2_hint() {
    // Ported from vars.test: Try catch variable 2 hint.
    case_tolk_inlay_hints(
        r"
            fun main() {
                try {} catch (e, d) {}
            }
        ",
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                try {} catch (e/* : int */, d/* : unknown */) {}
            }"#]],
    );
}
