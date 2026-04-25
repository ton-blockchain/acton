mod common;

use crate::common::{check, check_with_width, check_without_trees};
use expect_test::expect;

#[test]
fn test_assignment() {
    check(
        "fun test() { x = 10; }",
        expect![[r"
                fun test() {
                    x = 10;
                }"]],
    );
}

#[test]
fn test_set_assignment() {
    check(
        "fun test() { x += 10; x -= 5; x *= 2; x /= 3; x %= 4; x &= 1; x |= 2; x ^= 3; x <<= 1; x >>= 2; }",
        expect![[r"
                fun test() {
                    x += 10;
                    x -= 5;
                    x *= 2;
                    x /= 3;
                    x %= 4;
                    x &= 1;
                    x |= 2;
                    x ^= 3;
                    x <<= 1;
                    x >>= 2;
                }"]],
    );
}

#[test]
fn test_binary_operator() {
    check(
        "fun test() { x = a + b - c * d / e % f & g | h ^ i << j >> k; }",
        expect![[r"
                fun test() {
                    x = a + b - c * d / e % f & g | h ^ i << j >> k;
                }"]],
    );
}

#[test]
fn test_binary_operator_with_comments() {
    check(
        r"
fun foo() {
    return 6 +
        // comment 1
        4 * 5 +
        3 + // comment 2
        2 +
        1;
}",
        expect![[r"
            fun foo() {
                return 6 +
                // comment 1
                4 * 5 +
                3 +
                // comment 2
                2 +
                1;
            }"]],
    );
}

#[test]
fn test_binary_operator_comment_immediately_after_operator() {
    check(
        r"
fun foo() {
    return 1 + // comment
        2;
}",
        expect![[r"
            fun foo() {
                return 1 +
                // comment
                2;
            }"]],
    );
}

#[test]
fn test_binary_operator_multiple_comments_immediately_after_operator() {
    check(
        r"
fun foo() {
    return 1 + // comment 1
        2 * 3 + // comment 2
        4;
}",
        expect![[r"
            fun foo() {
                return 1 +
                // comment 1
                2 * 3 +
                // comment 2
                4;
            }"]],
    );
}

#[test]
fn test_binary_operator_chain_with_comment_on_middle_operand() {
    check(
        r"
fun foo() {
    return 1 +
        2 // comment
        + 3;
}",
        expect![[r"
            fun foo() {
                return 1 + 2 // comment
                + 3;
            }"]],
    );
}

#[test]
fn test_null_coalescing_operator() {
    check(
        "fun test() { x = a ?? b; y = a ?? b ?? c; }",
        expect![[r"
                fun test() {
                    x = a ?? b;
                    y = a ?? b ?? c;
                }"]],
    );
}

#[test]
fn test_binary_operator_breaking() {
    // TODO:
    check_with_width(
        "fun test() { x = a + b + c + d; }",
        expect![[r"
                fun test() {
                    x = a + b + c +
                    d;
                }"]],
        20,
    );
}

#[test]
fn test_unary_operator() {
    check(
        "fun test() { x = -a; y = !b; z = ~c; }",
        expect![[r"
                fun test() {
                    x = -a;
                    y = !b;
                    z = ~c;
                }"]],
    );
}

#[test]
fn test_ternary_operator() {
    check(
        "fun test() { x = a ? b : c; }",
        expect![[r"
                fun test() {
                    x = a ? b : c;
                }"]],
    );
}

#[test]
fn test_ternary_operator_breaking() {
    check_with_width(
        "fun test() { x = long_condition ? long_consequence : long_alternative; }",
        expect![[r"
                fun test() {
                    x = long_condition
                        ? long_consequence
                        : long_alternative;
                }"]],
        30,
    );
}

#[test]
fn test_dot_access() {
    check(
        "fun test() { x = a.b; y = a.0; }",
        expect![[r"
                fun test() {
                    x = a.b;
                    y = a.0;
                }"]],
    );
}

#[test]
fn test_dot_access_breaking() {
    check_with_width(
        "fun test() { x = very_long_object_name.very_long_field_name; }",
        expect![[r"
            fun test() {
                x = very_long_object_name.very_long_field_name;
            }"]],
        30,
    );
}

#[test]
fn test_dot_access_for_struct_litral() {
    check_with_width(
        "fun test() { Foo { loooooooooong }.toCell() }",
        expect![[r"
                fun test() {
                    Foo {
                        loooooooooong,
                    }.toCell();
                }"]],
        20,
    );
}

#[test]
fn test_function_call() {
    check(
        "fun test() { foo(); bar(1); baz(1, 2); }",
        expect![[r"
                fun test() {
                    foo();
                    bar(1);
                    baz(1, 2);
                }"]],
    );
}

#[test]
fn test_function_call_mutate() {
    check(
        "fun test() { foo(mutate x, y); }",
        expect![[r"
                fun test() {
                    foo(mutate x, y);
                }"]],
    );
}

#[test]
fn test_function_call_breaking() {
    check_with_width(
        "fun test() { foo(arg1, arg2, arg3, arg4); }",
        expect![[r"
                fun test() {
                    foo(
                        arg1,
                        arg2,
                        arg3,
                        arg4,
                    );
                }"]],
        20,
    );
}

#[test]
fn test_single_long_string_argument_does_not_break() {
    check_with_width(
        r#"fun test() { log("This is a very very very very very long string argument"); }"#,
        expect![[r#"
                fun test() {
                    log("This is a very very very very very long string argument");
                }"#]],
        20,
    );
}

#[test]
fn test_object_literal() {
    check(
        "fun test() { x = Point { x: 10, y: 20 }; }",
        expect![[r"
                fun test() {
                    x = Point {
                        x: 10,
                        y: 20,
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_without_type() {
    check(
        "fun test() { x = { x: 10, y: 20 }; }",
        expect![[r"
                fun test() {
                    x = { x: 10, y: 20 };
                }"]],
    );
}

#[test]
fn test_object_literal_without_type_mixed_two_fields_stays_single_line() {
    check(
        "fun test() { x = { x, y: 20 }; }",
        expect![[r"
                fun test() {
                    x = { x, y: 20 };
                }"]],
    );
}

#[test]
fn test_single_typeless_object_call_argument_preserves_multiline_layout() {
    check(
        r"
            fun test() {
                val counter = Counter.fromStorage({
                    id: 0,
                    counter: 0,
                });
            }",
        expect![[r"
                fun test() {
                    val counter = Counter.fromStorage({
                        id: 0,
                        counter: 0,
                    });
                }"]],
    );
}

#[test]
fn test_single_typeless_object_call_argument_preserves_partial_multiline_layout() {
    check(
        r"
            fun test() {
                val contract = Counter.fromStorage({
                    id: 0, counter: 0,
                });
            }",
        expect![[r"
                fun test() {
                    val contract = Counter.fromStorage({
                        id: 0,
                        counter: 0,
                    });
                }"]],
    );
}

#[test]
fn test_single_typeless_object_call_argument_preserves_single_line_layout() {
    check_with_width(
        "fun test() { val counter = Counter.fromStorage({ id: 0, counter: 0 }); }",
        expect![[r"
                fun test() {
                    val counter = Counter.fromStorage({ id: 0, counter: 0 });
                }"]],
        40,
    );
}

#[test]
fn test_object_literal_without_type_shorthand_all_uses_default_threshold() {
    check(
        "fun test() { x = { x, y, z }; }",
        expect![[r"
                fun test() {
                    x = {
                        x,
                        y,
                        z,
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_shorthand() {
    check(
        "fun test() { x = Point { x, y }; }",
        expect![[r"
                fun test() {
                    x = Point { x, y };
                }"]],
    );
}

#[test]
fn test_object_literal_two_fields_mixed_forces_multiline() {
    check(
        "fun test() { x = Point { x, y: 20 }; }",
        expect![[r"
                fun test() {
                    x = Point {
                        x,
                        y: 20,
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_with_expr_and_field_with_same_name() {
    check_without_trees(
        "fun test() { x = Point { x: x, y: y }; }",
        expect![[r"
                fun test() {
                    x = Point { x, y };
                }"]],
    );
}

#[test]
fn test_object_literal_multiline() {
    check(
        "fun test() { x = Point { x: 10, y: 20, z: 30 }; }",
        expect![[r"
                fun test() {
                    x = Point {
                        x: 10,
                        y: 20,
                        z: 30,
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_multiline_with_empty_lines() {
    check(
        r"
                fun test() {
                    x = Point {
                        x: 10,

                        y: 20,

                        z: 30,
                    };
                }",
        expect![[r"
                fun test() {
                    x = Point {
                        x: 10,

                        y: 20,

                        z: 30,
                    };
                }"]],
    );
}

#[test]
fn test_tensor_expression() {
    check(
        "fun test() { x = (1, 2); y = (1); z = (); }",
        expect![[r"
                fun test() {
                    x = (1, 2);
                    y = (1);
                    z = ();
                }"]],
    );
}

#[test]
fn test_typed_tuple() {
    check(
        "fun test() { x = [1, 2]; y = [1]; z = []; }",
        expect![[r"
                fun test() {
                    x = [1, 2];
                    y = [1];
                    z = [];
                }"]],
    );
}

#[test]
fn test_typed_tuple_with_type() {
    check(
        "fun test() { x = array<int> [1, 2]; y = map<int, slice> []; z = []; }",
        expect![[r"
                fun test() {
                    x = array<int> [1, 2];
                    y = map<int, slice> [];
                    z = [];
                }"]],
    );
}

#[test]
fn test_complex_tuple() {
    check(
        "
            fun foo() {
                val x = 1;

                val   matrix   =   [
                    [1,   2,   3,   4,   5],
                    [6,   7,   8,   9,   10],
                    [11,  12,  13,  14,  15],
                    [16,  17,  18,  19,  20]
                ];

                val y = 2;
            }",
        expect![[r"
                fun foo() {
                    val x = 1;

                    val matrix = [
                        [1, 2, 3, 4, 5],
                        [6, 7, 8, 9, 10],
                        [11, 12, 13, 14, 15],
                        [16, 17, 18, 19, 20],
                    ];

                    val y = 2;
                }"]],
    );
}

#[test]
fn test_lambda_expression() {
    check(
        "fun test() { x = fun(a: int, b: int): int { return a + b; }; }",
        expect![[r"
                fun test() {
                    x = fun(a: int, b: int): int {
                        return a + b;
                    };
                }"]],
    );
}

#[test]
fn test_cast_as_operator() {
    check(
        "fun test() { x = a as int; }",
        expect![[r"
                fun test() {
                    x = a as int;
                }"]],
    );
}

#[test]
fn test_is_type_operator() {
    check(
        "fun test() { x = a is int; y = a !is int; }",
        expect![[r"
                fun test() {
                    x = a is int;
                    y = a !is int;
                }"]],
    );
}

#[test]
fn test_call_arguments_comments() {
    check(
        "fun main() {
                foo(
                    // leading
                    a, // inline
                    b
                    // trailing
                );
            }",
        expect![[r"
                fun main() {
                    foo(
                        // leading
                        a, // inline
                        b,
                        // trailing
                    );
                }"]],
    );
}

#[test]
fn test_tuple_comments() {
    check(
        "fun main() {
                val x = [
                    // leading
                    1, // inline
                    2
                    // trailing
                ];
            }",
        expect![[r"
                fun main() {
                    val x = [
                        // leading
                        1, // inline
                        2,
                        // trailing
                    ];
                }"]],
    );
}

#[test]
fn test_tensor_comments() {
    check(
        "fun main() {
                val x = (
                    // leading
                    1, // inline
                    2
                    // trailing
                );
            }",
        expect![[r"
                fun main() {
                    val x = (
                        // leading
                        1, // inline
                        2,
                        // trailing
                    );
                }"]],
    );
}

#[test]
fn test_not_null_operator() {
    check(
        "fun test() { x = a!; }",
        expect![[r"
                fun test() {
                    x = a!;
                }"]],
    );
}

#[test]
fn test_lazy_expression() {
    check(
        "fun test() { x = lazy Foo.fromCell(cell); }",
        expect![[r"
                fun test() {
                    x = lazy Foo.fromCell(cell);
                }"]],
    );
}

#[test]
fn test_generic_instantiation() {
    check(
        "fun test() { x = foo<int, slice>; }",
        expect![[r"
                fun test() {
                    x = foo<int, slice>;
                }"]],
    );
}

#[test]
fn test_complex_binary_operators() {
    check(
        "fun test() { x = a + b * c - d / e % f; }",
        expect![[r"
                fun test() {
                    x = a + b * c - d / e % f;
                }"]],
    );
}

#[test]
fn test_bitwise_operators() {
    check(
        "fun test() { x = a & b | c ^ d; y = a << b >> c; }",
        expect![[r"
                fun test() {
                    x = a & b | c ^ d;
                    y = a << b >> c;
                }"]],
    );
}

#[test]
fn test_comparison_operators() {
    check(
        "fun test() { x = a == b != c < d <= e > f >= g <=> h; }",
        expect![[r"
                fun test() {
                    x = a == b != c < d <= e > f >= g <=> h;
                }"]],
    );
}

#[test]
fn test_logical_operators() {
    check(
        "fun test() { x = a && b || c; y = !a && !b; }",
        expect![[r"
                fun test() {
                    x = a && b || c;
                    y = !a && !b;
                }"]],
    );
}

#[test]
fn test_arithmetic_special_operators() {
    check(
        "fun test() { x = a ~/ b ^/ c; }",
        expect![[r"
                fun test() {
                    x = a ~/ b ^/ c;
                }"]],
    );
}

#[test]
fn test_mixed_operators_complex() {
    check(
        "fun test() { x = (a + b) * c << d & e | f ^ g && h || i == j; }",
        expect![[r"
                fun test() {
                    x = (a + b) * c << d & e | f ^ g && h || i == j;
                }"]],
    );
}

#[test]
fn test_unary_multiple() {
    check(
        "fun test() { x = -+~-!a; y = !!b; }",
        expect![[r"
                fun test() {
                    x = -+~-!a;
                    y = !!b;
                }"]],
    );
}

#[test]
fn test_null_checks() {
    check(
        "fun test() { x = a != null && b is int; }",
        expect![[r"
                fun test() {
                    x = a != null && b is int;
                }"]],
    );
}

#[test]
fn test_number_literals() {
    check(
        "fun test() { x = 42; z = 0xFF; w = 0b1010; }",
        expect![[r"
                fun test() {
                    x = 42;
                    z = 0xFF;
                    w = 0b1010;
                }"]],
    );
}

#[test]
fn test_string_literals() {
    check(
        r#"fun test() { x = "hello"; y = """with
new
line"""; }"#,
        expect![[r#"
                fun test() {
                    x = "hello";
                    y = """with
                new
                line""";
                }"#]],
    );
}

#[test]
fn test_boolean_literals() {
    check(
        "fun test() { x = true; y = false; z = !true; }",
        expect![[r"
                fun test() {
                    x = true;
                    y = false;
                    z = !true;
                }"]],
    );
}

#[test]
fn test_null_literal() {
    check(
        "fun test() { x = null; y = a == null; }",
        expect![[r"
                fun test() {
                    x = null;
                    y = a == null;
                }"]],
    );
}

#[test]
fn test_numeric_index() {
    check(
        "fun test() { x = a.0; y = b.1; z = c.42; }",
        expect![[r"
                fun test() {
                    x = a.0;
                    y = b.1;
                    z = c.42;
                }"]],
    );
}

#[test]
fn test_identifiers() {
    check(
        "fun test() { x = variable_name; y = _private; z = camelCase; w = snake_case; }",
        expect![[r"
                fun test() {
                    x = variable_name;
                    y = _private;
                    z = camelCase;
                    w = snake_case;
                }"]],
    );
}

#[test]
fn test_function_call_complex() {
    check(
        "fun test() { result = calculate(a + b, c * d, func(e, f)); }",
        expect![[r"
                fun test() {
                    result = calculate(a + b, c * d, func(e, f));
                }"]],
    );
}

#[test]
fn test_function_call_with_mutate() {
    check(
        "fun test() { foo(mutate x, mutate y, z); bar(mutate a.b, c); }",
        expect![[r"
                fun test() {
                    foo(mutate x, mutate y, z);
                    bar(mutate a.b, c);
                }"]],
    );
}

#[test]
fn test_method_calls() {
    check(
        "fun test() { x = obj.method(); y = a.b.c.method(arg); }",
        expect![[r"
                fun test() {
                    x = obj.method();
                    y = a.b.c.method(arg);
                }"]],
    );
}

#[test]
fn test_nested_function_calls() {
    check(
        "fun test() { x = outer(inner1(), inner2(a, b)); }",
        expect![[r"
                fun test() {
                    x = outer(inner1(), inner2(a, b));
                }"]],
    );
}

#[test]
fn test_function_call_with_literals() {
    check(
        r#"fun test() { x = create_point(10, 20, "origin"); y = sum(1, 2, 3, 4, 5); }"#,
        expect![[r#"
                fun test() {
                    x = create_point(10, 20, "origin");
                    y = sum(1, 2, 3, 4, 5);
                }"#]],
    );
}

#[test]
fn test_function_call_with_newline_after_open_paren_stays_multiline() {
    check(
        "fun test() { foo(\n            a, b); }",
        expect![[r"
                fun test() {
                    foo(
                        a,
                        b,
                    );
                }"]],
    );
}

#[test]
fn test_function_call_with_single_internal_newline_stays_multiline() {
    check(
        "fun test() { foo(a,\n            b); }",
        expect![[r"
                fun test() {
                    foo(
                        a,
                        b,
                    );
                }"]],
    );
}

#[test]
fn test_function_call_with_newline_before_close_paren_stays_multiline() {
    check(
        "fun test() { foo(a, b\n        ); }",
        expect![[r"
                fun test() {
                    foo(
                        a,
                        b,
                    );
                }"]],
    );
}

#[test]
fn test_single_object_call_argument_with_top_level_newline_stays_multiline() {
    check(
        r"
            fun test() {
                val counter = Counter.fromStorage(
                    { id: 0, counter: 0 });
            }",
        expect![[r"
                fun test() {
                    val counter = Counter.fromStorage(
                        { id: 0, counter: 0 },
                    );
                }"]],
    );
}

#[test]
fn test_single_string_call_argument_with_top_level_newline_stays_multiline() {
    check(
        r#"
            fun test() {
                log(
                    "hello");
            }"#,
        expect![[r#"
                fun test() {
                    log(
                        "hello",
                    );
                }"#]],
    );
}

#[test]
fn test_function_call_breaking_long() {
    check_with_width(
        "fun test() { very_long_function_name(argument_one, argument_two, argument_three, argument_four, argument_five); }",
        expect![[r"
                fun test() {
                    very_long_function_name(
                        argument_one,
                        argument_two,
                        argument_three,
                        argument_four,
                        argument_five,
                    );
                }"]],
        50,
    );
}

#[test]
fn test_single_lambda_call_argument_stays_after_open_paren() {
    check_with_width(
        "fun test() { nums.map<int>(fun(x: int): int { return x * x; }); }",
        expect![[r"
                fun test() {
                    nums.map<int>(fun(x: int): int {
                        return x * x;
                    });
                }"]],
        100,
    );
}

#[test]
fn test_single_lambda_call_argument_with_top_level_newline_stays_multiline() {
    check_with_width(
        r"
            fun test() {
                nums.map<int>(
                    fun(x: int): int { return x * x; });
            }",
        expect![[r"
                fun test() {
                    nums.map<int>(
                        fun(x: int): int {
                            return x * x;
                        },
                    );
                }"]],
        100,
    );
}

#[test]
fn test_lambda_call_argument_forces_multiline_argument_list() {
    check_with_width(
        "fun test() { foo(a, fun(x: int): bool { return x > 0; }); }",
        expect![[r"
                fun test() {
                    foo(
                        a,
                        fun(x: int): bool {
                            return x > 0;
                        },
                    );
                }"]],
        100,
    );
}

#[test]
fn test_method_call_with_top_level_newline_stays_multiline() {
    check(
        r"
            fun test() {
                x.foo(
                    a, b);
            }",
        expect![[r"
                fun test() {
                    x.foo(
                        a,
                        b,
                    );
                }"]],
    );
}

#[test]
fn test_lambda_call_argument_in_method_chain_with_generics() {
    check_with_width(
        "fun test() { val squared = nums.filter(fun(x: int): bool { return x % 2 == 0; }).map<int>(fun(x: int): int { return x * x; }); }",
        expect![[r"
                fun test() {
                    val squared = nums
                        .filter(fun(x: int): bool {
                            return x % 2 == 0;
                        })
                        .map<int>(fun(x: int): int {
                            return x * x;
                        });
                }"]],
        80,
    );
}

#[test]
fn test_chain_continues_after_generic_lambda_call() {
    check_with_width(
        "fun test() { val size = nums.map<int>(fun(x: int): int { return x * x; }).size(); }",
        expect![[r"
                fun test() {
                    val size = nums
                        .map<int>(fun(x: int): int {
                            return x * x;
                        })
                        .size();
                }"]],
        60,
    );
}

#[test]
fn test_method_chain_with_multiple_arguments_and_lambda_breaks_cleanly() {
    check_with_width(
        "fun test() { val sum = nums.fold(0, fun(acc: int, x: int): int { return acc + x; }); }",
        expect![[r"
                fun test() {
                    val sum = nums.fold(
                        0,
                        fun(acc: int, x: int): int {
                            return acc + x;
                        },
                    );
                }"]],
        80,
    );
}

#[test]
fn test_multiple_lambda_arguments_force_multiline_layout() {
    check_with_width(
        "fun test() { foo(fun(x: int): int { return x; }, fun(y: int): int { return y + 1; }); }",
        expect![[r"
                fun test() {
                    foo(
                        fun(x: int): int {
                            return x;
                        },
                        fun(y: int): int {
                            return y + 1;
                        },
                    );
                }"]],
        100,
    );
}

#[test]
fn test_object_literal_field_lambda_keeps_existing_layout() {
    check_with_width(
        "fun test() { expect(res).toHaveSuccessfulTx({ exitCode: fun(code: int32): bool { return code != 0; }, success: true }); }",
        expect![[r"
                fun test() {
                    expect(res).toHaveSuccessfulTx({ exitCode: fun(code: int32): bool {
                            return code != 0;
                        }, success: true });
                }"]],
        100,
    );
}

#[test]
fn test_empty_match_expression() {
    check(
        "fun test() { match (1) {}; }",
        expect![[r"
                fun test() {
                    match (1) {}
                }"]],
    );
}

#[test]
fn test_match_expression_simple() {
    check(
        "fun test() { x = match (value) { int => 1, string => 2, else => 0 }; }",
        expect![[r"
                fun test() {
                    x = match (value) {
                        int => 1,
                        string => 2,
                        else => 0,
                    };
                }"]],
    );
}

#[test]
fn test_match_expression_with_empty_lines() {
    check(
        r"
                fun test() {
                    x = match (value) {
                        int => 1,

                        string => 2,

                        else => 0,
                    };
                }",
        expect![[r"
                fun test() {
                    x = match (value) {
                        int => 1,

                        string => 2,

                        else => 0,
                    };
                }"]],
    );
}

#[test]
fn test_match_expression_with_comments() {
    check(
        r"
                fun test() {
                    x = match (value) {
                        // leading comment
                        int => 1 // inline comment
                        // leading comment 2
                        string => 2, // inline comment 2
                        // leading comment 3
                        else => 0,
                        // trailing comment 3
                    };
                }",
        expect![[r"
                fun test() {
                    x = match (value) {
                        // leading comment
                        int => 1,    // inline comment
                        // leading comment 2
                        string => 2, // inline comment 2
                        // leading comment 3
                        else => 0,
                        // trailing comment 3
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_with_comments_alignment() {
    check_with_width(
        r"fun test() {
    x = MyStruct {
        field1: 1, // comment 1
        longField2: 2, // comment 2
    };
}",
        expect![[r"
                fun test() {
                    x = MyStruct {
                        field1: 1,     // comment 1
                        longField2: 2, // comment 2
                    };
                }"]],
        100,
    );
}

#[test]
fn test_match_expression_with_comments_alignment() {
    check_with_width(
        r"fun test() {
    val restoreAmount = match (msg) {
        InternalTransferStep => msg.jettonAmount, // safe to fetch jettonAmount, because
        BurnNotificationForMinter => msg.jettonAmount, // it's in the beginning of a message
    };
}",
        expect![[r"
                fun test() {
                    val restoreAmount = match (msg) {
                        InternalTransferStep => msg.jettonAmount,      // safe to fetch jettonAmount, because
                        BurnNotificationForMinter => msg.jettonAmount, // it's in the beginning of a message
                    };
                }"]],
        100,
    );
}

#[test]
fn test_match_expression_with_blocks() {
    check(
        "fun test() { result = match (x) { 1 => { return 1; }, 2 => { return 2; }, else => { return 0; } }; }",
        expect![[r"
                fun test() {
                    result = match (x) {
                        1 => {
                            return 1;
                        }
                        2 => {
                            return 2;
                        }
                        else => {
                            return 0;
                        }
                    };
                }"]],
    );
}

#[test]
fn test_match_expression_with_expressions() {
    check(
        "fun test() { x = match (a) { 1 => a + 1, 2 => a * 2, else => 0 }; }",
        expect![[r"
                fun test() {
                    x = match (a) {
                        1 => a + 1,
                        2 => a * 2,
                        else => 0,
                    };
                }"]],
    );
}

#[test]
fn test_match_expression_complex_patterns() {
    check(
        "fun test() { x = match (data) { Point => data.x, else => -1 }; }",
        expect![[r"
                fun test() {
                    x = match (data) {
                        Point => data.x,
                        else => -1,
                    };
                }"]],
    );
}

#[test]
fn test_match_expression_with_local_vars() {
    check(
        "fun test() { x = match (val a = get_value()) { int => a + b, else => 0 }; }",
        expect![[r"
                fun test() {
                    x = match (val a = get_value()) {
                        int => a + b,
                        else => 0,
                    };
                }"]],
    );
}

#[test]
fn test_match_expression_nested() {
    check(
        "fun test() { x = match (outer) { int => match (inner) { 1 => true, else => false }, else => null }; }",
        expect![[r"
                fun test() {
                    x = match (outer) {
                        int => match (inner) {
                            1 => true,
                            else => false,
                        },
                        else => null,
                    };
                }"]],
    );
}

#[test]
fn test_lambda_simple() {
    check(
        "fun test() { x = fun(a: int, b: int): int { return a + b; }; }",
        expect![[r"
                fun test() {
                    x = fun(a: int, b: int): int {
                        return a + b;
                    };
                }"]],
    );
}

#[test]
fn test_lambda_without_types() {
    check(
        "fun test() { x = fun(a, b) { return a + b; }; }",
        expect![[r"
                fun test() {
                    x = fun(a, b) {
                        return a + b;
                    };
                }"]],
    );
}

#[test]
fn test_lambda_single_param() {
    check(
        "fun test() { x = fun(x: int): int { return x * 2; }; }",
        expect![[r"
                fun test() {
                    x = fun(x: int): int {
                        return x * 2;
                    };
                }"]],
    );
}

#[test]
fn test_lambda_no_params() {
    check(
        "fun test() { x = fun(): int { return 42; }; }",
        expect![[r"
                fun test() {
                    x = fun(): int {
                        return 42;
                    };
                }"]],
    );
}

#[test]
fn test_lambda_with_mutate() {
    check(
        "fun test() { x = fun(mutate a: int, b: int) { a = a + b; return a; }; }",
        expect![[r"
                fun test() {
                    x = fun(mutate a: int, b: int) {
                        a = a + b;
                        return a;
                    };
                }"]],
    );
}

#[test]
fn test_lambda_complex_body() {
    check(
        "fun test() { x = fun(a: int, b: int): int { if (a > b) { return a; } else { return b; } }; }",
        expect![[r"
                fun test() {
                    x = fun(a: int, b: int): int {
                        if (a > b) {
                            return a;
                        } else {
                            return b;
                        }
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_typed() {
    check(
        "fun test() { x = Point { x: 10, y: 20 }; }",
        expect![[r"
                fun test() {
                    x = Point {
                        x: 10,
                        y: 20,
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_shorthand_all() {
    check(
        "fun test() { x = Point { x, y, z }; }",
        expect![[r"
                fun test() {
                    x = Point { x, y, z };
                }"]],
    );
}

#[test]
fn test_object_literal_shorthand_all_breaks_by_width() {
    check_with_width(
        "fun test() { x = Point { firstVeryLongFieldName, secondVeryLongFieldName, thirdVeryLongFieldName }; }",
        expect![[r"
                fun test() {
                    x = Point {
                        firstVeryLongFieldName,
                        secondVeryLongFieldName,
                        thirdVeryLongFieldName,
                    };
                }"]],
        40,
    );
}

#[test]
fn test_object_literal_mixed() {
    check(
        "fun test() { x = Config { enabled: true, name, value: 42 }; }",
        expect![[r"
                fun test() {
                    x = Config {
                        enabled: true,
                        name,
                        value: 42,
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_mixe_with_comments() {
    check(
        r"
                fun test() {
                    x = Config {
                        // leading comment
                        enabled: true, // inline comment
                        // leading comment 2
                        name, // inline comment
                        value: 42,
                        // trailing comment
                    };
                }",
        expect![[r"
                fun test() {
                    x = Config {
                        // leading comment
                        enabled: true, // inline comment
                        // leading comment 2
                        name,          // inline comment
                        value: 42,
                        // trailing comment
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_empty() {
    check(
        "fun test() { x = Empty {}; }",
        expect![[r"
                fun test() {
                    x = Empty {};
                }"]],
    );
}

#[test]
fn test_object_literal_single_field() {
    check(
        "fun test() { x = Singleton { value: 1 }; }",
        expect![[r"
                fun test() {
                    x = Singleton { value: 1 };
                }"]],
    );
}

#[test]
fn test_object_literal_with_expressions() {
    check(
        "fun test() { x = Point { x: a + b, y: c * 2, z: func() }; }",
        expect![[r"
                fun test() {
                    x = Point {
                        x: a + b,
                        y: c * 2,
                        z: func(),
                    };
                }"]],
    );
}

#[test]
fn test_object_literal_breaking() {
    check_with_width(
        "fun test() { x = VeryLongTypeName { very_long_field_name: very_long_expression_value, another_field: another_value }; }",
        expect![[r"
                fun test() {
                    x = VeryLongTypeName {
                        very_long_field_name: very_long_expression_value,
                        another_field: another_value,
                    };
                }"]],
        40,
    );
}

#[test]
fn test_tensor_expressions() {
    check(
        "fun test() { x = (1, 2); y = (1); z = (); w = (a, b, c); }",
        expect![[r"
                fun test() {
                    x = (1, 2);
                    y = (1);
                    z = ();
                    w = (a, b, c);
                }"]],
    );
}

#[test]
fn test_tensor_with_expressions() {
    check(
        "fun test() { x = (a + b, c * d, func()); y = (1,); }",
        expect![[r"
                fun test() {
                    x = (a + b, c * d, func());
                    y = (1);
                }"]],
    );
}

#[test]
fn test_tensor_breaking() {
    check_with_width(
        "fun test() { x = (very_long_expression_one, very_long_expression_two, very_long_expression_three); }",
        expect![[r"
                fun test() {
                    x = (
                        very_long_expression_one,
                        very_long_expression_two,
                        very_long_expression_three,
                    );
                }"]],
        40,
    );
}

#[test]
fn test_typed_tuples() {
    check(
        "fun test() { x = [1, 2]; y = [1]; z = []; w = [a, b, c]; }",
        expect![[r"
                fun test() {
                    x = [1, 2];
                    y = [1];
                    z = [];
                    w = [a, b, c];
                }"]],
    );
}

#[test]
fn test_typed_tuples_with_expressions() {
    check(
        "fun test() { x = [a + 1, b * 2, func(c)]; y = [single_element]; }",
        expect![[r"
                fun test() {
                    x = [a + 1, b * 2, func(c)];
                    y = [single_element];
                }"]],
    );
}

#[test]
fn test_typed_tuples_breaking() {
    check_with_width(
        "fun test() { x = [very_long_first_element, very_long_second_element, very_long_third_element]; }",
        expect![[r"
                fun test() {
                    x = [
                        very_long_first_element,
                        very_long_second_element,
                        very_long_third_element,
                    ];
                }"]],
        40,
    );
}

#[test]
fn test_generic_instantiation_with_function_calls() {
    check(
        "fun test() { x = create_map<string, int>(); y = List<int>.empty(); }",
        expect![[r"
                fun test() {
                    x = create_map<string, int>();
                    y = List<int>.empty();
                }"]],
    );
}

#[test]
fn test_generic_instantiation_complex_types() {
    check(
        "fun test() { x = Dict<string, int>; }",
        expect![[r"
                fun test() {
                    x = Dict<string, int>;
                }"]],
    );
}

#[test]
fn test_deeply_nested_expressions() {
    check(
        "fun test() { x = a.b.c.d.e.f(); y = (a + b).c.d.e(); }",
        expect![[r"
                fun test() {
                    x = a.b.c.d.e.f();
                    y = (a + b).c.d.e();
                }"]],
    );
}

#[test]
fn test_complex_expression_combination() {
    check(
        "fun test() { x = func(a + b, c * d).field.0.method(e ? f : g); }",
        expect![[r"
                fun test() {
                    x = func(a + b, c * d).field.0.method(e ? f : g);
                }"]],
    );
}

#[test]
fn test_complex_expression_combination_with_breaking() {
    // TODO
    check_with_width(
        "fun test() { x = func(a + b, c * d).field.0.method(e ? f : g); }",
        expect![[r"
                fun test() {
                    x = func(
                        a + b,
                        c * d,
                    )
                        .field
                        .0
                        .method(
                            e
                                ? f
                                : g,
                        );
                }"]],
        20,
    );

    check_with_width(
        "fun test() { x = func(a + b, c * d).field.0.method(e ? f : g); }",
        expect![[r"
            fun test() {
                x = func(a + b, c * d)
                    .field
                    .0
                    .method(e ? f : g);
            }"]],
        40,
    );

    check_with_width(
        r"
        fun test() {
            expect(equalAddressArrays(
                mapToAddressArray(after.signersMap()),
                mapToAddressArray(before.signersMap())
            ))
                .toEqual(true);
        }
        ",
        expect![[r"
            fun test() {
                expect(
                    equalAddressArrays(
                        mapToAddressArray(after.signersMap()),
                        mapToAddressArray(before.signersMap()),
                    ),
                ).toEqual(true);
            }"]],
        100,
    );

    check_with_width(
        r"
        fun test() {
            expect(vote).toHaveSuccessfulTx<ApproveAccepted>({
                from: fixture.order.address,
                to: signerAddress,
            });
        }
        ",
        expect![[r"
            fun test() {
                expect(vote).toHaveSuccessfulTx<ApproveAccepted>({
                    from: fixture.order.address,
                    to: signerAddress,
                });
            }"]],
        100,
    );

    check_with_width(
        r"
        fun test() {
            expect(equalAddressArrays(mapToAddressArray(after1.signersMap()), mapToAddressArray(before1.signersMap()))).toEqual(true);
        }
        ",
        expect![[r"
            fun test() {
                expect(
                    equalAddressArrays(
                        mapToAddressArray(after1.signersMap()),
                        mapToAddressArray(before1.signersMap()),
                    ),
                ).toEqual(true);
            }"]],
        100,
    );

    check_with_width(
        r"
        fun test() {
            val execBody = ExecuteOrderRequest {
                queryId: 0,
                orderSeqno: legitData.nextOrderSeqno as uint256,
                expirationDate: 0xffffffffffff,
                approvalsNum: 255,
                signersHash: legitSignersHash,
                order: evilPayload,
            }.toCell();
        }
        ",
        expect![[r"
            fun test() {
                val execBody = ExecuteOrderRequest {
                    queryId: 0,
                    orderSeqno: legitData.nextOrderSeqno as uint256,
                    expirationDate: 0xffffffffffff,
                    approvalsNum: 255,
                    signersHash: legitSignersHash,
                    order: evilPayload,
                }.toCell();
            }"]],
        100,
    );

    check_with_width(
        r#"
        fun test() {
            var i = 0;
            while (i < 253) {
                rootOrder.set(
                    i as uint8,
                    makeTransferAction(
                        ctx.deployer.address,
                        ton("0.01"),
                        i,
                        SEND_MODE_PAY_FEES_SEPARATELY,
                        true,
                    ).toCell(),
                );
                i += 1;
            }
        }
        "#,
        expect![[r#"
            fun test() {
                var i = 0;
                while (i < 253) {
                    rootOrder.set(
                        i as uint8,
                        makeTransferAction(
                            ctx.deployer.address,
                            ton("0.01"),
                            i,
                            SEND_MODE_PAY_FEES_SEPARATELY,
                            true,
                        ).toCell(),
                    );
                    i += 1;
                }
            }"#]],
        100,
    );

    check_with_width(
        r#"
        fun test() {
             val chained = beginCell().storeSlice("a".beginParse()).storeRef(beginCell().storeSlice("p"
                .beginParse())
                .storeRef(beginCell().storeSlice("prove".beginParse()).endCell())
                .endCell())
                .endCell();
        }
        "#,
        expect![[r#"
            fun test() {
                val chained = beginCell()
                    .storeSlice("a".beginParse())
                    .storeRef(
                        beginCell()
                            .storeSlice("p".beginParse())
                            .storeRef(beginCell().storeSlice("prove".beginParse()).endCell())
                            .endCell(),
                    )
                    .endCell();
            }"#]],
        100,
    );

    check_with_width(
        r"
        fun test() {
             expect(containsMessageWithOpcode(secondActions, ApproveAccepted.__getDeclaredPackPrefix()))
                .toEqual(true);
             expect(containsMessageWithOpcode(secondActions, ExecuteOrderRequest.__getDeclaredPackPrefix()))
                .toEqual(true);
        }
        ",
        expect![[r"
            fun test() {
                expect(
                    containsMessageWithOpcode(secondActions, ApproveAccepted.__getDeclaredPackPrefix()),
                ).toEqual(true);
                expect(
                    containsMessageWithOpcode(secondActions, ExecuteOrderRequest.__getDeclaredPackPrefix()),
                ).toEqual(true);
            }"]],
        100,
    );
    check_with_width(
        r#"
        fun test() {
              val changedOrder = singleActionOrder(makeTransferAction(
                  randomAddress("changed_order"),
                  DEFAULT_TRANSFER_VALUE,
                  777777
              ) as GenericOrderAction);
        }
        "#,
        expect![[r#"
            fun test() {
                val changedOrder = singleActionOrder(
                    makeTransferAction(
                        randomAddress("changed_order"),
                        DEFAULT_TRANSFER_VALUE,
                        777777,
                    ) as GenericOrderAction,
                );
            }"#]],
        100,
    );
    check_with_width(
        r#"
        fun test() {
              val chain = beginCell().storeSlice("a".beginParse()).storeRef(beginCell().storeSlice("p"
                .beginParse())
                .storeRef(beginCell().storeSlice("p".beginParse()).storeRef(beginCell().storeSlice("r"
                .beginParse())
                .storeRef(beginCell().storeSlice("o".beginParse()).storeRef(beginCell().storeSlice("v"
                .beginParse())
                .storeRef(beginCell().storeSlice("e".beginParse()).endCell())
                .endCell())
                .endCell())
                .endCell())
                .endCell())
                .endCell())
                .endCell();
        }
        "#,
        expect![[r#"
            fun test() {
                val chain = beginCell()
                    .storeSlice("a".beginParse())
                    .storeRef(
                        beginCell()
                            .storeSlice("p".beginParse())
                            .storeRef(
                                beginCell()
                                    .storeSlice("p".beginParse())
                                    .storeRef(
                                        beginCell()
                                            .storeSlice("r".beginParse())
                                            .storeRef(
                                                beginCell()
                                                    .storeSlice("o".beginParse())
                                                    .storeRef(
                                                        beginCell()
                                                            .storeSlice("v".beginParse())
                                                            .storeRef(
                                                                beginCell()
                                                                    .storeSlice("e".beginParse())
                                                                    .endCell(),
                                                            )
                                                            .endCell(),
                                                    )
                                                    .endCell(),
                                            )
                                            .endCell(),
                                    )
                                    .endCell(),
                            )
                            .endCell(),
                    )
                    .endCell();
            }"#]],
        100,
    );
    check_with_width(
        r"
        fun test() {
              {
                   assert (
                       msg.signersHash == storage.signers.hashCell() &&
                       msg.approvalsNum >= storage.threshold
                   ) throw ERROR_SIGNERS_OUTDATED;
              }
        }
        ",
        expect![[r"
            fun test() {
                {
                    assert (
                        msg.signersHash == storage.signers.hashCell() &&
                        msg.approvalsNum >= storage.threshold
                    ) throw ERROR_SIGNERS_OUTDATED;
                }
            }"]],
        80,
    );
    check_with_width(
        r"
        fun test() {
              {
                   {
                       val (signerIndex, foundSigner) = storage.remaining.signers.findSignerByAddress(in
                       .senderAddress);
                   }
              }
        }
        ",
        expect![[r"
            fun test() {
                {
                    {
                        val (signerIndex, foundSigner) = storage
                            .remaining
                            .signers
                            .findSignerByAddress(in.senderAddress);
                    }
                }
            }"]],
        80,
    );
    check_with_width(
        r"
        fun test() {
              val forwardFees = calculateForwardFee(
                  BASECHAIN,
                  INIT_ORDER_BIT_OVERHEAD + orderBits + signersBits,
                  INIT_ORDER_CELL_OVERHEAD + orderCells + signersCells
              ) +
              calculateForwardFee(
                  BASECHAIN,
                  EXECUTE_ORDER_BIT_OVERHEAD + orderBits,
                  EXECUTE_ORDER_CELL_OVERHEAD + orderCells
              );
        }
        ",
        expect![[r"
            fun test() {
                val forwardFees = calculateForwardFee(
                    BASECHAIN,
                    INIT_ORDER_BIT_OVERHEAD + orderBits + signersBits,
                    INIT_ORDER_CELL_OVERHEAD + orderCells + signersCells,
                ) +
                calculateForwardFee(
                    BASECHAIN,
                    EXECUTE_ORDER_BIT_OVERHEAD + orderBits,
                    EXECUTE_ORDER_CELL_OVERHEAD + orderCells,
                );
            }"]],
        80,
    );
    check_with_width(
        r"
        fun test() {
              if (count > HIGHLOAD_MAX_INLINE_ACTIONS) {
                  val chained = self.packActionsRange(
                      messages,
                      start + HIGHLOAD_CHAIN_CUT,
                      count - HIGHLOAD_CHAIN_CUT,
                      value,
                      queryId
                  );
                  val chainMode = value > 0 ? SEND_MODE_PAY_FEES_SEPARATELY : SEND_MODE_CARRY_ALL_BALANCE;

                  var head = packSendActionsToCell(messages, start, HIGHLOAD_CHAIN_CUT) as Cell<OutList>;
                  head = OutList {
                      prev: head,
                      action: OutActionSendMessage { mode: chainMode as uint8, outMsg: chained.messageCell } as OutAction,
                  }.toCell();

                  return self.createInternalTransferMessageFromActionsCell(queryId, head, value);
              }
        }
        ",
        expect![[r"
            fun test() {
                if (count > HIGHLOAD_MAX_INLINE_ACTIONS) {
                    val chained = self.packActionsRange(
                        messages,
                        start + HIGHLOAD_CHAIN_CUT,
                        count - HIGHLOAD_CHAIN_CUT,
                        value,
                        queryId,
                    );
                    val chainMode = value > 0 ? SEND_MODE_PAY_FEES_SEPARATELY : SEND_MODE_CARRY_ALL_BALANCE;

                    var head = packSendActionsToCell(messages, start, HIGHLOAD_CHAIN_CUT) as Cell<OutList>;
                    head = OutList {
                        prev: head,
                        action: OutActionSendMessage {
                            mode: chainMode as uint8,
                            outMsg: chained.messageCell,
                        } as OutAction,
                    }.toCell();

                    return self.createInternalTransferMessageFromActionsCell(queryId, head, value);
                }
            }"]],
        100,
    );
    check_with_width(
        r#"
        fun test() {
              val outMsg = createMessage({
                  bounce: false,
                  value: ton("123"),
                  dest: testAddr,
                  body: testBody,
              })
                  .messageCell;
        }
        "#,
        expect![[r#"
            fun test() {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("123"),
                    dest: testAddr,
                    body: testBody,
                }).messageCell;
            }"#]],
        100,
    );
    check_with_width(
        r"
        fun badExternalOutWithBadSource(): cell {
            val invalidDestAsInternal = beginCell()
                .storeUint(2, 2) // addr_std$10
                .storeUint(0, 1) // anycast nothing
                .storeInt(0, 8)
                .storeUint(1, 10)
                .endCell();

            return beginCell()
                .storeUint(3, 2) // ext_out_msg_info$11
                .storeBool(false) // invalid src for MsgAddressInt
                .storeSlice(invalidDestAsInternal.beginParse())
                .endCell();
        }
        ",
        expect![[r"
            fun badExternalOutWithBadSource(): cell {
                val invalidDestAsInternal = beginCell()
                    .storeUint(2, 2) // addr_std$10
                    .storeUint(0, 1) // anycast nothing
                    .storeInt(0, 8)
                    .storeUint(1, 10)
                    .endCell();

                return beginCell()
                    .storeUint(3, 2) // ext_out_msg_info$11
                    .storeBool(false) // invalid src for MsgAddressInt
                    .storeSlice(invalidDestAsInternal.beginParse())
                    .endCell();
            }"]],
        100,
    );
    check_with_width(
        r"
        fun test() {
            val body = beginCell()
                .storeUint(0x5fcc3d14, 32)  // op::transfer
                .storeUint(42, 64)          // queryId
                .storeAddress(nftReceiverAddress)  // new_owner
                .storeAddress(responseAddress)     // response_destination
                .storeMaybeRef(null)        // custom_payload
                .storeCoins(999)            // forward_amount
                // missing forward_payload!
                .endCell();
        }
        ",
        expect![[r"
            fun test() {
                val body = beginCell()
                    .storeUint(0x5fcc3d14, 32) // op::transfer
                    .storeUint(42, 64) // queryId
                    .storeAddress(nftReceiverAddress) // new_owner
                    .storeAddress(responseAddress) // response_destination
                    .storeMaybeRef(null) // custom_payload
                    .storeCoins(999) // forward_amount
                    // missing forward_payload!
                    .endCell();
            }"]],
        100,
    );
    check_with_width(
        r#"
        fun badExternalOutWithBadSource(): cell {
            val result = nftItem.sendAskToChangeOwnership(
                notOwner.address,  // NOT the owner!
                0,  // queryId
                nftReceiverAddress,
                null,  // sendExcessesTo
                createEmptyDict(),  // customPayload
                0,  // forwardTonAmount
                createEmptySlice(),  // forwardPayload
                {
                    value: ton("0.05"),
                    bounce: true,
                }
            );
        }
        "#,
        expect![[r#"
            fun badExternalOutWithBadSource(): cell {
                val result = nftItem.sendAskToChangeOwnership(
                    notOwner.address,   // NOT the owner!
                    0,                  // queryId
                    nftReceiverAddress,
                    null,               // sendExcessesTo
                    createEmptyDict(),  // customPayload
                    0,                  // forwardTonAmount
                    createEmptySlice(), // forwardPayload
                    { value: ton("0.05"), bounce: true },
                );
            }"#]],
        100,
    );
    check_with_width(
        r"
        fun main() {
            val bouncedBody = beginCell().storeUint(0xffffffff, 32).storeSlice(
                outBody.toCell().beginParse(),
            ).endCell();
        }
        ",
        expect![[r"
            fun main() {
                val bouncedBody = beginCell()
                    .storeUint(0xffffffff, 32)
                    .storeSlice(
                        outBody.toCell().beginParse(),
                    )
                    .endCell();
            }"]],
        100,
    );
    check_with_width(
        r"
        fun main() {
            val actionsCell = beginCell()
                .storeUint(0, 1)   // no c5 actions
                .storeUint(1, 1)   // has extra actions
                .storeSlice(setDataAction.beginParse())
                .endCell();
        }
        ",
        expect![[r"
            fun main() {
                val actionsCell = beginCell()
                    .storeUint(0, 1) // no c5 actions
                    .storeUint(1, 1) // has extra actions
                    .storeSlice(setDataAction.beginParse())
                    .endCell();
            }"]],
        100,
    );
    check_with_width(
        r"
        fun main() {
            val minWithForward = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
            MIN_EDGE_DELTA;
        }
        ",
        expect![[r"
            fun main() {
                val minWithForward = calcMinimalTransferAmount(DEFAULT_FORWARD_TON_AMOUNT, fwdFee) +
                MIN_EDGE_DELTA;
            }"]],
        100,
    );
    check_with_width(
        r"
        fun main() {
            if (
                payloadBody.remainingBitsCount() == 1 && payloadBody.preloadUint(1) == 1 &&
                payloadBody.remainingRefsCount() == 1
            ) {
                payloadBody.loadUint(1);
                expect(payloadBody.loadRef().hash()).toEqual(forwardPayload.hash());
            } else {
                expect(payloadRef.hash()).toEqual(forwardPayload.hash());
            }
        }
        ",
        expect![[r"
            fun main() {
                if (
                    payloadBody.remainingBitsCount() == 1 && payloadBody.preloadUint(1) == 1 &&
                    payloadBody.remainingRefsCount() == 1
                ) {
                    payloadBody.loadUint(1);
                    expect(payloadBody.loadRef().hash()).toEqual(forwardPayload.hash());
                } else {
                    expect(payloadRef.hash()).toEqual(forwardPayload.hash());
                }
            }"]],
        100,
    );
}

#[test]
fn test_nested_parenthesized_expressions() {
    check(
        "fun test() { x = ((a + b) * (c - d)) / ((e + f) * g); }",
        expect![[r"
                fun test() {
                    x = ((a + b) * (c - d)) / ((e + f) * g);
                }"]],
    );
}

#[test]
fn test_match_in_expressions() {
    check(
        "fun test() { x = process(match (value) { int => value * 2, string => value.len(), else => 0 }); }",
        expect![[r"
                fun test() {
                    x = process(
                        match (value) {
                            int => value * 2,
                            string => value.len(),
                            else => 0,
                        },
                    );
                }"]],
    );
}
