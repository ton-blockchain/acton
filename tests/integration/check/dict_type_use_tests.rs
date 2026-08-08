use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E027";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_dict_type_use_reports_parameter_type() {
    run_simple_test(
        "dict_type_use",
        r"
            fun main(data: dict) {}
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dict_type_use_reports_type_alias_rhs() {
    run_simple_test(
        "dict_type_use",
        r"
            type StorageDict = dict

            fun main(data: StorageDict) {
                data;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dict_type_use_ignores_map_type() {
    run_simple_test(
        "dict_type_use",
        r"
            fun main(data: map<uint32, cell>) {
                data;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dict_type_use_respects_suppression() {
    run_simple_test(
        "dict_type_use",
        r"
            // check-disable-next-line dict-type-use
            fun main(data: dict) {
                data;
            }
        ",
        function_name!(),
    );
}
