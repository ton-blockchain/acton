use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const UNUSED_VARIABLE_CONTRACT: &str = r#"
            fun main() {
                val x = 1;
            }
        "#;

const HELPER_WITH_UNUSED_VARIABLE: &str = r#"
            fun dangerous() {
                val helper_unused = 1;
            }
        "#;

const BROKEN_HELPER: &str = r#"
            fun broken(value: int): int {
                return value +
            }
        "#;

#[test]
#[named]
fn check_lint_exclude_skips_excluded_contract_root() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("alpha", UNUSED_VARIABLE_CONTRACT)
        .contract("beta", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_exclude("contracts/beta.tolk")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exclude/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exclude_hides_dependency_diagnostics_but_keeps_graph_valid() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "main",
            r#"
                import "./helper.tolk";

                fun main() {
                    dangerous();
                }
            "#,
        )
        .file("contracts/helper", HELPER_WITH_UNUSED_VARIABLE)
        .with_lint_level("unused-variable", "warn")
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_exclude("contracts/helper.tolk")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exclude/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exclude_is_ignored_for_explicit_target() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("alpha", UNUSED_VARIABLE_CONTRACT)
        .contract("beta", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_level("unused-variable", "warn")
        .with_lint_exclude("contracts/beta.tolk")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("beta")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exclude/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exclude_is_ignored_for_explicit_target_path() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract("alpha", UNUSED_VARIABLE_CONTRACT)
        .contract("beta", UNUSED_VARIABLE_CONTRACT)
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_level("unused-variable", "warn")
        .with_lint_exclude("contracts/beta.tolk")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("contracts/beta.tolk")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exclude/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn check_lint_exclude_does_not_hide_compiler_errors_from_excluded_files() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "main",
            r#"
                import "./broken_helper.tolk";

                fun onInternalMessage(_in: InMessage) {
                    broken(1);
                }
            "#,
        )
        .file("contracts/broken_helper", BROKEN_HELPER)
        .with_lint_level("explicit-return-type", "allow")
        .with_lint_exclude("contracts/broken_helper.tolk")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .code(1)
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/lint_exclude/{}.txt",
            function_name!()
        ));
}
