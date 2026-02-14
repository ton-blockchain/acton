#[cfg(test)]
mod tests {
    use crate::file_db::FileDb;
    use crate::resolve_index::Resolved;
    use expect_test::{Expect, expect};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn test_local_var() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    <caret>x;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_tuple_local_var() {
        check_definition(
            r#"
                fun main() {
                    val [x, y] = [1, 2];
                    <caret>x;
                    <caret>y;
                }
            "#,
            expect![[r#"
                x -> Local(x at 55-56)
                y -> Local(y at 58-59)
            "#]],
        );
    }

    #[test]
    fn test_nested_tuple_local_var() {
        check_definition(
            r#"
                fun main() {
                    val [x, [[y, z], w]] = [];
                    <caret>x;
                    <caret>y;
                    <caret>z;
                    <caret>w;
                }
            "#,
            expect![[r#"
                x -> Local(x at 55-56)
                y -> Local(y at 60-61)
                z -> Local(z at 63-64)
                w -> Local(w at 67-68)
            "#]],
        );
    }

    #[test]
    fn test_tensor_local_var() {
        check_definition(
            r#"
                fun main() {
                    val (x, y) = (1, 2);
                    <caret>x;
                    <caret>y;
                }
            "#,
            expect![[r#"
                x -> Local(x at 55-56)
                y -> Local(y at 58-59)
            "#]],
        );
    }

    #[test]
    fn test_nested_tensor_local_var() {
        check_definition(
            r#"
                fun main() {
                    val ((x, ((y, z), w)) = [];
                    <caret>x;
                    <caret>y;
                    <caret>z;
                    <caret>w;
                }
            "#,
            expect![[r#"
                x -> Local(x at 56-57)
                y -> Local(y at 61-62)
                z -> Local(z at 64-65)
                w -> Local(w at 68-69)
            "#]],
        );
    }

    #[test]
    fn test_function_param() {
        check_definition(
            r#"
                fun foo(a: int) {
                    <caret>a;
                }
            "#,
            expect![[r#"
                a -> Local(a at 25-26)
            "#]],
        );
    }

    #[test]
    fn test_global_function() {
        check_definition(
            r#"
                fun foo() {}

                fun main() {
                    <caret>foo();
                }
            "#,
            expect![[r#"
                foo -> Global(foo at test.tolk:21-24)
            "#]],
        );
    }

    #[test]
    fn test_nested_scopes() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    {
                        val x = 2;
                        <caret>x;
                    }
                    <caret>x;
                }
            "#,
            expect![[r#"
                x -> Local(x at 111-112)
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_unresolved() {
        check_definition(
            r#"
                fun main() {
                    <caret>unknown_symbol;
                }
            "#,
            expect![[r#"
                unknown_symbol -> Unresolved
            "#]],
        );
    }

    #[test]
    fn test_global_var_and_const() {
        check_definition(
            r#"
                global g: int;

                const C: int = 42;

                fun main() {
                    <caret>g;
                    <caret>C;
                }
            "#,
            expect![[r#"
                g -> Global(g at test.tolk:24-25)
                C -> Global(C at test.tolk:55-56)
            "#]],
        );
    }

    #[test]
    fn test_type_alias() {
        check_definition(
            r#"
                type MyInt = int;
                fun foo(x: <caret>MyInt) {}
            "#,
            expect![[r#"
                MyInt -> Global(MyInt at test.tolk:22-27)
            "#]],
        );
    }

    #[test]
    fn test_struct_and_enum() {
        check_definition(
            r#"
                struct S { x: int }

                enum E { A, B }

                fun main() {
                    val s: <caret>S;
                    val e: <caret>E = <caret>E.A;
                }
            "#,
            expect![[r#"
                S -> Global(S at test.tolk:24-25)
                E -> Global(E at test.tolk:59-60)
                E -> Global(E at test.tolk:59-60)
            "#]],
        );
    }

    #[test]
    fn test_lambda() {
        check_definition(
            r#"
                const FOO = 100;

                fun main() {
                    val f = fun (x: int = <caret>FOO) {
                        <caret>x;
                    };
                }
            "#,
            expect![[r#"
                FOO -> Global(FOO at test.tolk:23-26)
                x -> Local(x at 97-98)
            "#]],
        );
    }

    #[test]
    fn test_lambda_return_type() {
        check_definition(
            r#"
                fun main() {
                    val f = fun (x: int): <caret>int {
                        <caret>x;
                    };
                }
            "#,
            expect![[r#"
                int -> Global(int at common.tolk:350-353)
                x -> Local(x at 63-64)
            "#]],
        );
    }

    #[test]
    fn test_catch() {
        check_definition(
            r#"
                fun main() {
                    try {
                    } catch (x, y) {
                        <caret>x;
                        <caret>y;
                    }
                }
            "#,
            expect![[r#"
                x -> Local(x at 85-86)
                y -> Local(y at 88-89)
            "#]],
        );
    }

    #[test]
    fn test_method() {
        check_definition(
            r#"
                struct S {}

                fun S.foo(self, x: int) {
                    <caret>self;
                    <caret>x;
                }
            "#,
            expect![[r#"
                self -> Local(self at 56-60)
                x -> Local(x at 62-63)
            "#]],
        );
    }

    #[test]
    fn test_get_method() {
        check_definition(
            r#"
                type MyInt = builtin;

                get fun foo(x: int): <caret>MyInt {
                    <caret>x;
                }
            "#,
            expect![[r#"
                MyInt -> Global(MyInt at test.tolk:22-27)
                x -> Local(x at 68-69)
            "#]],
        );
    }

    #[test]
    fn test_match() {
        check_definition(
            r#"
                fun main() {
                    match (val x = 1) {
                        int => { <caret>x; }
                        else => {}
                    }
                }
            "#,
            expect![[r#"
                x -> Local(x at 61-62)
            "#]],
        );
    }

    #[test]
    fn test_match_with_return_and_throw() {
        check_definition(
            r#"
                fun main() {
                    match (val x = 1) {
                        1 => return <caret>x
                        2 => throw <caret>x
                        else => <caret>x
                    }
                }
            "#,
            expect![[r#"
                x -> Local(x at 61-62)
                x -> Local(x at 61-62)
                x -> Local(x at 61-62)
            "#]],
        );
    }

    #[test]
    fn test_match_arm() {
        check_definition(
            r#"
                type ctring = slice;

                fun main() {
                    match (1) {
                        <caret>ctring => {}
                        _ => {}
                    }
                }
            "#,
            expect![[r#"
                ctring -> Global(ctring at test.tolk:22-28)
            "#]],
        );
    }

    #[test]
    fn test_match_ambiguous_arm() {
        check_definition(
            r#"
                type ctring = slice;

                fun main() {
                    val ctring = "hello";
                    match (1) {
                        <caret>ctring => {}
                        _ => {}
                    }
                }
            "#,
            expect![[r#"
                ctring -> Local(ctring at 92-98)
            "#]],
        );
    }

    #[test]
    fn test_get_method_call() {
        check_definition(
            r#"
                get fun balance() {
                    return 100;
                }

                fun main() {
                    val b = <caret>balance();
                }
            "#,
            expect![[r#"
                balance -> Global(balance at test.tolk:25-32)
            "#]],
        );
    }

    #[test]
    fn test_struct_field_access() {
        check_definition(
            r#"
                struct Person {
                    name: string;
                    age: int;
                }

                fun main() {
                    val p: Person;
                    <caret>p.name;
                    <caret>p.age;
                }
            "#,
            expect![[r#"
                p -> Local(p at 169-170)
                p -> Local(p at 169-170)
            "#]],
        );
    }

    #[test]
    fn test_enum_member_access() {
        check_definition(
            r#"
                enum Color {
                    Red,
                    Green,
                    Blue
                }

                fun main() {
                    val c = <caret>Color.Red;
                    <caret>Color.Green;
                }
            "#,
            expect![[r#"
                Color -> Global(Color at test.tolk:22-27)
                Color -> Global(Color at test.tolk:22-27)
            "#]],
        );
    }

    #[test]
    fn test_function_call() {
        check_definition(
            r#"
                fun add(a: int, b: int): int {
                    return a + b;
                }

                fun main() {
                    val x = 1;
                    val y = 2;
                    val result = <caret>add(<caret>x, <caret>y);
                }
            "#,
            expect![[r#"
                add -> Global(add at test.tolk:21-24)
                x -> Local(x at 154-155)
                y -> Local(y at 185-186)
            "#]],
        );
    }

    #[test]
    fn test_object_literal() {
        check_definition(
            r#"
                struct Point {
                    x: int;
                    y: int;
                }

                fun main() {
                    val origin = <caret>Point { <caret>x: 0, <caret>y: 0 };
                }
            "#,
            expect![[r#"
                Point -> Global(Point at test.tolk:24-29)
                UnresolvedUnresolved"#]],
        );
    }

    #[test]
    fn test_method_call() {
        check_definition(
            r#"
                struct Calculator {
                    value: int;
                }

                fun Calculator.add(self, x: int) {
                    self.value += x;
                }

                fun main() {
                    val calc: Calculator;
                    <caret>calc.add(5);
                }
            "#,
            expect![[r#"
                calc -> Local(calc at 248-252)
            "#]],
        );
    }

    #[test]
    fn test_cast_as_operator() {
        check_definition(
            r#"
                type string2 = slice;
                fun main() {
                    val x: int = 42;
                    val y = <caret>x as <caret>string2;
                }
            "#,
            expect![[r#"
                x -> Local(x at 92-93)
                string2 -> Global(string2 at test.tolk:22-29)
            "#]],
        );
    }

    #[test]
    fn test_is_type_operator() {
        check_definition(
            r#"
                fun main() {
                    val x: int = 42;
                    val result = <caret>x is int;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_not_null_operator() {
        check_definition(
            r#"
                fun main() {
                    val x: int? = 42;
                    val result = <caret>x!;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_set_assignment() {
        check_definition(
            r#"
                fun main() {
                    var x = 1;
                    <caret>x += 5;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_ternary_operator() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    val y = 2;
                    val result = <caret>x > 0 ? <caret>x : <caret>y;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                x -> Local(x at 54-55)
                y -> Local(y at 85-86)
            "#]],
        );
    }

    #[test]
    fn test_lazy_expression() {
        check_definition(
            r#"
                fun expensive(): int {
                    return 42;
                }

                fun main() {
                    val x = true;
                    val result = <caret>x && lazy <caret>expensive();
                }
            "#,
            expect![[r#"
                x -> Local(x at 143-144)
                expensive -> Global(expensive at test.tolk:21-30)
            "#]],
        );
    }

    #[test]
    fn test_generic_instantiation() {
        check_definition(
            r#"
                fun identity<T>(x: T): T {
                    return x;
                }

                fun main() {
                    val result = <caret>identity<int>(<caret>identity<string>("hello"));
                }
            "#,
            expect![[r#"
                identity -> Global(identity at test.tolk:21-29)
                identity -> Global(identity at test.tolk:21-29)
            "#]],
        );
    }

    #[test]
    fn test_tensor_expression() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    val y = 2;
                    val tuple = (<caret>x, <caret>y);
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                y -> Local(y at 85-86)
            "#]],
        );
    }

    #[test]
    fn test_typed_tuple() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    val y = 2;
                    val tuple = [<caret>x, <caret>y];
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                y -> Local(y at 85-86)
            "#]],
        );
    }

    #[test]
    fn test_underscore() {
        check_definition(
            r#"
                fun main() {
                    match (val x = 1) {
                        int => { <caret>_ = 2; }
                        _ => {}
                    }
                }
            "#,
            expect!["Unresolved"],
        );
    }

    #[test]
    fn test_parenthesized_expression() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    val result = (<caret>x + 1) * 2;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_if_else() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    val y = 2;
                    if (<caret>x > 0) {
                        <caret>y;
                    } else {
                        <caret>x;
                    }
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                y -> Local(y at 85-86)
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_while_loop() {
        check_definition(
            r#"
                fun main() {
                    var x = 0;
                    while (<caret>x < 10) {
                        <caret>x += 1;
                    }
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_do_while_loop() {
        check_definition(
            r#"
                fun main() {
                    var x = 0;
                    do {
                        <caret>x += 1;
                    } while (<caret>x < 10);
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_repeat_loop() {
        check_definition(
            r#"
                fun main() {
                    val n = 5;
                    repeat (<caret>n) {
                        var x = 1;
                        <caret>x;
                    }
                }
            "#,
            expect![[r#"
                n -> Local(n at 54-55)
                x -> Local(x at 122-123)
            "#]],
        );
    }

    #[test]
    fn test_try_catch() {
        check_definition(
            r#"
                fun main() {
                    try {
                        val x = 1;
                        <caret>x;
                    } catch (e, arg) {
                        <caret>e;
                        <caret>arg;
                    }
                }
            "#,
            expect![[r#"
                x -> Local(x at 84-85)
                e -> Local(e at 147-148)
                arg -> Local(arg at 150-153)
            "#]],
        );
    }

    #[test]
    fn test_assert_statement() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    assert(<caret>x > 0, 100);
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_throw_statement() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    throw <caret>x;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_return_statement() {
        check_definition(
            r#"
                fun test() {
                    val x = 1;
                    return <caret>x;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_binary_operators() {
        check_definition(
            r#"
                fun main() {
                    val a = 1;
                    val b = 2;
                    val c = <caret>a + <caret>b;
                    val d = <caret>a && <caret>b;
                    val e = <caret>a == <caret>b;
                }
            "#,
            expect![[r#"
                a -> Local(a at 54-55)
                b -> Local(b at 85-86)
                a -> Local(a at 54-55)
                b -> Local(b at 85-86)
                a -> Local(a at 54-55)
                b -> Local(b at 85-86)
            "#]],
        );
    }

    #[test]
    fn test_unary_operators() {
        check_definition(
            r#"
                fun main() {
                    val x = true;
                    val y = -<caret>x;
                    val z = !<caret>x;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_type_parameters() {
        check_definition(
            r#"
                type MyType<T> = <caret>T;

                fun main() {
                    val result: <caret>MyType<int>;
                }
            "#,
            expect![[r#"
                T -> Local(T at 29-30)
                MyType -> Global(MyType at test.tolk:22-28)
            "#]],
        );
    }

    #[test]
    fn test_type_parameters_in_method_receiver() {
        check_definition(
            r#"
                struct Generic<T> {}

                fun Generic<T>.foo(): <caret>T {}
            "#,
            expect![[r#"
                T -> Local(T at 67-68)
            "#]],
        );
    }

    #[test]
    fn test_enum_with_backed_type() {
        check_definition(
            r#"
                type int = builtin;

                enum Status: <caret>int {
                    Active = 1,
                    Inactive = 0
                }

                fun main() {
                    val s: <caret>Status = <caret>Status.Active;
                }
            "#,
            expect![[r#"
                int -> Global(int at common.tolk:350-353)
                Status -> Global(Status at test.tolk:59-65)
                Status -> Global(Status at test.tolk:59-65)
            "#]],
        );
    }

    #[test]
    fn test_multiple_carets_same_symbol() {
        check_definition(
            r#"
                fun main() {
                    val x = 1;
                    <caret>x;
                    <caret>x;
                    <caret>x;
                }
            "#,
            expect![[r#"
                x -> Local(x at 54-55)
                x -> Local(x at 54-55)
                x -> Local(x at 54-55)
            "#]],
        );
    }

    #[test]
    fn test_local_var_references() {
        check_references(
            r#"
                fun main() {
                    val x = 1;
                    <def>x;
                    x;
                }
            "#,
            expect![[r#"
                x at 81-82
                x at 104-105
            "#]],
        );
    }

    #[test]
    fn test_function_param_references() {
        check_references(
            r#"
                fun foo(a: int) {
                    <def>a;
                    a;
                }
            "#,
            expect![[r#"
                a at 55-56
                a at 78-79
            "#]],
        );
    }

    #[test]
    fn test_global_function_references() {
        check_references(
            r#"
                fun foo() {}

                fun main() {
                    <def>foo();
                    foo();
                }
            "#,
            expect![[r#"
                foo at 80-83
                foo at 107-110
            "#]],
        );
    }

    #[test]
    fn test_struct_references() {
        check_references(
            r#"
                struct S { x: int }

                fun main() {
                    val s: <def>S;
                    val t: S;
                }
            "#,
            expect![[r#"
                S at 94-95
                S at 124-125
            "#]],
        );
    }

    #[test]
    fn test_nested_scopes_references() {
        check_references(
            r#"
                fun main() {
                    val x = 1;
                    {
                        val x = 2;
                        <def>x;
                    }
                    x;
                }
            "#,
            expect![[r#"
                x at 142-143
            "#]],
        );
    }

    #[test]
    fn test_import_references() {
        ResolveTestBuilder::new()
            .file("other.tolk", "fun other_fun() {}")
            .file(
                "main.tolk",
                r#"
                import "other.tolk"
                fun main() {
                    <def>other_fun();
                    other_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_references(expect![[r#"
                other_fun at 86-95
                other_fun at 119-128
            "#]]);
    }

    #[test]
    fn test_stdlib_import_references() {
        ResolveTestBuilder::new()
            .file(
                "main.tolk",
                r#"
                import "@stdlib/gas-payments"

                fun main() {
                    <def>getGasConsumedAtTheMoment();
                    getGasConsumedAtTheMoment();
                }
                "#,
            )
            .check_references(expect![[r#"
                getGasConsumedAtTheMoment at 97-122
                getGasConsumedAtTheMoment at 146-171
            "#]]);
    }

    #[test]
    fn test_nested_imports_references() {
        ResolveTestBuilder::new()
            .file("a.tolk", "fun a_fun() {}")
            .file(
                "b.tolk",
                r#"
                import "a.tolk"

                fun b_fun() { a_fun(); }
                "#,
            )
            .file(
                "main.tolk",
                r#"
                import "b.tolk"

                fun main() {
                    <def>b_fun();
                    b_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_references(expect![[r#"
                b_fun at 83-88
                b_fun at 112-117
            "#]]);
    }

    #[test]
    fn test_no_import_references() {
        ResolveTestBuilder::new()
            .file("other.tolk", "fun other_fun() {}")
            .file(
                "main.tolk",
                r#"
                fun main() {
                    <def>other_fun();
                    other_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_references(expect![[r#"
                Unresolved
            "#]]);
    }

    fn check_definition(input: &str, expect: Expect) {
        ResolveTestBuilder::new()
            .file("test.tolk", input)
            .check_definition(expect);
    }

    fn check_references(input: &str, expect: Expect) {
        ResolveTestBuilder::new()
            .file("test.tolk", input)
            .check_references(expect);
    }

    #[test]
    fn test_import() {
        ResolveTestBuilder::new()
            .file("other.tolk", "fun other_fun() {}")
            .file(
                "main.tolk",
                r#"
                import "other.tolk"
                fun main() {
                    <caret>other_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_definition(expect![[r#"
                other_fun -> Global(other_fun at other.tolk:4-13)
            "#]]);
    }

    #[test]
    fn test_mapped_import() {
        ResolveTestBuilder::new()
            .mapping("@core", "libs")
            .file("libs/math.tolk", "fun mapped_fun() {}")
            .file(
                "main.tolk",
                r#"
                import "@core/math"
                fun main() {
                    <caret>mapped_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_definition(expect![[r#"
                mapped_fun -> Global(mapped_fun at math.tolk:4-14)
            "#]]);
    }

    #[test]
    fn test_mapped_import_with_normalized_mapping_key() {
        ResolveTestBuilder::new()
            .mapping("core", "libs")
            .file("libs/math.tolk", "fun mapped_fun() {}")
            .file(
                "main.tolk",
                r#"
                import "@core/math"
                fun main() {
                    <caret>mapped_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_definition(expect![[r#"
                mapped_fun -> Global(mapped_fun at math.tolk:4-14)
            "#]]);
    }

    #[test]
    fn test_stdlib_import() {
        ResolveTestBuilder::new()
            .file(
                "main.tolk",
                r#"
                import "@stdlib/gas-payments"

                fun main() {
                    <caret>getGasConsumedAtTheMoment();
                }
                "#,
            )
            .check_definition(expect![[r#"
                getGasConsumedAtTheMoment -> Global(getGasConsumedAtTheMoment at gas-payments.tolk:180-205)
            "#]]);
    }

    #[test]
    fn test_nested_imports() {
        ResolveTestBuilder::new()
            .file("a.tolk", "fun a_fun() {}")
            .file(
                "b.tolk",
                r#"
                import "a.tolk"

                fun b_fun() { a_fun(); }
                "#,
            )
            .file(
                "main.tolk",
                r#"
                import "b.tolk"

                fun main() {
                    <caret>b_fun();
                    <caret>a_fun();
                }
                "#,
            )
            .target("main.tolk")
            .check_definition(expect![[r#"
                b_fun -> Global(b_fun at b.tolk:54-59)
                a_fun -> Unresolved
            "#]]);
    }

    #[derive(Clone, Copy)]
    enum CheckMode {
        Definition,
        References,
    }

    struct ResolveTestBuilder {
        files: Vec<(String, String)>,
        target_file: Option<String>,
        mappings: Option<BTreeMap<String, String>>,
    }

    impl ResolveTestBuilder {
        fn new() -> Self {
            Self {
                files: Vec::new(),
                target_file: None,
                mappings: None,
            }
        }

        fn mapping(mut self, prefix: &str, target: &str) -> Self {
            self.mappings
                .get_or_insert_with(BTreeMap::new)
                .insert(prefix.to_string(), target.to_string());
            self
        }

        fn file(mut self, path: &str, content: &str) -> Self {
            self.files.push((path.to_string(), content.to_string()));
            self
        }

        fn target(mut self, path: &str) -> Self {
            self.target_file = Some(path.to_string());
            self
        }

        fn check_definition(self, expect: Expect) {
            self.check_internal(expect, CheckMode::Definition);
        }

        fn check_references(self, expect: Expect) {
            self.check_internal(expect, CheckMode::References);
        }

        fn check_internal(self, expect: Expect, mode: CheckMode) {
            let target_file = self.target_file.clone().unwrap_or_else(|| {
                self.files
                    .first()
                    .map(|(p, _)| p.clone())
                    .expect("No files in test")
            });

            let temp_dir = tempfile::tempdir().unwrap();
            let project_root = temp_dir.path();

            let mut carets = Vec::new();
            let mut defs = Vec::new();
            let mut clean_target_content = String::new();
            let mut target_abs_path = None;

            for (path, content) in &self.files {
                let full_path = project_root.join(path);
                std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();

                if path == &target_file {
                    let mut pos = 0;
                    let mut chars = content.chars();
                    while let Some(c) = chars.next() {
                        if c == '<' {
                            // Check for <caret>
                            let mut next_chars = chars.clone();
                            let mut is_caret = true;
                            for caret_char in "caret>".chars() {
                                if next_chars.next() != Some(caret_char) {
                                    is_caret = false;
                                    break;
                                }
                            }
                            if is_caret {
                                carets.push(pos);
                                for _ in 0..6 {
                                    chars.next();
                                }
                                continue;
                            }

                            // Check for <def>
                            let mut next_chars = chars.clone();
                            let mut is_def = true;
                            for def_char in "def>".chars() {
                                if next_chars.next() != Some(def_char) {
                                    is_def = false;
                                    break;
                                }
                            }
                            if is_def {
                                defs.push(pos);
                                for _ in 0..4 {
                                    chars.next();
                                }
                                continue;
                            }
                        }
                        clean_target_content.push(c);
                        pos += c.len_utf8() as u32;
                    }
                    std::fs::write(&full_path, &clean_target_content).unwrap();
                    target_abs_path = Some(dunce::canonicalize(full_path).unwrap());
                } else {
                    std::fs::write(&full_path, content).unwrap();
                }
            }

            let target_abs_path = target_abs_path.expect("Target file not found in project files");

            let stdlib_path = PathBuf::from("../../crates/tolkc/assets/tolk-stdlib");
            let file_db = FileDb::new(stdlib_path.clone(), None);

            let stdlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(stdlib_path);
            let stdlib_path = dunce::canonicalize(stdlib_path).unwrap();

            let mappings = self.mappings.as_ref().map(|mappings| {
                mappings
                    .iter()
                    .map(|(key, value)| {
                        let value_path = PathBuf::from(value);
                        if value_path.is_absolute() {
                            (key.clone(), value.clone())
                        } else {
                            (
                                key.clone(),
                                project_root.join(value).to_string_lossy().to_string(),
                            )
                        }
                    })
                    .collect::<BTreeMap<_, _>>()
            });

            let mut project_index =
                crate::project_index::ProjectIndex::builder(&file_db, target_abs_path.clone())
                    .with_stdlib(stdlib_path)
                    .with_mappings(&mappings)
                    .build()
                    .unwrap();

            crate::symbol_resolver::resolve(&file_db, &mut project_index);

            let file_id = project_index
                .path_to_file_id()
                .get(&target_abs_path)
                .unwrap();
            let resolved_uses = project_index.get_resolved_uses(*file_id).unwrap();

            let mut actual = String::new();

            match mode {
                CheckMode::Definition => {
                    for caret_pos in carets {
                        let name_use = resolved_uses.find_use(caret_pos as usize);
                        match name_use {
                            Some(u) => {
                                let resolved_str = match &u.resolved {
                                    Resolved::Global(id) => {
                                        let symbol = project_index.resolve_symbol(*id).unwrap();
                                        let symbol_file =
                                            project_index.files().get(&id.file_id).unwrap();
                                        let file_name =
                                            symbol_file.path.file_name().unwrap().to_string_lossy();
                                        format!(
                                            "Global({} at {}:{})",
                                            symbol.name, file_name, symbol.name_span
                                        )
                                    }
                                    Resolved::Local(id) => {
                                        let local = resolved_uses
                                            .locals
                                            .iter()
                                            .find(|l| l.id == *id)
                                            .unwrap();
                                        format!("Local({} at {})", local.name, local.def_span)
                                    }
                                    Resolved::Unresolved => "Unresolved".to_string(),
                                };
                                actual.push_str(&format!("{} -> {}\n", u.name, resolved_str));
                            }
                            None => {
                                actual.push_str("Unresolved");
                            }
                        }
                    }
                }
                CheckMode::References => {
                    for def_pos in defs {
                        let name_use = resolved_uses.find_use(def_pos as usize);
                        if let Some(u) = name_use {
                            match &u.resolved {
                                Resolved::Global(symbol_id) => {
                                    let mut usages: Vec<_> =
                                        resolved_uses.global_usages_of(*symbol_id).collect();
                                    usages.sort_by_key(|u| u.span.start);
                                    for usage in usages {
                                        actual.push_str(&format!(
                                            "{} at {}\n",
                                            usage.name, usage.span
                                        ));
                                    }
                                }
                                Resolved::Local(local_id) => {
                                    let mut usages: Vec<_> =
                                        resolved_uses.local_usages_of(*local_id).collect();
                                    usages.sort_by_key(|u| u.span.start);
                                    for usage in usages {
                                        actual.push_str(&format!(
                                            "{} at {}\n",
                                            usage.name, usage.span
                                        ));
                                    }
                                }
                                Resolved::Unresolved => {
                                    actual.push_str("Unresolved\n");
                                }
                            }
                        } else {
                            actual.push_str("Unresolved\n");
                        }
                    }
                }
            }

            expect.assert_eq(&actual);
        }
    }
}
