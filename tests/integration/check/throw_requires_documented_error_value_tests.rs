use crate::integration::check::run_rule_check_test_with_files;
use crate::integration::check::run_rule_test;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const RULE_CODE: &str = "E036";

fn run_simple_test(content: &str, name: &str) {
    run_rule_test(
        "throw_requires_documented_error_value",
        RULE_CODE,
        content,
        name,
    );
}

#[test]
#[named]
fn test_check_throw_requires_documented_error_value_reports_throw_enum_value_without_comment() {
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
fn test_check_throw_requires_documented_error_value_reports_assert_enum_value_without_comment() {
    run_simple_test(
        r"
            enum Errors {
                InvalidMessage = 0xFFFF
            }

            fun main(in: InMessage) {
                assert (in.body.isEmpty()) throw Errors.InvalidMessage;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_documented_error_value_reports_trailing_comment() {
    run_simple_test(
        r"
            enum Errors {
                NotOwner = 401 // sender is not the current owner
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
fn test_check_throw_requires_documented_error_value_reports_plain_comment_above() {
    run_simple_test(
        r"
            enum Errors {
                // Sender is not the current owner.
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
fn test_check_throw_requires_documented_error_value_skips_doc_comment_immediately_above() {
    run_simple_test(
        r"
            enum Errors {
                /// Sender is not the current owner.
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
fn test_check_throw_requires_documented_error_value_reports_cast() {
    run_simple_test(
        r"
            enum Errors {
                NotFromAdmin = 401
            }

            fun main() {
                throw (Errors.NotFromAdmin as int);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_documented_error_value_reports_cross_file_declaration() {
    run_rule_check_test_with_files(
        "throw_requires_documented_error_value",
        RULE_CODE,
        r#"
            import "./errors.tolk";

            fun main() {
                throw Errors.NotOwner;
            }
        "#,
        &[(
            "contracts/errors",
            r"
                enum Errors {
                    NotOwner = 401
                }
            ",
        )],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_throw_requires_documented_error_value_is_allow_by_default() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "main",
            r"
                enum Errors {
                    NotOwner = 401
                }

                fun main() {
                    throw Errors.NotOwner;
                }
            ",
        )
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/throw_requires_documented_error_value/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn test_check_throw_requires_documented_error_value_can_be_enabled_from_config() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "main",
            r"
                enum Errors {
                    NotOwner = 401
                }

                fun main() {
                    throw Errors.NotOwner;
                }
            ",
        )
        .with_lint_level("throw-requires-documented-error-value", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/throw_requires_documented_error_value/{}.txt",
            function_name!()
        ));
}
