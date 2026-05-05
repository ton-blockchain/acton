use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "S003";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_field_init_can_be_folded() {
    run_simple_test(
        "field_init_can_be_folded",
        r"
            struct Foo {
                bar: int,
            }

            fun fold(bar: int): Foo {
                return Foo {
                    bar: bar,
                };
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_field_init_can_be_folded() {
    run_fix_test(
        r"
            struct Foo {
                bar: int,
            }

            fun fold(bar: int): Foo {
                return Foo {
                    bar: bar,
                };
            }
        ",
        r"
            struct Foo {
                bar: int,
            }

            fun fold(bar: int): Foo {
                return Foo {
                    bar,
                };
            }
        ",
        function_name!(),
    );
}
