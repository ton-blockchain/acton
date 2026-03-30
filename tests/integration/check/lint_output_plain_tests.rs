use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const UNUSED_VARIABLE_CONTRACT: &str = r"
            fun main() {
                val x = 1;
            }
        ";

#[test]
#[named]
fn check_lint_plain_rejects_output_file_argument() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--output-file")
        .arg(".acton/reports/should-not-exist.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_output_plain_format/{}.stderr.txt",
            function_name!()
        ));

    assert!(
        !project
            .path()
            .join(".acton/reports/should-not-exist.json")
            .exists(),
        "report file should not be created when plain output format rejects --output-file"
    );
}
