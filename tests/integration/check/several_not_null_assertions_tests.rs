use crate::integration::check::run_fix_test;
use crate::integration::check::run_simple_test;
use function_name::named;

#[test]
#[named]
fn test_check_several_not_null_assertions_reports_double_assertion() {
    run_simple_test(
        "several_not_null_assertions",
        r#"
            fun main(a: int?) {
                val b = a!!;
                b;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_several_not_null_assertions_ignores_single_assertion() {
    run_simple_test(
        "several_not_null_assertions",
        r#"
            fun main(a: int?) {
                val b = a!;
                b;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_several_not_null_assertions_reports_only_outermost_chain() {
    run_simple_test(
        "several_not_null_assertions",
        r#"
            fun main(a: int?) {
                val b = a!!!;
                b;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_several_not_null_assertions_double() {
    run_fix_test(
        r#"
            fun main(a: int?) {
                val b = a!!;
                b;
            }
        "#,
        r#"
            fun main(a: int?) {
                val b = a!;
                b;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_several_not_null_assertions_triple_in_single_pass() {
    run_fix_test(
        r#"
            fun main(a: int?) {
                val b = a!!!;
                b;
            }
        "#,
        r#"
            fun main(a: int?) {
                val b = a!;
                b;
            }
        "#,
        function_name!(),
    );
}
