use crate::integration::check::deprecated_tests::run_simple_test;
use function_name::named;

#[test]
#[named]
fn test_check_method_can_be_static_with_unused_self() {
    run_simple_test(
        "method_can_be_static",
        r#"
            struct Foo {}

            fun Foo.bar(self, a: int): int {
                return a + 1;
            }

            fun main() {
                Foo{}.bar(10);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_method_can_be_static_with_used_self() {
    run_simple_test(
        "method_can_be_static",
        r#"
            struct Foo {
                value: int,
            }

            fun Foo.bar(self): int {
                return self.value + 1;
            }

            fun main() {
                Foo { value: 10 }.bar();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_method_can_be_static_with_recursive_self_only() {
    run_simple_test(
        "method_can_be_static",
        r#"
            struct Foo {}

            fun Foo.bar(self, n: int): int {
                if (n <= 0) {
                    return 0;
                }
                return self.bar(n - 1);
            }

            fun main() {
                Foo{}.bar(2);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_method_can_be_static_with_recursive_and_normal_self_usage() {
    run_simple_test(
        "method_can_be_static",
        r#"
            struct Foo {
                value: int,
            }

            fun Foo.bar(self, n: int): int {
                if (n <= 0) {
                    return self.value;
                }
                return self.bar(n - 1);
            }

            fun main() {
                Foo { value: 2 }.bar(2);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_method_can_be_static_for_static_method() {
    run_simple_test(
        "method_can_be_static",
        r#"
            struct Foo {}

            fun Foo.bar(_n: int): int {
                return 1;
            }

            fun main() {
                Foo.bar(2);
            }
        "#,
        function_name!(),
    )
}
