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
            @pure
            fun add(a: int, b: int): int {
                return a + b;
            }

            fun main() {
                add(1, 2);
            }
        ",
        function_name!(),
    );
}
