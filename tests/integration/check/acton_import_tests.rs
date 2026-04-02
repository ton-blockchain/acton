use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

#[test]
#[named]
fn test_check_acton_import_in_contract_direct() {
    run_check(
        "acton_import",
        r#"
            import "../.acton/tlb/maybe.tolk";

            fun onInternalMessage(_: InMessage) {
                val maybeValue = Maybe<int>.none();
                maybeValue.unwrapOr(0);
            }
        "#,
        &[],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_acton_import_in_transitive_dependency() {
    run_check(
        "acton_import",
        r#"
            import "./helper.tolk";

            fun onInternalMessage(_: InMessage) {
                useMaybe();
            }
        "#,
        &[(
            "contracts/helper",
            r#"
                import "../.acton/tlb/maybe.tolk";

                fun useMaybe() {
                    val maybeValue = Maybe<int>.some(1);
                    maybeValue.unwrapOr(0);
                }
            "#,
        )],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_acton_import_with_mappings_direct() {
    let project = ProjectBuilder::new("check-acton-import-with-mappings-direct")
        .mapping("acton", "./.acton")
        .contract(
            "main",
            r#"
            import "@acton/tlb/maybe";

            fun onInternalMessage(_: InMessage) {
                val maybeValue = Maybe<int>.none();
                maybeValue.unwrapOr(0);
            }
        "#,
        )
        .build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E014")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/acton_import/{}.txt",
            function_name!()
        ));
}

#[test]
#[named]
fn test_check_acton_import_with_mappings_transitive_dependency() {
    let project = ProjectBuilder::new("check-acton-import-with-mappings-transitive")
        .mapping("acton", "./.acton")
        .contract(
            "main",
            r#"
            import "./helper.tolk";

            fun onInternalMessage(_: InMessage) {
                useMaybe();
            }
        "#,
        )
        .file(
            "contracts/helper",
            r#"
            import "@acton/tlb/maybe";

            fun useMaybe() {
                val maybeValue = Maybe<int>.some(1);
                maybeValue.unwrapOr(0);
            }
        "#,
        )
        .build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E014")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/acton_import/{}.txt",
            function_name!()
        ));
}

#[test]
fn test_check_acton_import_rule_is_disabled_for_test_files() {
    let project = ProjectBuilder::new("check-acton-import-test-file")
        .contract("main", "fun onInternalMessage(_: InMessage) {}")
        .test_file(
            "main",
            r#"
                import "../.acton/tlb/maybe.tolk";

                fun test_noop() {
                    val maybeValue = Maybe<int>.none();
                    maybeValue.unwrapOr(0);
                }
            "#,
        )
        .build();

    project.acton().init().run().success();

    let output = project
        .acton()
        .check()
        .arg("tests/main.test.tolk")
        .run()
        .success();

    assert!(
        !output.get_normalized_stderr().contains("E014"),
        "E014 should not be emitted for explicit .test.tolk checks:\n{}",
        output.get_normalized_stderr()
    );
}

fn run_check(group: &str, main_content: &str, files: &[(&str, &str)], name: &str) {
    let mut builder = ProjectBuilder::new(&format!("check-{name}")).contract("main", main_content);
    for (path, content) in files {
        builder = builder.file(path, content);
    }

    let project = builder.build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E014")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(&format!("integration/snapshots/check/{group}/{name}.txt"));
}
