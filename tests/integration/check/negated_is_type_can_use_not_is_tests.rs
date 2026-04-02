use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E022";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_negated_is_type_can_use_not_is_reports_negated_is() {
    run_simple_test(
        "negated_is_type_can_use_not_is",
        r"
            fun main(a: int?) {
                val b = !(a is int);
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_negated_is_type_can_use_not_is_reports_with_nested_parens() {
    run_simple_test(
        "negated_is_type_can_use_not_is",
        r"
            fun main(a: int?) {
                val b = !(((a)) is int);
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_negated_is_type_can_use_not_is_ignores_not_is() {
    run_simple_test(
        "negated_is_type_can_use_not_is",
        r"
            fun main(a: int?) {
                val b = a !is int;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_negated_is_type_can_use_not_is_ignores_non_is_negation() {
    run_simple_test(
        "negated_is_type_can_use_not_is",
        r"
            fun main(flag: bool) {
                val b = !flag;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_negated_is_type_can_use_not_is_ignores_negated_not_is() {
    run_simple_test(
        "negated_is_type_can_use_not_is",
        r"
            fun main(a: int?) {
                val b = !(a !is int);
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_negated_is_type_can_use_not_is() {
    run_fix_test(
        r"
            fun main(a: int?) {
                val b = !(a is int);
                b;
            }
        ",
        r"
            fun main(a: int?) {
                val b = a !is int;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_negated_is_type_can_use_not_is_with_nested_parens() {
    run_fix_test(
        r"
            fun main(a: int?) {
                val b = !(((a)) is int);
                b;
            }
        ",
        r"
            fun main(a: int?) {
                val b = ((a)) !is int;
                b;
            }
        ",
        function_name!(),
    );
}
