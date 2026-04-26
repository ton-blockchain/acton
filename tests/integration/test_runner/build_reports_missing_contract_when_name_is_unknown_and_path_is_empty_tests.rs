use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn replace_contract_display_name(project_path: &Path, from: &str, to: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let acton_toml = fs::read_to_string(&acton_toml_path).expect("failed to read Acton.toml");
    let updated = acton_toml.replace(
        &format!("display-name = \"{from}\""),
        &format!("display-name = \"{to}\""),
    );
    fs::write(&acton_toml_path, updated).expect("failed to write Acton.toml");
}

#[test]
fn build_reports_missing_contract_when_name_is_unknown_and_path_is_empty() {
    ProjectBuilder::new("ax-stdlib-build-missing-contract-empty-path")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_missing_contract_empty_path",
            r#"
            import "../../lib/build"

            get fun `test ax build missing contract empty path`() {
                val _ = build("missing", "");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Contract missing not found in Acton.toml")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reports_missing_contract_when_name_is_unknown_and_path_is_empty/build_reports_missing_contract_when_name_is_unknown_and_path_is_empty.stdout.txt",
        );
}

#[test]
fn build_reports_missing_contract_when_name_and_path_are_empty() {
    ProjectBuilder::new("ax-stdlib-build-empty-contract-inputs")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_empty_contract_inputs",
            r#"
            import "../../lib/build"

            get fun `test ax build empty contract inputs`() {
                val _ = build("", "");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("not found in Acton.toml")
        .assert_contains("Available contracts:")
        .assert_contains("simple")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reports_missing_contract_when_name_is_unknown_and_path_is_empty/build_reports_missing_contract_when_name_and_path_are_empty.stdout.txt",
        );
}

#[test]
fn build_reports_display_name_when_contract_id_is_required() {
    let project = ProjectBuilder::new("ax-stdlib-build-display-name-hint")
        .contract("simple_id", SIMPLE_CONTRACT)
        .test_file(
            "build_display_name_contract_input",
            r#"
            import "../../lib/build"

            get fun `test ax build display-name hint`() {
                val _ = build("Visible Simple", "");
            }
        "#,
        )
        .build();
    replace_contract_display_name(project.path(), "simple_id", "Visible Simple");

    project
        .acton()
        .test()
        .run()
        .failure()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_reports_missing_contract_when_name_is_unknown_and_path_is_empty/build_reports_display_name_when_contract_id_is_required.stdout.txt",
        );
}
