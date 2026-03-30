use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E028";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_no_global_variables_reports_global_declaration() {
    run_simple_test(
        "no_global_variables",
        r"
            global counter: int

            fun main() {
                 counter = 100;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_no_global_variables_ignores_local_variables() {
    run_simple_test(
        "no_global_variables",
        r"
            fun main() {
                val counter = 1;
                val total = counter + 41;
                total;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_no_global_variables_reports_multiple_global_declarations() {
    run_simple_test(
        "no_global_variables",
        r"
            global counter: int
            global owner: address

            fun main() {
                counter;
                owner;
            }
        ",
        function_name!(),
    );
}
