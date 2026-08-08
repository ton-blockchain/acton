use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E001";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_unused_variable() {
    run_simple_test(
        "unused_variable",
        r"
            fun main() {
                val unused = 1;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unused_variable() {
    run_fix_test(
        r"
            fun main() {
                val unused = 1;
            }
        ",
        r"
            fun main() {
                val _unused = 1;
            }
        ",
        function_name!(),
    );
}
