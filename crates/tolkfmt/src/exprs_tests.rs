#[cfg(test)]
mod tests {
    use crate::{Context, decls};
    use expect_test::{Expect, expect};
    use std::collections::HashMap;
    use tolk_ast::SourceFile;

    fn check(code: &str, expect: Expect) {
        check_with_width(code, expect, 80)
    }

    fn check_with_width(code: &str, expect: Expect, width: usize) {
        // unsafe { std::env::set_var("UPDATE_EXPECT", "1") }

        let tree = tolk_parser::parser::parse(code).expect("Failed to parse");
        let source_file = SourceFile {
            tree: tree.clone(),
            source: code.into(),
        };
        let ctx = Context {
            code: code.into(),
            comments: HashMap::new(),
        };
        let doc = decls::print_source_file(&ctx, &source_file).unwrap();
        let mut out = Vec::new();
        doc.render(width, &mut out).unwrap();
        let res = String::from_utf8(out).unwrap();
        expect.assert_eq(&res);
    }

    #[test]
    fn test_assignment() {
        check(
            "fun test() { x = 10; }",
            expect![[r#"
                fun test() {
                    x = 10;
                }"#]],
        );
    }

    #[test]
    fn test_set_assignment() {
        check(
            "fun test() { x += 10; x -= 5; x *= 2; x /= 3; x %= 4; x &= 1; x |= 2; x ^= 3; x <<= 1; x >>= 2; }",
            expect![[r#"
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
                }"#]],
        );
    }

    #[test]
    fn test_binary_operator() {
        check(
            "fun test() { x = a + b - c * d / e % f & g | h ^ i << j >> k; }",
            expect![[r#"
                fun test() {
                    x = a + b - c * d / e % f & g | h ^ i << j >> k;
                }"#]],
        );
    }

    #[test]
    fn test_binary_operator_breaking() {
        // TODO:
        check_with_width(
            "fun test() { x = a + b + c + d; }",
            expect![[r#"
                fun test() {
                    x = a + b + c +
                    d;
                }"#]],
            20,
        );
    }

    #[test]
    fn test_unary_operator() {
        check(
            "fun test() { x = -a; y = !b; z = ~c; }",
            expect![[r#"
                fun test() {
                    x = -a;
                    y = !b;
                    z = ~c;
                }"#]],
        );
    }

    #[test]
    fn test_ternary_operator() {
        check(
            "fun test() { x = a ? b : c; }",
            expect![[r#"
                fun test() {
                    x = a ? b : c;
                }"#]],
        );
    }

    #[test]
    fn test_ternary_operator_breaking() {
        check_with_width(
            "fun test() { x = long_condition ? long_consequence : long_alternative; }",
            expect![[r#"
                fun test() {
                    x = long_condition
                        ? long_consequence
                        : long_alternative;
                }"#]],
            30,
        );
    }

    #[test]
    fn test_dot_access() {
        check(
            "fun test() { x = a.b; y = a.0; }",
            expect![[r#"
                fun test() {
                    x = a.b;
                    y = a.0;
                }"#]],
        );
    }

    #[test]
    fn test_dot_access_breaking() {
        check_with_width(
            "fun test() { x = very_long_object_name.very_long_field_name; }",
            expect![[r#"
                fun test() {
                    x = very_long_object_name
                        .very_long_field_name;
                }"#]],
            30,
        );
    }

    #[test]
    fn test_dot_access_for_struct_litral() {
        check_with_width(
            "fun test() { Foo { loooooooooong }.toCell() }",
            expect![[r#"
                fun test() {
                    Foo {
                        loooooooooong,
                    }.toCell();
                }"#]],
            20,
        );
    }

    #[test]
    fn test_function_call() {
        check(
            "fun test() { foo(); bar(1); baz(1, 2); }",
            expect![[r#"
                fun test() {
                    foo();
                    bar(1);
                    baz(1, 2);
                }"#]],
        );
    }

    #[test]
    fn test_function_call_mutate() {
        check(
            "fun test() { foo(mutate x, y); }",
            expect![[r#"
                fun test() {
                    foo(mutate x, y);
                }"#]],
        );
    }

    #[test]
    fn test_function_call_breaking() {
        check_with_width(
            "fun test() { foo(arg1, arg2, arg3, arg4); }",
            expect![[r#"
                fun test() {
                    foo(
                        arg1,
                        arg2,
                        arg3,
                        arg4,
                    );
                }"#]],
            20,
        );
    }

    #[test]
    fn test_object_literal() {
        check(
            "fun test() { x = Point { x: 10, y: 20 }; }",
            expect![[r#"
                fun test() {
                    x = Point { x: 10, y: 20 };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_without_type() {
        check(
            "fun test() { x = { x: 10, y: 20 }; }",
            expect![[r#"
                fun test() {
                    x = { x: 10, y: 20 };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_shorthand() {
        check(
            "fun test() { x = Point { x, y }; }",
            expect![[r#"
                fun test() {
                    x = Point { x, y };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_with_expr_and_field_with_same_name() {
        check(
            "fun test() { x = Point { x: x, y: y }; }",
            expect![[r#"
                fun test() {
                    x = Point { x, y };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_multiline() {
        check(
            "fun test() { x = Point { x: 10, y: 20, z: 30 }; }",
            expect![[r#"
                fun test() {
                    x = Point {
                        x: 10,
                        y: 20,
                        z: 30,
                    };
                }"#]],
        );
    }

    // TODO: стрипаются пробелы из-за чего тест не проходит
    // #[test]
    // fn test_object_literal_multiline_with_empty_lines() {
    //     check(
    //         r#"
    //             fun test() {
    //                 x = Point {
    //                     x: 10,
    //
    //                     y: 20,
    //
    //                     z: 30,
    //                 };
    //             }"#,
    //         expect![[r#"
    //             fun test() {
    //                 x = Point {
    //                     x: 10,
    //
    //                     y: 20,
    //
    //                     z: 30,
    //                 };
    //             }"#]],
    //     );
    // }

    #[test]
    fn test_tensor_expression() {
        check(
            "fun test() { x = (1, 2); y = (1); z = (); }",
            expect![[r#"
                fun test() {
                    x = (1, 2);
                    y = (1);
                    z = ();
                }"#]],
        );
    }

    #[test]
    fn test_typed_tuple() {
        check(
            "fun test() { x = [1, 2]; y = [1]; z = []; }",
            expect![[r#"
                fun test() {
                    x = [1, 2];
                    y = [1];
                    z = [];
                }"#]],
        );
    }

    #[test]
    fn test_lambda_expression() {
        check(
            "fun test() { x = fun(a: int, b: int): int { return a + b; }; }",
            expect![[r#"
                fun test() {
                    x = fun(a: int, b: int): int {
                        return a + b;
                    };
                }"#]],
        );
    }

    #[test]
    fn test_cast_as_operator() {
        check(
            "fun test() { x = a as int; }",
            expect![[r#"
                fun test() {
                    x = a as int;
                }"#]],
        );
    }

    #[test]
    fn test_is_type_operator() {
        check(
            "fun test() { x = a is int; y = a !is int; }",
            expect![[r#"
                fun test() {
                    x = a is int;
                    y = a !is int;
                }"#]],
        );
    }

    #[test]
    fn test_not_null_operator() {
        check(
            "fun test() { x = a!; }",
            expect![[r#"
                fun test() {
                    x = a!;
                }"#]],
        );
    }

    #[test]
    fn test_lazy_expression() {
        check(
            "fun test() { x = lazy Foo.fromCell(cell); }",
            expect![[r#"
                fun test() {
                    x = lazy Foo.fromCell(cell);
                }"#]],
        );
    }

    #[test]
    fn test_generic_instantiation() {
        check(
            "fun test() { x = foo<int, slice>; }",
            expect![[r#"
                fun test() {
                    x = foo<int, slice>;
                }"#]],
        );
    }

    #[test]
    fn test_complex_binary_operators() {
        check(
            "fun test() { x = a + b * c - d / e % f; }",
            expect![[r#"
                fun test() {
                    x = a + b * c - d / e % f;
                }"#]],
        );
    }

    #[test]
    fn test_bitwise_operators() {
        check(
            "fun test() { x = a & b | c ^ d; y = a << b >> c; }",
            expect![[r#"
                fun test() {
                    x = a & b | c ^ d;
                    y = a << b >> c;
                }"#]],
        );
    }

    #[test]
    fn test_comparison_operators() {
        check(
            "fun test() { x = a == b != c < d <= e > f >= g <=> h; }",
            expect![[r#"
                fun test() {
                    x = a == b != c < d <= e > f >= g <=> h;
                }"#]],
        );
    }

    #[test]
    fn test_logical_operators() {
        check(
            "fun test() { x = a && b || c; y = !a && !b; }",
            expect![[r#"
                fun test() {
                    x = a && b || c;
                    y = !a && !b;
                }"#]],
        );
    }

    #[test]
    fn test_arithmetic_special_operators() {
        check(
            "fun test() { x = a ~/ b ^/ c; }",
            expect![[r#"
                fun test() {
                    x = a ~/ b ^/ c;
                }"#]],
        );
    }

    #[test]
    fn test_mixed_operators_complex() {
        check(
            "fun test() { x = (a + b) * c << d & e | f ^ g && h || i == j; }",
            expect![[r#"
                fun test() {
                    x = (a + b) * c << d & e | f ^ g && h || i == j;
                }"#]],
        );
    }

    #[test]
    fn test_unary_multiple() {
        check(
            "fun test() { x = -+~-!a; y = !!b; }",
            expect![[r#"
                fun test() {
                    x = -+~-!a;
                    y = !!b;
                }"#]],
        );
    }

    #[test]
    fn test_null_checks() {
        check(
            "fun test() { x = a != null && b is int; }",
            expect![[r#"
                fun test() {
                    x = a != null && b is int;
                }"#]],
        );
    }

    #[test]
    fn test_number_literals() {
        check(
            "fun test() { x = 42; z = 0xFF; w = 0b1010; }",
            expect![[r#"
                fun test() {
                    x = 42;
                    z = 0xFF;
                    w = 0b1010;
                }"#]],
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
            expect![[r#"
                fun test() {
                    x = true;
                    y = false;
                    z = !true;
                }"#]],
        );
    }

    #[test]
    fn test_null_literal() {
        check(
            "fun test() { x = null; y = a == null; }",
            expect![[r#"
                fun test() {
                    x = null;
                    y = a == null;
                }"#]],
        );
    }

    #[test]
    fn test_underscore_literal() {
        check(
            "fun test() { x = _; match (x) { _ => return 1; } }",
            expect![[r#"
                fun test() {
                    x = _;
                    match (x) {
                        _ => return 1,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_numeric_index() {
        check(
            "fun test() { x = a.0; y = b.1; z = c.42; }",
            expect![[r#"
                fun test() {
                    x = a.0;
                    y = b.1;
                    z = c.42;
                }"#]],
        );
    }

    #[test]
    fn test_identifiers() {
        check(
            "fun test() { x = variable_name; y = _private; z = camelCase; w = snake_case; }",
            expect![[r#"
                fun test() {
                    x = variable_name;
                    y = _private;
                    z = camelCase;
                    w = snake_case;
                }"#]],
        );
    }

    #[test]
    fn test_function_call_complex() {
        check(
            "fun test() { result = calculate(a + b, c * d, func(e, f)); }",
            expect![[r#"
                fun test() {
                    result = calculate(a + b, c * d, func(e, f));
                }"#]],
        );
    }

    #[test]
    fn test_function_call_with_mutate() {
        check(
            "fun test() { foo(mutate x, mutate y, z); bar(mutate a.b, c); }",
            expect![[r#"
                fun test() {
                    foo(mutate x, mutate y, z);
                    bar(mutate a.b, c);
                }"#]],
        );
    }

    #[test]
    fn test_method_calls() {
        check(
            "fun test() { x = obj.method(); y = a.b.c.method(arg); }",
            expect![[r#"
                fun test() {
                    x = obj.method();
                    y = a.b.c.method(arg);
                }"#]],
        );
    }

    #[test]
    fn test_nested_function_calls() {
        check(
            "fun test() { x = outer(inner1(), inner2(a, b)); }",
            expect![[r#"
                fun test() {
                    x = outer(inner1(), inner2(a, b));
                }"#]],
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
    fn test_function_call_breaking_long() {
        check_with_width(
            "fun test() { very_long_function_name(argument_one, argument_two, argument_three, argument_four, argument_five); }",
            expect![[r#"
                fun test() {
                    very_long_function_name(
                        argument_one,
                        argument_two,
                        argument_three,
                        argument_four,
                        argument_five,
                    );
                }"#]],
            50,
        );
    }

    #[test]
    fn test_empty_match_expression() {
        check(
            "fun test() { match (1) {}; }",
            expect![[r#"
                fun test() {
                    match (1) {}
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_simple() {
        check(
            "fun test() { x = match (value) { int => 1, string => 2, else => 0 }; }",
            expect![[r#"
                fun test() {
                    x = match (value) {
                        int => 1,
                        string => 2,
                        else => 0,
                    };
                }"#]],
        );
    }

    // // TODO: стрипаются пробелы из-за чего тест не проходит
    // #[test]
    // fn test_match_expression_with_empty_lines() {
    //     check(
    //         r#"
    //             fun test() {
    //                 x = match (value) {
    //                     int => 1,
    //
    //                     string => 2,
    //
    //                     else => 0,
    //                 };
    //             }"#,
    //         expect![[r#"
    //             fun test() {
    //                 x = match (value) {
    //                 int => 1,
    //
    //                     string => 2,
    //
    //                     else => 0,
    //                 };
    //             }"#]],
    //     );
    // }

    #[test]
    fn test_match_expression_with_blocks() {
        check(
            "fun test() { result = match (x) { 1 => { return 1; }, 2 => { return 2; }, else => { return 0; } }; }",
            expect![[r#"
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
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_with_expressions() {
        check(
            "fun test() { x = match (a) { 1 => a + 1, 2 => a * 2, else => 0 }; }",
            expect![[r#"
                fun test() {
                    x = match (a) {
                        1 => a + 1,
                        2 => a * 2,
                        else => 0,
                    };
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_complex_patterns() {
        check(
            "fun test() { x = match (data) { Point => data.x, else => -1 }; }",
            expect![[r#"
                fun test() {
                    x = match (data) {
                        Point => data.x,
                        else => -1,
                    };
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_with_local_vars() {
        check(
            "fun test() { x = match (val a = get_value()) { int => a + b, else => 0 }; }",
            expect![[r#"
                fun test() {
                    x = match (val a = get_value()) {
                        int => a + b,
                        else => 0,
                    };
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_nested() {
        check(
            "fun test() { x = match (outer) { int => match (inner) { 1 => true, else => false }, else => null }; }",
            expect![[r#"
                fun test() {
                    x = match (outer) {
                        int => match (inner) {
                            1 => true,
                            else => false,
                        },
                        else => null,
                    };
                }"#]],
        );
    }

    #[test]
    fn test_lambda_simple() {
        check(
            "fun test() { x = fun(a: int, b: int): int { return a + b; }; }",
            expect![[r#"
                fun test() {
                    x = fun(a: int, b: int): int {
                        return a + b;
                    };
                }"#]],
        );
    }

    #[test]
    fn test_lambda_without_types() {
        check(
            "fun test() { x = fun(a, b) { return a + b; }; }",
            expect![[r#"
                fun test() {
                    x = fun(a, b) {
                        return a + b;
                    };
                }"#]],
        );
    }

    #[test]
    fn test_lambda_single_param() {
        check(
            "fun test() { x = fun(x: int): int { return x * 2; }; }",
            expect![[r#"
                fun test() {
                    x = fun(x: int): int {
                        return x * 2;
                    };
                }"#]],
        );
    }

    #[test]
    fn test_lambda_no_params() {
        check(
            "fun test() { x = fun(): int { return 42; }; }",
            expect![[r#"
                fun test() {
                    x = fun(): int {
                        return 42;
                    };
                }"#]],
        );
    }

    #[test]
    fn test_lambda_with_mutate() {
        check(
            "fun test() { x = fun(mutate a: int, b: int) { a = a + b; return a; }; }",
            expect![[r#"
                fun test() {
                    x = fun(mutate a: int, b: int) {
                        a = a + b;
                        return a;
                    };
                }"#]],
        );
    }

    #[test]
    fn test_lambda_complex_body() {
        check(
            "fun test() { x = fun(a: int, b: int): int { if (a > b) { return a; } else { return b; } }; }",
            expect![[r#"
                fun test() {
                    x = fun(a: int, b: int): int {
                        if (a > b) {
                            return a;
                        } else {
                            return b;
                        }
                    };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_typed() {
        check(
            "fun test() { x = Point { x: 10, y: 20 }; }",
            expect![[r#"
                fun test() {
                    x = Point { x: 10, y: 20 };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_shorthand_all() {
        check(
            "fun test() { x = Point { x, y, z }; }",
            expect![[r#"
                fun test() {
                    x = Point {
                        x,
                        y,
                        z,
                    };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_mixed() {
        check(
            "fun test() { x = Config { enabled: true, name, value: 42 }; }",
            expect![[r#"
                fun test() {
                    x = Config {
                        enabled: true,
                        name,
                        value: 42,
                    };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_empty() {
        check(
            "fun test() { x = Empty {}; }",
            expect![[r#"
                fun test() {
                    x = Empty {};
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_single_field() {
        check(
            "fun test() { x = Singleton { value: 1 }; }",
            expect![[r#"
                fun test() {
                    x = Singleton { value: 1 };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_with_expressions() {
        check(
            "fun test() { x = Point { x: a + b, y: c * 2, z: func() }; }",
            expect![[r#"
                fun test() {
                    x = Point {
                        x: a + b,
                        y: c * 2,
                        z: func(),
                    };
                }"#]],
        );
    }

    #[test]
    fn test_object_literal_breaking() {
        check_with_width(
            "fun test() { x = VeryLongTypeName { very_long_field_name: very_long_expression_value, another_field: another_value }; }",
            expect![[r#"
                fun test() {
                    x = VeryLongTypeName {
                        very_long_field_name: very_long_expression_value,
                        another_field: another_value,
                    };
                }"#]],
            40,
        );
    }

    #[test]
    fn test_tensor_expressions() {
        check(
            "fun test() { x = (1, 2); y = (1); z = (); w = (a, b, c); }",
            expect![[r#"
                fun test() {
                    x = (1, 2);
                    y = (1);
                    z = ();
                    w = (a, b, c);
                }"#]],
        );
    }

    #[test]
    fn test_tensor_with_expressions() {
        check(
            "fun test() { x = (a + b, c * d, func()); y = (1,); }",
            expect![[r#"
                fun test() {
                    x = (a + b, c * d, func());
                    y = (1);
                }"#]],
        );
    }

    #[test]
    fn test_tensor_breaking() {
        check_with_width(
            "fun test() { x = (very_long_expression_one, very_long_expression_two, very_long_expression_three); }",
            expect![[r#"
                fun test() {
                    x = (
                        very_long_expression_one,
                        very_long_expression_two,
                        very_long_expression_three,
                    );
                }"#]],
            40,
        );
    }

    #[test]
    fn test_typed_tuples() {
        check(
            "fun test() { x = [1, 2]; y = [1]; z = []; w = [a, b, c]; }",
            expect![[r#"
                fun test() {
                    x = [1, 2];
                    y = [1];
                    z = [];
                    w = [a, b, c];
                }"#]],
        );
    }

    #[test]
    fn test_typed_tuples_with_expressions() {
        check(
            "fun test() { x = [a + 1, b * 2, func(c)]; y = [single_element]; }",
            expect![[r#"
                fun test() {
                    x = [a + 1, b * 2, func(c)];
                    y = [single_element];
                }"#]],
        );
    }

    #[test]
    fn test_typed_tuples_breaking() {
        check_with_width(
            "fun test() { x = [very_long_first_element, very_long_second_element, very_long_third_element]; }",
            expect![[r#"
                fun test() {
                    x = [
                        very_long_first_element,
                        very_long_second_element,
                        very_long_third_element,
                    ];
                }"#]],
            40,
        );
    }

    #[test]
    fn test_generic_instantiation_with_function_calls() {
        check(
            "fun test() { x = create_map<string, int>(); y = List<int>.empty(); }",
            expect![[r#"
                fun test() {
                    x = create_map<string, int>();
                    y = List<int>.empty();
                }"#]],
        );
    }

    #[test]
    fn test_generic_instantiation_complex_types() {
        check(
            "fun test() { x = Dict<string, int>; }",
            expect![[r#"
                fun test() {
                    x = Dict<string, int>;
                }"#]],
        );
    }

    #[test]
    fn test_deeply_nested_expressions() {
        check(
            "fun test() { x = a.b.c.d.e.f(); y = (a + b).c.d.e(); }",
            expect![[r#"
                fun test() {
                    x = a.b.c.d.e.f();
                    y = (a + b).c.d.e();
                }"#]],
        );
    }

    #[test]
    fn test_complex_expression_combination() {
        check(
            "fun test() { x = func(a + b, c * d).field.0.method(e ? f : g); }",
            expect![[r#"
                fun test() {
                    x = func(a + b, c * d).field.0.method(e ? f : g);
                }"#]],
        );
    }

    #[test]
    fn test_complex_expression_combination_with_breaking() {
        // TODO
        check_with_width(
            "fun test() { x = func(a + b, c * d).field.0.method(e ? f : g); }",
            expect![[r#"
                fun test() {
                    x = func(
                        a + b,
                        c * d,
                    ).field.0
                        .method(e
                        ? f
                        : g);
                }"#]],
            20,
        );
    }

    #[test]
    fn test_nested_parenthesized_expressions() {
        check(
            "fun test() { x = ((a + b) * (c - d)) / ((e + f) * g); }",
            expect![[r#"
                fun test() {
                    x = ((a + b) * (c - d)) / ((e + f) * g);
                }"#]],
        );
    }

    #[test]
    fn test_match_in_expressions() {
        check(
            "fun test() { x = process(match (value) { int => value * 2, string => value.len(), else => 0 }); }",
            expect![[r#"
                fun test() {
                    x = process(match (value) {
                        int => value * 2,
                        string => value.len(),
                        else => 0,
                    });
                }"#]],
        );
    }
}
