use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E014";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_several_not_null_assertions_reports_double_assertion() {
    run_simple_test(
        "several_not_null_assertions",
        r"
            fun main(a: int?) {
                val b = a!!;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_several_not_null_assertions_ignores_single_assertion() {
    run_simple_test(
        "several_not_null_assertions",
        r"
            fun main(a: int?) {
                val b = a!;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_several_not_null_assertions_reports_only_outermost_chain() {
    run_simple_test(
        "several_not_null_assertions",
        r"
            fun main(a: int?) {
                val b = a!!!;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_several_not_null_assertions_double() {
    run_fix_test(
        r"
            fun main(a: int?) {
                val b = a!!;
                b;
            }
        ",
        r"
            fun main(a: int?) {
                val b = a!;
                b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_several_not_null_assertions_triple_in_single_pass() {
    run_fix_test(
        r"
            fun main(a: int?) {
                val b = a!!!;
                b;
            }
        ",
        r"
            fun main(a: int?) {
                val b = a!;
                b;
            }
        ",
        function_name!(),
    );
}
