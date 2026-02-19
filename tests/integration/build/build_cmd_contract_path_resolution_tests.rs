use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;

fn write_contract_manifest(
    project_root: &Path,
    package_name: &str,
    contract_name: &str,
    src: &str,
) {
    let toml_content = format!(
        r#"[package]
name = "{package_name}"
description = ""
version = "0.1.0"

[contracts.{contract_name}]
name = "{contract_name}"
src = "{src}"
depends = []
"#
    );

    fs::write(project_root.join("Acton.toml"), toml_content).expect("Write Acton.toml");
}

fn to_unix_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn build_resolves_relative_contract_src_with_normalized_segments() {
    let project = ProjectBuilder::new("build-contract-src-relative-normalized")
        .raw_file(
            "contracts/nested/relative_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-relative-normalized",
        "relative_target",
        "./contracts/nested/../nested/relative_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling relative_target");
}

#[test]
fn build_resolves_absolute_contract_src() {
    let project = ProjectBuilder::new("build-contract-src-absolute")
        .raw_file(
            "contracts/absolute_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    let absolute_src = to_unix_path(&project.path().join("contracts/absolute_target.tolk"));
    write_contract_manifest(
        project.path(),
        "build-contract-src-absolute",
        "absolute_target",
        &absolute_src,
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling absolute_target");
}

#[test]
fn build_resolves_relative_contract_src_with_parent_traversal_outside_contracts_dir() {
    let project = ProjectBuilder::new("build-contract-src-relative-parent-outside-contracts")
        .raw_file("contracts/nested/marker.txt", "marker")
        .raw_file(
            "shared/traversal_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-relative-parent-outside-contracts",
        "traversal_target",
        "./contracts/nested/../../shared/traversal_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling traversal_target");
}

#[test]
fn build_resolves_relative_contract_src_with_mixed_dot_and_parent_segments() {
    let project = ProjectBuilder::new("build-contract-src-relative-corner-segments")
        .raw_file("contracts/corner/deep/marker.txt", "marker")
        .raw_file(
            "contracts/corner_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-relative-corner-segments",
        "corner_target",
        "./contracts/./corner/deep/.././../corner_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling corner_target");
}

#[test]
fn build_resolves_relative_contract_src_with_nested_relative_roots_when_intermediate_dirs_exist() {
    let project = ProjectBuilder::new("build-contract-src-relative-nested-roots-existing")
        .raw_file("contracts/root/inner/branch/marker.txt", "marker")
        .raw_file(
            "contracts/root/relative_nested_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-relative-nested-roots-existing",
        "relative_nested_target",
        "contracts/root/inner/branch/../../../root/relative_nested_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling relative_nested_target");
}

#[test]
fn build_relative_contract_src_with_parent_segments_through_missing_dir_is_not_normalized_bug() {
    let project = ProjectBuilder::new("build-contract-src-relative-missing-parent")
        .raw_file(
            "contracts/relative_missing_parent_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-relative-missing-parent",
        "relative_missing_parent_target",
        "contracts/missing_layer/../relative_missing_parent_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        // BUG: relative `src` paths with parent segments are not normalized before file lookup.
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_path_resolution_tests/build_relative_contract_src_with_parent_segments_through_missing_dir_is_not_normalized_bug.stderr.txt",
        );
}

#[test]
fn build_relative_contract_src_with_nested_relative_roots_through_missing_intermediate_dirs_is_not_normalized_bug()
 {
    let project = ProjectBuilder::new("build-contract-src-relative-nested-roots-missing")
        .raw_file(
            "contracts/root/relative_nested_missing_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-relative-nested-roots-missing",
        "relative_nested_missing_target",
        "contracts/missing_root/deeper/../../root/relative_nested_missing_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        // BUG: nested relative roots that include missing intermediates before `..`
        // are not normalized before file lookup.
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_path_resolution_tests/build_relative_contract_src_with_nested_relative_roots_through_missing_intermediate_dirs_is_not_normalized_bug.stderr.txt",
        );
}

#[test]
fn build_resolves_absolute_contract_src_with_parent_segments_when_intermediate_dirs_exist() {
    let project = ProjectBuilder::new("build-contract-src-absolute-parent-existing-dir-success")
        .raw_file("contracts/absolute_root/nested/marker.txt", "marker")
        .raw_file(
            "contracts/absolute_root/absolute_existing_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    let absolute_src = to_unix_path(
        &project
            .path()
            .join("contracts/absolute_root/nested/../absolute_existing_target.tolk"),
    );
    write_contract_manifest(
        project.path(),
        "build-contract-src-absolute-parent-existing-dir-success",
        "absolute_existing_target",
        &absolute_src,
    );

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling absolute_existing_target");
}

#[test]
fn build_absolute_contract_src_with_parent_segments_is_not_normalized_bug() {
    let project = ProjectBuilder::new("build-contract-src-absolute-normalized")
        .raw_file(
            "contracts/absolute_normalized_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    let absolute_src = to_unix_path(
        &project
            .path()
            .join("contracts/nested/../absolute_normalized_target.tolk"),
    );
    write_contract_manifest(
        project.path(),
        "build-contract-src-absolute-normalized",
        "absolute_normalized_target",
        &absolute_src,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        // BUG: absolute `src` paths with parent segments are not normalized before file lookup.
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_path_resolution_tests/build_absolute_contract_src_with_parent_segments_is_not_normalized_bug.stderr.txt",
        );
}

#[test]
fn build_absolute_contract_src_with_nested_missing_intermediate_dirs_is_not_canonicalized_bug() {
    let project = ProjectBuilder::new("build-contract-src-absolute-nested-missing-roots")
        .raw_file(
            "contracts/absolute_nested_missing_target.tolk",
            r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .build();

    let absolute_src = to_unix_path(
        &project
            .path()
            .join("contracts/missing_root/deeper/../../absolute_nested_missing_target.tolk"),
    );
    write_contract_manifest(
        project.path(),
        "build-contract-src-absolute-nested-missing-roots",
        "absolute_nested_missing_target",
        &absolute_src,
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        // BUG: absolute `src` paths with nested missing intermediates before `..`
        // are not canonicalized before file lookup.
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_path_resolution_tests/build_absolute_contract_src_with_nested_missing_intermediate_dirs_is_not_canonicalized_bug.stderr.txt",
        );
}

#[test]
fn build_reports_not_found_for_missing_contract_src_with_normalized_segments() {
    let project = ProjectBuilder::new("build-contract-src-missing-normalized").build();

    write_contract_manifest(
        project.path(),
        "build-contract-src-missing-normalized",
        "missing_target",
        "contracts/nested/../missing_target.tolk",
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("Failed to locate contracts/nested/../missing_target.tolk")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_contract_path_resolution_tests/build_reports_not_found_for_missing_contract_src_with_normalized_segments.stderr.txt",
        );
}
