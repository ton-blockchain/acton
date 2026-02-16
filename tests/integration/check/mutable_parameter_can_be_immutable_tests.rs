use crate::integration::check::run_fix_test;
use crate::integration::check::run_simple_test;
use function_name::named;

#[test]
#[named]
fn test_check_mutable_parameter_can_be_immutable() {
    run_simple_test(
        "mutable_parameter_can_be_immutable",
        r#"
            fun foo(mutate a: int): int {
                return a + 1;
            }

            fun main() {
                var value = 10;
                foo(mutate value);
                value;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_parameter_can_be_immutable_with_actual_write_to() {
    run_simple_test(
        "mutable_parameter_can_be_immutable",
        r#"
            fun foo(mutate a: int): int {
                a += 1;
                return a;
            }

            fun main() {
                var value = 10;
                foo(mutate value);
                value;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_parameter_can_be_immutable_with_usage_as_mutate_argument() {
    run_simple_test(
        "mutable_parameter_can_be_immutable",
        r#"
            fun touch(mutate x: int) {
                x += 1;
            }

            fun foo(mutate a: int): int {
                touch(mutate a);
                return a;
            }

            fun main() {
                var value = 10;
                foo(mutate value);
                value;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_parameter_can_be_immutable_for_immutable_parameter() {
    run_simple_test(
        "mutable_parameter_can_be_immutable",
        r#"
            fun foo(a: int): int {
                return a + 1;
            }

            fun main() {
                foo(10);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_mutable_parameter_can_be_immutable_without_usages() {
    run_simple_test(
        "mutable_parameter_can_be_immutable",
        r#"
            fun foo(mutate _a: int): int {
                return 10;
            }

            fun main() {
                var value = 10;
                foo(mutate value);
                value;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_mutable_parameter_can_be_immutable() {
    run_fix_test(
        r#"
            fun foo(mutate a: int): int {
                return a + 1;
            }
        "#,
        r#"
            fun foo(a: int): int {
                return a + 1;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_mutable_parameter_can_be_immutable_for_multiple_parameters() {
    run_fix_test(
        r#"
            fun foo(mutate a: int, mutate b: int, mutate c: int): int {
                b += 1;
                return a + b + c;
            }
        "#,
        r#"
            fun foo(a: int, mutate b: int, c: int): int {
                b += 1;
                return a + b + c;
            }
        "#,
        function_name!(),
    );
}
