use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const UNUSED_VARIABLE_CONTRACT: &str = r#"
            fun main() {
                val x = 1;
            }
        "#;

#[test]
#[named]
fn check_lint_sarif_writes_report_from_config() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_sarif_path(".acton/reports/lint.sarif")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_file_snapshot_matches(
            ".acton/reports/lint.sarif",
            &format!(
                "integration/snapshots/check/lint_sarif/{}.sarif.json",
                function_name!()
            ),
        );
}

#[test]
#[named]
fn check_lint_sarif_cli_overrides_config_path() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_sarif_path(".acton/reports/from-config.sarif")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--sarif")
        .arg(".acton/reports/from-cli.sarif")
        .run()
        .success()
        .assert_file_snapshot_matches(
            ".acton/reports/from-cli.sarif",
            &format!(
                "integration/snapshots/check/lint_sarif/{}.sarif.json",
                function_name!()
            ),
        );

    assert!(
        !project.path().join(".acton/reports/from-config.sarif").exists(),
        "Config SARIF path should be ignored when --sarif is passed"
    );
}
