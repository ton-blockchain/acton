use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

const UNFORMATTED_TOLK: &str = r"
fun onInternalMessage(in:InMessage){
val x=1;
    val y = 2;
}
";

const FORMATTED_TOLK: &str = r"fun onInternalMessage(in: InMessage) {
    val x = 1;
    val y = 2;
}
";

const IMPORTS_UNFORMATTED_TOLK: &str = r#"import "./b"
import "@acton/io"
import "@stdlib/reflection"
import "../z"
import "./a"
import "../a"
import "@contracts/types"
fun main() {}
"#;

#[test]
fn test_fmt_simple() {
    let project = ProjectBuilder::new("fmt-simple")
        .contract("simple", UNFORMATTED_TOLK)
        .build();

    let contract_path = project.path().join("contracts/simple.tolk");

    assert_eq!(
        fs::read_to_string(&contract_path).unwrap(),
        UNFORMATTED_TOLK
    );

    project
        .acton()
        .fmt()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_fmt_simple.stdout.txt");

    assert_eq!(fs::read_to_string(&contract_path).unwrap(), FORMATTED_TOLK);
}

#[test]
fn test_fmt_check_failure() {
    let project = ProjectBuilder::new("fmt-check-fail")
        .contract("simple", UNFORMATTED_TOLK)
        .build();

    project
        .acton()
        .fmt()
        .arg("--check")
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_fmt_check_failure.stdout.txt");
}

#[test]
fn test_fmt_check_success() {
    let project = ProjectBuilder::new("fmt-check-success")
        .contract("simple", FORMATTED_TOLK)
        .build();

    project
        .acton()
        .fmt()
        .arg("--check")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_fmt_check_success.stdout.txt");
}

#[test]
fn test_fmt_specific_file() {
    let project = ProjectBuilder::new("fmt-specific")
        .contract("simple1", UNFORMATTED_TOLK)
        .contract("simple2", UNFORMATTED_TOLK)
        .build();

    project
        .acton()
        .fmt()
        .arg("contracts/simple1.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_fmt_specific_file.stdout.txt");

    assert_eq!(
        fs::read_to_string(project.path().join("contracts/simple1.tolk")).unwrap(),
        FORMATTED_TOLK
    );
    assert_eq!(
        fs::read_to_string(project.path().join("contracts/simple2.tolk")).unwrap(),
        UNFORMATTED_TOLK
    );
}

#[test]
fn test_fmt_ignore_from_config() {
    let project = ProjectBuilder::new("fmt-ignore")
        .contract("simple1", UNFORMATTED_TOLK)
        .contract("simple2", UNFORMATTED_TOLK)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut toml_content = fs::read_to_string(&acton_toml_path).unwrap();
    toml_content.push_str("\n[fmt]\nignore = [\"contracts/simple2.tolk\"]\n");
    fs::write(&acton_toml_path, toml_content).unwrap();

    project
        .acton()
        .fmt()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_fmt_ignore_from_config.stdout.txt");

    assert_eq!(
        fs::read_to_string(project.path().join("contracts/simple1.tolk")).unwrap(),
        FORMATTED_TOLK
    );
    assert_eq!(
        fs::read_to_string(project.path().join("contracts/simple2.tolk")).unwrap(),
        UNFORMATTED_TOLK
    );
}

#[test]
fn test_fmt_syntax_error() {
    let project = ProjectBuilder::new("fmt-syntax-error")
        .contract("broken", "fun broken {")
        .build();

    project
        .acton()
        .fmt()
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/test_fmt_syntax_error.stderr.txt");
}

#[test]
fn test_fmt_syntax_errors_in_two_files() {
    let project = ProjectBuilder::new("fmt-syntax-error-two-files")
        .contract("broken1", "fun broken1 {")
        .contract("broken2", "fun broken2() { val x = 1 + }")
        .build();

    project
        .acton()
        .fmt()
        .arg("contracts/broken1.tolk")
        .arg("contracts/broken2.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_fmt_syntax_errors_in_two_files.stderr.txt",
        );
}

#[test]
fn test_fmt_nonexistent_path() {
    let project = ProjectBuilder::new("fmt-nonexistent-path")
        .contract("simple", UNFORMATTED_TOLK)
        .build();

    project
        .acton()
        .fmt()
        .arg("contracts/missing.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_fmt_nonexistent_path.stderr.txt",
        );
}

#[test]
fn test_fmt_explicit_non_tolk_file_fails() {
    let project = ProjectBuilder::new("fmt-explicit-non-tolk")
        .raw_file("README.md", "# Notes\n")
        .build();

    project
        .acton()
        .fmt()
        .arg("README.md")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_fmt_explicit_non_tolk_file_fails.stderr.txt",
        );
}

#[test]
fn test_fmt_mixed_paths_partial_failure_keeps_existing_file_unchanged() {
    let project = ProjectBuilder::new("fmt-mixed-paths-partial-failure")
        .contract("simple", UNFORMATTED_TOLK)
        .build();

    assert_eq!(
        fs::read_to_string(project.path().join("contracts/simple.tolk")).unwrap(),
        UNFORMATTED_TOLK
    );

    project
        .acton()
        .fmt()
        .arg("contracts/simple.tolk")
        .arg("contracts/missing.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_fmt_mixed_paths_partial_failure.stderr.txt",
        )
        .assert_file_snapshot_matches(
            "contracts/simple.tolk",
            "integration/snapshots/test_fmt_mixed_paths_partial_failure.result.txt",
        );
}

#[test]
fn test_fmt_custom_width() {
    let long_line = "val x = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];\n";
    let code = format!("fun test() {{\n    {long_line}\n}}\n");

    let project = ProjectBuilder::new("fmt-width")
        .contract("wide", &code)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut toml_content = fs::read_to_string(&acton_toml_path).unwrap();
    toml_content.push_str("\n[fmt]\nwidth = 20\n");
    fs::write(&acton_toml_path, toml_content).unwrap();

    let output = project.acton().fmt().run().success();

    output.assert_snapshot_matches("integration/snapshots/test_fmt_custom_width.stdout.txt");

    output.assert_file_snapshot_matches(
        project.path().join("contracts/wide.tolk").to_str().unwrap(),
        "integration/snapshots/test_fmt_custom_width.result.txt",
    );
}

#[test]
fn test_fmt_import_group_separators_from_config() {
    let project = ProjectBuilder::new("fmt-import-groups")
        .contract("imports", IMPORTS_UNFORMATTED_TOLK)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut toml_content = fs::read_to_string(&acton_toml_path).unwrap();
    toml_content.push_str("\n[fmt]\nseparate-import-groups = true\n");
    fs::write(&acton_toml_path, toml_content).unwrap();

    let output = project.acton().fmt().run().success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_fmt_import_group_separators_from_config.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        project
            .path()
            .join("contracts/imports.tolk")
            .to_str()
            .unwrap(),
        "integration/snapshots/test_fmt_import_group_separators_from_config.result.txt",
    );
}

#[test]
fn test_fmt_import_group_separators_disabled_from_config() {
    let project = ProjectBuilder::new("fmt-import-groups-disabled")
        .contract("imports", IMPORTS_UNFORMATTED_TOLK)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut toml_content = fs::read_to_string(&acton_toml_path).unwrap();
    toml_content.push_str("\n[fmt]\nseparate-import-groups = false\n");
    fs::write(&acton_toml_path, toml_content).unwrap();

    let output = project.acton().fmt().run().success();

    output.assert_snapshot_matches(
        "integration/snapshots/test_fmt_import_group_separators_disabled_from_config.stdout.txt",
    );
    output.assert_file_snapshot_matches(
        project
            .path()
            .join("contracts/imports.tolk")
            .to_str()
            .unwrap(),
        "integration/snapshots/test_fmt_import_group_separators_disabled_from_config.result.txt",
    );
}
