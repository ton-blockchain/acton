use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E034";

fn run_simple_test(content: &str, name: &str) {
    run_rule_test("throw_requires_errors_enum", RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_reports_throw_constant() {
    run_simple_test(
        r"
            const ERR_NOT_OWNER = 401

            fun main() {
                throw ERR_NOT_OWNER;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_reports_assert_constant() {
    run_simple_test(
        r"
            const ERR_INVALID_MESSAGE = 0xFFFF

            fun main(in: InMessage) {
                assert (in.body.isEmpty()) throw ERR_INVALID_MESSAGE;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_reports_tuple_constant() {
    run_simple_test(
        r"
            const ERR_NOT_FOUND = 404

            fun main(id: int) {
                throw (ERR_NOT_FOUND, id);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_skips_errors_enum_member() {
    run_simple_test(
        r"
            enum Errors {
                NotOwner = 401
            }

            fun main() {
                throw Errors.NotOwner;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_skips_tuple_errors_enum_member() {
    run_simple_test(
        r"
            enum Errors {
                NotFound = 404
            }

            fun main(id: int) {
                throw (Errors.NotFound, id);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_skips_numeric_literal() {
    run_simple_test(
        r"
            fun main() {
                throw 401;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_errors_enum_skips_local_rethrow() {
    run_simple_test(
        r"
            fun main() {
                try {
                    throw 1;
                } catch (e, _) {
                    throw e;
                }
            }
        ",
        function_name!(),
    );
}
