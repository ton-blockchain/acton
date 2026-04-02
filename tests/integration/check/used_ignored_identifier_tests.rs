use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E010";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_used_ignored_identifier_for_variable() {
    run_simple_test(
        "used_ignored_identifier",
        r"
            fun main() {
                val _value = 10;
                _value;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_used_ignored_identifier_for_parameter() {
    run_simple_test(
        "used_ignored_identifier",
        r"
            fun foo(_value: int): int {
                return _value + 1;
            }

            fun main() {
                foo(10);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_used_ignored_identifier_ignores_double_underscore() {
    run_simple_test(
        "used_ignored_identifier",
        r"
            fun main() {
                val __internal = 10;
                __internal;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_used_ignored_identifier_ignores_truly_unused_identifier() {
    run_simple_test(
        "used_ignored_identifier",
        r"
            fun main() {
                val _value = 10;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_used_ignored_identifier_for_variable() {
    run_fix_test(
        r"
            fun main() {
                val _value = 10;
                _value;
            }
        ",
        r"
            fun main() {
                val value = 10;
                value;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_used_ignored_identifier_for_multiple_variables() {
    run_fix_test(
        r"
            fun main() {
                val _a = 10;
                val _b = 20;
                _a + _b;
            }
        ",
        r"
            fun main() {
                val a = 10;
                val b = 20;
                a + b;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_used_ignored_identifier_for_parameter() {
    run_fix_test(
        r"
            fun foo(_value: int): int {
                return _value + _value;
            }

            fun main() {
                foo(10);
            }
        ",
        r"
            fun foo(value: int): int {
                return value + value;
            }

            fun main() {
                foo(10);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_used_ignored_identifier_ignores_double_underscore() {
    run_fix_test(
        r"
            fun main() {
                val __internal = 10;
                __internal;
            }
        ",
        r"
            fun main() {
                val __internal = 10;
                __internal;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_used_ignored_identifier_ignores_truly_unused_identifier() {
    run_fix_test(
        r"
            fun main() {
                val _value = 10;
            }
        ",
        r"
            fun main() {
                val _value = 10;
            }
        ",
        function_name!(),
    );
}
