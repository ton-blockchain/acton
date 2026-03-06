use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const UNUSED_VARIABLE_CONTRACT: &str = r#"
            fun main() {
                val x = 1;
            }
        "#;

const USED_IGNORED_IDENTIFIER_CONTRACT: &str = r#"
            fun main() {
                val _value = 1;
                _value;
            }
        "#;

#[test]
#[named]
fn check_lint_github_cli_overrides_config_output_format() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_output_format("github")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("github")
        .arg("--output-file")
        .arg(".acton/reports/from-cli.github")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_github_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/from-cli.github",
            &format!(
                "integration/snapshots/check/lint_output_github_format/{}.report.github",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_github_stderr_output_works() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("github")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_github_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_github_format/{}.stdout.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_github_writes_report_to_output_file() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("github")
        .arg("--output-file")
        .arg(".acton/reports/report.github")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_github_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/report.github",
            &format!(
                "integration/snapshots/check/lint_output_github_format/{}.report.github",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_github_includes_secondary_annotations_help_and_applicability() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", USED_IGNORED_IDENTIFIER_CONTRACT)
        .with_lint_level("used-ignored-identifier", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("github")
        .arg("--output-file")
        .arg(".acton/reports/metadata.github")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_github_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/metadata.github",
            &format!(
                "integration/snapshots/check/lint_output_github_format/{}.report.github",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_github_writes_report_to_output_file_even_when_exit_code_is_non_zero() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_max_warnings(0)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("github")
        .arg("--output-file")
        .arg(".acton/reports/non-zero.github")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_github_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/non-zero.github",
            &format!(
                "integration/snapshots/check/lint_output_github_format/{}.report.github",
                function_name!()
            ),
        );
}
