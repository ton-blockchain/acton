use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E026";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_duplicated_condition_reports_duplicated_else_if_condition() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (a < 1) {
                    return 1;
                } else if (a > 4) {
                    return 2;
                } else if (a > 4) {
                    return 3;
                }
                return 4;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_reports_non_adjacent_else_if_duplicate() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (a > 4) {
                    return 1;
                } else if (a < 0) {
                    return 2;
                } else if (a > 4) {
                    return 3;
                }
                return 4;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_ignores_distinct_else_if_conditions() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (a < 1) {
                    return 1;
                } else if (a > 4) {
                    return 2;
                } else if (a > 5) {
                    return 3;
                }
                return 4;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_ignores_if_without_else_if_chain() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (a > 4) {
                    return 1;
                }
                return 2;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_reports_complex_arithmetic_expression_duplicate() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (((a + 1) * (a - 2)) > 10) {
                    return 1;
                } else if (((a + 1) * (a - 2)) > 10) {
                    return 2;
                }
                return 3;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_reports_duplicate_with_comments_in_condition() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if ((a + 1 /* keep */) > 10) {
                    return 1;
                } else if ((a + 1) > 10) {
                    return 2;
                }
                return 3;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_reports_duplicate_across_long_chain() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (a > 100) {
                    return 1;
                } else if (a < 0) {
                    return 2;
                } else if (a == 42) {
                    return 3;
                } else if (a > 100) {
                    return 4;
                }
                return 5;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_reports_multiple_duplicates_in_same_chain() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (a > 7) {
                    return 1;
                } else if (a < 0) {
                    return 2;
                } else if (a > 7) {
                    return 3;
                } else if (a > 7) {
                    return 4;
                }
                return 5;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_ignores_complex_conditions_with_different_literals() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int): int {
                if (((a + 1) * (a - 2)) > 10) {
                    return 1;
                } else if (((a + 1) * (a - 2)) > 11) {
                    return 2;
                }
                return 3;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_duplicated_condition_ignores_complex_conditions_with_different_identifiers() {
    run_simple_test(
        "duplicated_condition",
        r"
            fun main(a: int, b: int): int {
                if (((a + 1) * (a - 2)) > 10) {
                    return 1;
                } else if (((b + 1) * (b - 2)) > 10) {
                    return 2;
                }
                return 3;
            }
        ",
        function_name!(),
    );
}
