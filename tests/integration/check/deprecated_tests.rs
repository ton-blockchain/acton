use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E004";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_deprecated_function_use() {
    run_simple_test(
        "deprecated",
        r"
            @deprecated
            fun foo() {}

            fun main() {
                foo();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_deprecated_function_use_with_message() {
    run_simple_test(
        "deprecated",
        r#"
            @deprecated("use bar instead")
            fun foo() {}

            fun main() {
                foo();
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_deprecated_struct_use() {
    run_simple_test(
        "deprecated",
        r#"
            @deprecated("use Bar instead")
            struct Foo {}

            fun main(_a: Foo) {
                Foo {}
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_deprecated_static_method_use() {
    run_simple_test(
        "deprecated",
        r#"
            struct Foo {}

            @deprecated("use Foo.baz instead")
            fun Foo.bar() {}

            fun main() {
                Foo.bar();
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_deprecated_instance_method_use() {
    run_simple_test(
        "deprecated",
        r#"
            struct Foo {}

            @deprecated("use Foo.baz instead")
            fun Foo.bar(self) {}

            fun main() {
                Foo{}.bar();
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_deprecated_global_var_use() {
    run_simple_test(
        "deprecated",
        r"
            @deprecated
            global foo: int

            fun main() {
                foo;
            }
        ",
        function_name!(),
    );
}
