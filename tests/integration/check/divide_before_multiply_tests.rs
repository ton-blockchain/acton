use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

fn run_divide_before_multiply_test(content: &str, name: &str) {
    let project = ProjectBuilder::new(&format!("check-{name}"))
        .contract("main", content)
        .with_lint_level("divide-before-multiply", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E019")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/divide_before_multiply/{name}.txt"
        ));
}

#[test]
#[named]
fn test_check_divide_before_multiply_reports_direct_left_division_in_multiplication() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                debug.print(a / b * c);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_reports_direct_right_division_in_multiplication() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                debug.print(c * (a / b));
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_allows_multiplication_before_division() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                debug.print(a * c / b);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_allows_parenthesized_multiplication_before_division() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                debug.print((a * c) / b);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_reports_tainted_variable_used_in_multiplication() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                val t = a / b;
                debug.print(t * c);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_reports_tainted_variable_through_assignment_chain() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                val t = a / b;
                val u = t + 1;
                debug.print(u * c);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_allows_division_nested_inside_function_call_arguments() {
    run_divide_before_multiply_test(
        r"
            fun passthrough(x: int): int {
                return x;
            }

            fun main(a: int, b: int, c: int) {
                val config = passthrough(a / b);
                debug.print(config * c);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_allows_loop_carried_self_taint_when_expression_multiplies_before_dividing()
 {
    run_divide_before_multiply_test(
        r"
            fun main(a: int) {
                var x = a;
                var i = 3;
                while (i > 0) {
                    x = x * 90 / 100;
                    i = i - 1;
                }
                debug.print(x);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_allows_when_division_happens_after_multiplication() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                val m = a * c;
                val t = a / b;
                debug.print(m + t);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_reports_when_taint_flows_from_conditional_branch() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int, cond: bool) {
                var t = 1;
                if (cond) {
                    t = a / b;
                }
                debug.print(t * c);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_allows_without_multiplication() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int) {
                val v = a / b;
                debug.print(v + c);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_divide_before_multiply_reports_in_return_expression() {
    run_divide_before_multiply_test(
        r"
            fun main(a: int, b: int, c: int): int {
                return (a / b) * c;
            }
        ",
        function_name!(),
    );
}
