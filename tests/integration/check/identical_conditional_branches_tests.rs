use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E027";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_identical_conditional_branches_reports_if_else_with_identical_returns() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, value: int): int {
                if (flag) {
                    return value + 1;
                } else {
                    return value + 1;
                }
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_reports_if_else_ignoring_comments_and_semicolons() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, value: int): int {
                if (flag) {
                    // same branch
                    return value;;
                } else {
                    return value;
                }
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_ignores_if_without_else() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, value: int) {
                if (flag) {
                    debug.print(value);
                }
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_ignores_if_else_with_different_identifiers() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, left: int, right: int): int {
                if (flag) {
                    return left;
                } else {
                    return right;
                }
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_ignores_else_if_with_different_branches() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, other: bool, a: int, b: int): int {
                if (flag) {
                    return a;
                } else if (other) {
                    return a;
                } else {
                    return b;
                }
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_reports_ternary_with_identical_arms() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, value: int): int {
                val result = flag ? value + 1 : value + 1;
                return result;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_reports_ternary_ignoring_comments() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, value: int): int {
                val result = flag ? (value + 1 /* same */) : (value + 1);
                return result;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_ignores_ternary_with_different_identifiers() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, left: int, right: int): int {
                val result = flag ? left : right;
                return result;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_ignores_ternary_with_different_operators() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool, value: int): int {
                val result = flag ? (value + 1) : (value - 1);
                return result;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_ignores_ternary_with_different_literals() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun main(flag: bool): int {
                val result = flag ? 1 : 2;
                return result;
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_identical_conditional_branches_reports_ternary_inside_call_argument() {
    run_simple_test(
        "identical_conditional_branches",
        r#"
            fun id(x: int): int {
                return x;
            }

            fun main(flag: bool, value: int): int {
                return id(flag ? value : value);
            }
        "#,
        function_name!(),
    );
}
