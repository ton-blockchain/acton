use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const UNUSED_VARIABLE_CONTRACT: &str = r#"
            fun main() {
                val x = 1;
            }
        "#;

const BROKEN_CONTRACT: &str = r#"
            fun main() {
                val x =
            }
        "#;

#[test]
#[named]
fn check_lint_exit_code_is_non_zero_when_errors_present() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", BROKEN_CONTRACT)
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .code(1)
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exit_code/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exit_code_stays_zero_for_warnings_without_limit() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exit_code/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exit_code_is_non_zero_when_max_warnings_exceeded() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_max_warnings(0)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .code(1)
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exit_code/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exit_code_stays_zero_when_max_warnings_not_exceeded() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_max_warnings(1)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exit_code/{}.txt",
            function_name!()
        ));
}
