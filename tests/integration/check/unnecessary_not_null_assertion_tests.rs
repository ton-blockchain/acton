use crate::integration::check::run_rule_test;
use crate::support::project::ProjectBuilder;
use function_name::named;

const RULE_CODE: &str = "E031";
const SNAPSHOT_GROUP: &str = "unnecessary_not_null_assertion";

fn run_simple_test(content: &str, name: &str) {
    run_rule_test(SNAPSHOT_GROUP, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_unnecessary_not_null_assertion_reports_non_null_expressions() {
    run_simple_test(
        r"
            fun main(nullable: int?, nonNullable: int) {
                val direct = nonNullable!;
                val coalesced = (nullable ?? 0)!;

                if (nullable != null) {
                    val smartCasted = nullable!;
                    direct + coalesced + smartCasted;
                }
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_unnecessary_not_null_assertion_ignores_nullable_values() {
    run_simple_test(
        r"
            fun main(nullable: int?) {
                val asserted = nullable!;
                asserted;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_unnecessary_not_null_assertion_reports_the_outer_operator_in_a_chain() {
    run_simple_test(
        r"
            fun main(nullable: int?) {
                val asserted = nullable!!;
                asserted;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_unnecessary_not_null_assertion_handles_generic_t_or_null() {
    run_simple_test(
        r"
            fun force<T>(value: T | null): T {
                return value!;
            }

            fun forceAfterCheck<T>(value: T | null): T {
                if (value != null) {
                    return value!;
                }
                throw 5;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unnecessary_not_null_assertion_removes_only_the_operator() {
    let project = ProjectBuilder::new("check-fix-unnecessary-not-null-assertion")
        .contract(
            "main",
            r"fun main(value: int, nullable: int?) {
    val same = value /* keep this comment */ !;
    val repeated = nullable!!;
    same + repeated;
}
",
        )
        .build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E014,E031")
        .arg("--fix")
        .run()
        .success();

    let fixed = std::fs::read_to_string(project.path().join("contracts/main.tolk"))
        .expect("fixed contract must be readable");
    snapbox::assert_data_eq!(
        format!("{}\n", fixed.trim()),
        snapbox::file!(
            "../snapshots/check/unnecessary_not_null_assertion/test_fix_unnecessary_not_null_assertion_removes_only_the_operator.tolk"
        )
    );
}
