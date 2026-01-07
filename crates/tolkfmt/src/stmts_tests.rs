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
        let mut ctx = Context {
            code: code.into(),
            comments: HashMap::new(),
        };
        let doc = decls::print_source_file(&mut ctx, &source_file).unwrap();
        let mut out = Vec::new();
        doc.render(width, &mut out).unwrap();
        let res = String::from_utf8(out).unwrap();
        expect.assert_eq(&res);
    }

    // Healthy tests to check if nothing is broken while formatting

    #[test]
    fn test_if_statement() {
        check(
            "fun test() { if (true) { return 1; } else { return 0; } }",
            expect![[r#"
                fun test() {
                    if (true) {
                        return 1;
                    } else {
                        return 0;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_if_without_else() {
        check(
            "fun test() { if (true) { return 1; } }",
            expect![[r#"
                fun test() {
                    if (true) {
                        return 1;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_if_else() {
        check(
            "fun test() { if (true) { return 1; } else { return 0; } }",
            expect![[r#"
                fun test() {
                    if (true) {
                        return 1;
                    } else {
                        return 0;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_if_else_if() {
        check(
            "fun test() { if (x > 10) { return 1; } else if (x > 5) { return 2; } }",
            expect![[r#"
                fun test() {
                    if (x > 10) {
                        return 1;
                    } else if (x > 5) {
                        return 2;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_if_else_if_else() {
        check(
            "fun test() { if (x > 10) { return 1; } else if (x > 5) { return 2; } else { return 0; } }",
            expect![[r#"
                fun test() {
                    if (x > 10) {
                        return 1;
                    } else if (x > 5) {
                        return 2;
                    } else {
                        return 0;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_nested_if() {
        check(
            "fun test() { if (x > 0) { if (y > 0) { return 1; } else { return 2; } } else { return 0; } }",
            expect![[r#"
                fun test() {
                    if (x > 0) {
                        if (y > 0) {
                            return 1;
                        } else {
                            return 2;
                        }
                    } else {
                        return 0;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_while_statement() {
        check(
            "fun test() { while (x < 10) { x = x + 1; } }",
            expect![[r#"
                fun test() {
                    while (x < 10) {
                        x = x + 1;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_repeat_statement() {
        check(
            "fun test() { repeat (10) { x = x + 1; } }",
            expect![[r#"
                fun test() {
                    repeat (10) {
                        x = x + 1;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_do_while_statement() {
        check(
            "fun test() { do { x = x + 1; } while (x < 10); }",
            expect![[r#"
                fun test() {
                    do {
                        x = x + 1;
                    } while (x < 10);
                }"#]],
        );
    }

    #[test]
    fn test_local_vars() {
        check(
            "fun test() { var x = 10; val y: int = 20; val [a, b] = [1, 2]; }",
            expect![[r#"
                fun test() {
                    var x = 10;
                    val y: int = 20;
                    val [a, b] = [1, 2];
                }"#]],
        );
    }

    #[test]
    fn test_var_without_type_without_init() {
        check(
            "fun test() { var x; }",
            expect![[r#"
                fun test() {
                    var x;
                }"#]],
        );
    }

    #[test]
    fn test_val_without_type_without_init() {
        check(
            "fun test() { val y; }",
            expect![[r#"
                fun test() {
                    val y;
                }"#]],
        );
    }

    #[test]
    fn test_var_with_type_without_init() {
        check(
            "fun test() { var x: int; }",
            expect![[r#"
                fun test() {
                    var x: int;
                }"#]],
        );
    }

    #[test]
    fn test_val_with_type_without_init() {
        check(
            "fun test() { val y: slice; }",
            expect![[r#"
                fun test() {
                    val y: slice;
                }"#]],
        );
    }

    #[test]
    fn test_var_without_type_with_init() {
        check(
            "fun test() { var x = 42; }",
            expect![[r#"
                fun test() {
                    var x = 42;
                }"#]],
        );
    }

    #[test]
    fn test_val_without_type_with_init() {
        check(
            "fun test() { val y = \"hello\"; }",
            expect![[r#"
                fun test() {
                    val y = "hello";
                }"#]],
        );
    }

    #[test]
    fn test_var_with_type_with_init() {
        check(
            "fun test() { var x: int = 42; }",
            expect![[r#"
                fun test() {
                    var x: int = 42;
                }"#]],
        );
    }

    #[test]
    fn test_val_with_type_with_init() {
        check(
            "fun test() { val y: slice = \"hello\"; }",
            expect![[r#"
                fun test() {
                    val y: slice = "hello";
                }"#]],
        );
    }

    #[test]
    fn test_var_redef() {
        check(
            "fun test() { var x redef; }",
            expect![[r#"
                fun test() {
                    var x redef;
                }"#]],
        );
    }

    #[test]
    fn test_val_redef() {
        check(
            "fun test() { val y redef; }",
            expect![[r#"
                fun test() {
                    val y redef;
                }"#]],
        );
    }

    #[test]
    fn test_tuple_destructuring() {
        check(
            "fun test() { var [a, b] = [1, 2]; }",
            expect![[r#"
                fun test() {
                    var [a, b] = [1, 2];
                }"#]],
        );
    }

    #[test]
    fn test_tuple_destructuring_with_types() {
        check(
            "fun test() { var [a: int, b: slice] = [1, 2]; }",
            expect![[r#"
                fun test() {
                    var [a: int, b: slice] = [1, 2];
                }"#]],
        );
    }

    #[test]
    fn test_tensor_destructuring() {
        check(
            "fun test() { var (a, b) = (1, 2); }",
            expect![[r#"
                fun test() {
                    var (a, b) = (1, 2);
                }"#]],
        );
    }

    #[test]
    fn test_tensor_destructuring_with_types() {
        check(
            "fun test() { var (a: int, b: slice) = (1, 2); }",
            expect![[r#"
                fun test() {
                    var (a: int, b: slice) = (1, 2);
                }"#]],
        );
    }

    #[test]
    fn test_nested_destructuring() {
        check(
            "fun test() { var [[a, b], (c, d)] = [[1, 2], (3, 4)]; }",
            expect![[r#"
                fun test() {
                    var [[a, b], (c, d)] = [[1, 2], (3, 4)];
                }"#]],
        );
    }

    #[test]
    fn test_complex_destructuring() {
        check(
            "fun test() { var  [a: int, b, c redef] = [1, 2, 3]; }",
            expect![[r#"
                fun test() {
                    var [a: int, b, c redef] = [1, 2, 3];
                }"#]],
        );
    }

    #[test]
    fn test_return_without_expression() {
        check(
            "fun test() { return; }",
            expect![[r#"
                fun test() {
                    return;
                }"#]],
        );
    }

    #[test]
    fn test_return_with_expression() {
        check(
            "fun test() { return 42; }",
            expect![[r#"
                fun test() {
                    return 42;
                }"#]],
        );
    }

    #[test]
    fn test_throw_with_expression() {
        check(
            "fun test() { throw 100; }",
            expect![[r#"
                fun test() {
                    throw 100;
                }"#]],
        );
    }

    #[test]
    fn test_return_in_match_arm() {
        check(
            "fun test() { match (x) { 1 => return 10, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => return 10,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_throw_in_match_arm() {
        check(
            "fun test() { match (x) { 1 => throw 100, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => throw 100,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_assert_comma_syntax() {
        check(
            "fun test() { assert(x > 0, 100); }",
            expect![[r#"
                fun test() {
                    assert(x > 0, 100);
                }"#]],
        );
    }

    #[test]
    fn test_assert_throw_syntax() {
        check(
            "fun test() { assert (x > 0) throw 100; }",
            expect![[r#"
                fun test() {
                    assert (x > 0) throw 100;
                }"#]],
        );
    }

    #[test]
    fn test_try_catch_no_vars() {
        check(
            "fun test() { try { risky(); } catch { return 0; } }",
            expect![[r#"
                fun test() {
                    try {
                        risky();
                    } catch {
                        return 0;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_try_catch_one_var() {
        check(
            "fun test() { try { risky(); } catch (e) { return e; } }",
            expect![[r#"
                fun test() {
                    try {
                        risky();
                    } catch (e) {
                        return e;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_try_catch_two_vars() {
        check(
            "fun test() { try { risky(); } catch (e, arg) { return e + arg; } }",
            expect![[r#"
                fun test() {
                    try {
                        risky();
                    } catch (e, arg) {
                        return e + arg;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_type_pattern() {
        check(
            "fun test() { match (x) { int => return 1, string => return 2, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        int => return 1,
                        string => return 2,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_pattern() {
        check(
            "fun test() { match (x) { 1 => return 10, \"hello\" => return 20, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => return 10,
                        "hello" => return 20,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_block_body() {
        check(
            "fun test() { match (x) { 1 => { var y = 10; return y; } else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => {
                            var y = 10;
                            return y;
                        }
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_expression_body() {
        check(
            "fun test() { match (x) { 1 => x * 2, else => 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => x * 2,
                        else => 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_with_var_declaration() {
        check(
            "fun test() { match (var x = getValue()) { 1 => return 10, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (var x = getValue()) {
                        1 => return 10,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_break_statement() {
        check(
            "fun test() { while (true) { break; } }",
            expect![[r#"
                fun test() {
                    while (true) {
                        break;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_continue_statement() {
        check(
            "fun test() { while (true) { continue; } }",
            expect![[r#"
                fun test() {
                    while (true) {
                        continue;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_empty_statement() {
        check("fun test() { ; }", expect!["fun test() {}"]);
    }

    #[test]
    fn test_empty_statements() {
        check("fun test() { ;;;;;; }", expect!["fun test() {}"]);
    }

    #[test]
    fn test_empty_statements_after_stmt() {
        check(
            "fun test() { val a = 100;;;;;; }",
            expect![[r#"
                fun test() {
                    val a = 100;
                }"#]],
        );
    }

    #[test]
    fn test_expression_statement() {
        check(
            "fun test() { x = 42; }",
            expect![[r#"
                fun test() {
                    x = 42;
                }"#]],
        );
    }

    #[test]
    fn test_function_call_expression() {
        check(
            "fun test() { doSomething(); }",
            expect![[r#"
                fun test() {
                    doSomething();
                }"#]],
        );
    }

    #[test]
    fn test_while_with_complex_condition() {
        check(
            "fun test() { while (x > 0 && y < 10) { x = x - 1; y = y + 1; } }",
            expect![[r#"
                fun test() {
                    while (x > 0 && y < 10) {
                        x = x - 1;
                        y = y + 1;
                    }
                }"#]],
        );
    }

    #[test]
    fn test_empty_block() {
        check(
            "fun test() { {} }",
            expect![[r#"
                fun test() {
                    {}
                }"#]],
        );
    }

    #[test]
    fn test_match() {
        check(
            "fun test() { match (x) { 1 => { return 10; } else => return 0; } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => {
                            return 10;
                        }
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_arm_return_statement() {
        check(
            "fun test() { match (x) { 1 => return 10, 2 => return 20, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => return 10,
                        2 => return 20,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_arm_throw_statement() {
        check(
            "fun test() { match (x) { 1 => throw 100, 2 => throw 200, else => return 0 } }",
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => throw 100,
                        2 => throw 200,
                        else => return 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_match_mixed_arm_bodies() {
        check(
            r#"fun test() {
                match (x) {
                    1 => return 10,
                    2 => { var y = 20; return y; },
                    3 => throw 100,
                    4 => x * 2,
                    else => 0
                }
            }"#,
            expect![[r#"
                fun test() {
                    match (x) {
                        1 => return 10,
                        2 => {
                            var y = 20;
                            return y;
                        }
                        3 => throw 100,
                        4 => x * 2,
                        else => 0,
                    }
                }"#]],
        );
    }

    #[test]
    fn test_deeply_nested_blocks() {
        check(
            r#"fun test() {
                {
                    {
                        {
                            var x = 1;
                            {
                                var y = 2;
                                return x + y;
                            }
                        }
                    }
                }
            }"#,
            expect![[r#"
                fun test() {
                    {
                        {
                            {
                                var x = 1;
                                {
                                    var y = 2;
                                    return x + y;
                                }
                            }
                        }
                    }
                }"#]],
        );
    }

    #[test]
    fn test_multiple_assert_statements() {
        check(
            r#"fun test() {
                assert(x > 0, 100);
                assert (y != null) throw 101;
                assert(z >= 0, 102);
                return x + y + z;
            }"#,
            expect![[r#"
                fun test() {
                    assert(x > 0, 100);
                    assert (y != null) throw 101;
                    assert(z >= 0, 102);
                    return x + y + z;
                }"#]],
        );
    }

    #[test]
    fn test_match_with_type_patterns() {
        check(
            r#"fun test() {
                match (getValue()) {
                    int => return 1,
                    string => return 2,
                    bool => return 3,
                    else => throw 200
                }
            }"#,
            expect![[r#"
                fun test() {
                    match (getValue()) {
                        int => return 1,
                        string => return 2,
                        bool => return 3,
                        else => throw 200,
                    }
                }"#]],
        );
    }

    // Tests for line breaking behavior with small widths

    #[test]
    fn test_line_breaking_if_statement_small_width() {
        check_with_width(
            "fun test() { if (very_long_condition_that_should_break) { return 42; } }",
            expect![[r#"
                fun test() {
                    if (
                        very_long_condition_that_should_break
                    ) {
                        return 42;
                    }
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_while_statement_small_width() {
        check_with_width(
            "fun test() { while (very_long_condition_that_should_break) { x = x + 1; } }",
            expect![[r#"
                fun test() {
                    while (
                        very_long_condition_that_should_break
                    ) {
                        x = x + 1;
                    }
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_repeat_statement_small_width() {
        check_with_width(
            "fun test() { repeat (very_long_expression_that_should_break) { x = x + 1; } }",
            expect![[r#"
                fun test() {
                    repeat (
                        very_long_expression_that_should_break
                    ) {
                        x = x + 1;
                    }
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_do_while_statement_small_width() {
        check_with_width(
            "fun test() { do { x = x + 1; } while (very_long_condition_that_should_break); }",
            expect![[r#"
                fun test() {
                    do {
                        x = x + 1;
                    } while (
                        very_long_condition_that_should_break
                    );
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_assert_statement_small_width() {
        check_with_width(
            "fun test() { assert (very_long_condition_that_should_break) throw error_code; }",
            expect![[r#"
                fun test() {
                    assert (
                        very_long_condition_that_should_break
                    ) throw error_code;
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_function_call_small_width() {
        check_with_width(
            "fun test() { very_long_function_name_that_should_break(arg1, arg2, arg3); }",
            expect![[r#"
                fun test() {
                    very_long_function_name_that_should_break(
                        arg1,
                        arg2,
                        arg3
                    );
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_match_expression_small_width() {
        check_with_width(
            r#"fun test() {
                match (very_long_expression_that_should_break) {
                    1 => return 10,
                    2 => return 20,
                    else => return 0
                }
            }"#,
            expect![[r#"
                fun test() {
                    match (
                        very_long_expression_that_should_break
                    ) {
                        1 => return 10,
                        2 => return 20,
                        else => return 0,
                    }
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_complex_expression_small_width() {
        check_with_width(
            "fun test() { x = a + b + c + d + e + f + g; }",
            expect![[r#"
                fun test() {
                    x = a + b + c +
                    d +
                    e +
                    f +
                    g;
                }"#]],
            20,
        );
    }

    #[test]
    fn test_line_breaking_nested_calls_small_width() {
        check_with_width(
            "fun test() { result = outer_function(inner_function(arg1, arg2), another_arg); }",
            expect![[r#"
                fun test() {
                    result = outer_function(
                        inner_function(
                            arg1,
                            arg2
                        ),
                        another_arg
                    );
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_tuple_small_width() {
        check_with_width(
            "fun test() { var [a, b, c] = [value1, value2, value3]; }",
            expect![[r#"
                fun test() {
                    var [a, b, c] = [
                        value1,
                        value2,
                        value3
                    ];
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_tensor_small_width() {
        check_with_width(
            "fun test() { var (a, b, c) = (value1, value2, value3); }",
            expect![[r#"
                fun test() {
                    var (a, b, c) = (
                        value1,
                        value2,
                        value3
                    );
                }"#]],
            30,
        );
    }

    #[test]
    fn test_line_breaking_try_catch_small_width() {
        check_with_width(
            r#"fun test() {
                try {
                    very_long_function_call_that_should_break();
                } catch (exception, code) {
                    handle_error(exception, code);
                }
            }"#,
            expect![[r#"
                fun test() {
                    try {
                        very_long_function_call_that_should_break();
                    } catch (exception, code) {
                        handle_error(
                            exception,
                            code
                        );
                    }
                }"#]],
            20,
        );
    }
}
