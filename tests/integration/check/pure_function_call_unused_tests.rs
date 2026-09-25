use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E006";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_pure_function_call_unused() {
    run_simple_test(
        "pure_function_call_unused",
        r"
            fun main() {
                createEmptyCell();
            }
        ",
        function_name!(),
    );
}
