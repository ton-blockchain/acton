use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const UNUSED_VARIABLE_CONTRACT: &str = r"
            fun main() {
                val x = 1;
            }
        ";

const USED_IGNORED_IDENTIFIER_CONTRACT: &str = r"
            fun main() {
                val _value = 1;
                _value;
            }
        ";

#[test]
#[named]
fn check_lint_gitlab_cli_overrides_config_output_format() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_output_format("sarif")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("--output-format")
        .arg("gitlab")
        .arg("--output-file")
        .arg(".acton/reports/from-cli.gl-code-quality.json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/from-cli.gl-code-quality.json",
            &format!(
                "integration/snapshots/check/lint_output_gitlab_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_gitlab_stderr_output_works() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("--output-format")
        .arg("gitlab")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stderr.json",
            function_name!()
        ))
        .assert_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stdout.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_gitlab_uses_output_format_from_config() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_output_format("gitlab")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .run()
        .success()
        .assert_contains("\"check_name\"")
        .assert_contains("\"location\"")
        .assert_not_contains("Checking");
}

#[test]
#[named]
fn check_lint_gitlab_writes_report_to_output_file() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("--output-format")
        .arg("gitlab")
        .arg("--output-file")
        .arg(".acton/reports/report.gl-code-quality.json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/report.gl-code-quality.json",
            &format!(
                "integration/snapshots/check/lint_output_gitlab_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_gitlab_includes_secondary_annotations_help_and_applicability() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", USED_IGNORED_IDENTIFIER_CONTRACT)
        .with_lint_level("used-ignored-identifier", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("--output-format")
        .arg("gitlab")
        .arg("--output-file")
        .arg(".acton/reports/metadata.gl-code-quality.json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/metadata.gl-code-quality.json",
            &format!(
                "integration/snapshots/check/lint_output_gitlab_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_gitlab_writes_report_to_output_file_even_when_exit_code_is_non_zero() {
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
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("--output-format")
        .arg("gitlab")
        .arg("--output-file")
        .arg(".acton/reports/non-zero.gl-code-quality.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/non-zero.gl-code-quality.json",
            &format!(
                "integration/snapshots/check/lint_output_gitlab_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_gitlab_rejects_fix_with_non_plain_output() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .arg("--output-format")
        .arg("gitlab")
        .arg("--fix")
        .arg("--output-file")
        .arg(".acton/reports/fixed.gl-code-quality.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_gitlab_format/{}.stderr.txt",
            function_name!()
        ));

    assert!(
        !project
            .path()
            .join(".acton/reports/fixed.gl-code-quality.json")
            .exists(),
        "report file should not be created when --fix is rejected"
    );
    assert!(
        std::fs::read_to_string(project.path().join("contracts/main.tolk"))
            .expect("contract source should be readable")
            .contains("val x = 1;"),
        "contract source should not be fixed when --fix is rejected"
    );
}
