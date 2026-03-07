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
fn check_lint_json_cli_overrides_config_output_format() {
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
        .arg("--output-format")
        .arg("json")
        .arg("--output-file")
        .arg(".acton/reports/from-cli.json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_json_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/from-cli.json",
            &format!(
                "integration/snapshots/check/lint_output_json_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_json_stderr_output_works() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_json_format/{}.stderr.json",
            function_name!()
        ))
        .assert_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_json_format/{}.stdout.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_json_uses_output_format_from_config() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_output_format("json")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_contains("\"diagnostics\"")
        .assert_contains("\"success\": true")
        .assert_not_contains("Checking");
}

#[test]
#[named]
fn check_lint_json_writes_report_to_output_file() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("json")
        .arg("--output-file")
        .arg(".acton/reports/report.json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_json_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/report.json",
            &format!(
                "integration/snapshots/check/lint_output_json_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_json_includes_secondary_annotations_help_and_applicability() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", USED_IGNORED_IDENTIFIER_CONTRACT)
        .with_lint_level("used-ignored-identifier", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("json")
        .arg("--output-file")
        .arg(".acton/reports/metadata.json")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_json_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/metadata.json",
            &format!(
                "integration/snapshots/check/lint_output_json_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_json_writes_report_to_output_file_even_when_exit_code_is_non_zero() {
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
        .arg("--output-format")
        .arg("json")
        .arg("--output-file")
        .arg(".acton/reports/non-zero.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_json_format/{}.stderr.txt",
            function_name!()
        ))
        .assert_file_snapshot_matches(
            ".acton/reports/non-zero.json",
            &format!(
                "integration/snapshots/check/lint_output_json_format/{}.report.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_json_fix_does_not_apply_with_non_plain_output() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-format")
        .arg("json")
        .arg("--fix")
        .arg("--output-file")
        .arg(".acton/reports/fixed.json")
        .run()
        .success()
        .assert_file_contains(".acton/reports/fixed.json", "\"name\": \"unused-variable\"")
        .assert_file_contains(".acton/reports/fixed.json", "\"applicability\": \"auto\"")
        .assert_file_contains("contracts/main.tolk", "val x = 1;");
}
