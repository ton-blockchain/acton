use crate::integration::check::run_simple_test;
use function_name::named;

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                var a = 100;
                foo(a);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_tensor_decl() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                var (a, b) = (100, 200);
                b = 100;
                foo(a + b);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_tuple_decl() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                var [a, b] = [100, 200];
                b = 100;
                foo(a + b);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_for_immutable_variable() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                val a = 100;
                foo(a);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_without_usages() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                var a = 100;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_actual_write_to() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                var a = 100;
                a = 200;
                foo(a);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_actual_set_write_to() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: int) {}

            fun main() {
                var a = 100;
                a += 200;
                foo(a);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_usage_as_mutate_argument() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(mutate _a: int) {}

            fun main() {
                var a = 100;
                foo(mutate a);
                a;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_actual_write_to_index() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            fun foo(_a: (int, int)) {}

            fun main() {
                var a = (100, 200);
                a.1 = 200;
                foo(a);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_actual_write_to_field() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            struct Foo { a: int }

            fun foo(_a: Foo) {}

            fun main() {
                var a = Foo { a: 10 };
                a.a = 200;
                foo(a);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_call_of_immutable_method() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            struct Foo { a: int }
            fun Foo.bar(self) { self }

            fun main() {
                var a = Foo { a: 10 };
                a.bar();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_call_of_mutable_method() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            struct Foo { a: int }
            fun Foo.bar(mutate self) { self }

            fun main() {
                var a = Foo { a: 10 };
                a.bar();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_variable_can_be_immutable_with_call_of_unresolved_method() {
    run_simple_test(
        "mutable_variable_can_be_immutable",
        r#"
            struct Foo { a: int }

            fun main() {
                var a = Foo { a: 10 };
                a.some();
            }
        "#,
        function_name!(),
    )
}
