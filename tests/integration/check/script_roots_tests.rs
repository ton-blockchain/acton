use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

#[test]
#[named]
fn check_project_wide_includes_scripts_with_main_using_relaxed_file_rules() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("main", "fun onInternalMessage(_: InMessage) {}")
        .script_file(
            "deploy",
            r#"
                import "../.acton/tlb/maybe.tolk";

                fun main() {
                    val maybeValue = TlbMaybe<int>.none();
                    maybeValue.unwrapOr(0);
                    val unused = 1;
                }
            "#,
        )
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_level("missing-contract-header", "allow")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/script_roots/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_project_wide_skips_workspace_files_without_main() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "alpha",
            r"
                fun main() {
                    val x = 1;
                }
            ",
        )
        .script_file(
            "helper",
            r"
                fun helper() {
                    val y = 1;
                }
            ",
        )
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
            "integration/snapshots/check/script_roots/{}.txt",
            function_name!()
        ));
}
