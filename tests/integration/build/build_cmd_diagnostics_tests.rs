use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;
const INVALID_PRECOMPILED_BOC: &[u8] =
    include_bytes!("../testdata/build/build_cmd_diagnostics_tests/invalid_precompiled.boc");

fn write_boc_contract_manifest(
    project_root: &Path,
    package_name: &str,
    contract_key: &str,
    contract_name: &str,
    src: &str,
) {
    let toml_content = format!(
        r#"[package]
name = "{package_name}"
description = ""
version = "0.1.0"

[contracts.{contract_key}]
name = "{contract_name}"
src = "{src}"
depends = []
"#
    );

    fs::write(project_root.join("Acton.toml"), toml_content).expect("Write Acton.toml");
}

fn assert_substrings_in_order(haystack: &str, needles: &[&str]) {
    let mut search_from = 0usize;
    for needle in needles {
        let relative_idx = haystack[search_from..]
            .find(needle)
            .unwrap_or_else(|| panic!("Expected '{needle}' in stderr, got:\n{haystack}"));
        search_from += relative_idx + needle.len();
    }
}

#[test]
fn build_aggregates_multiple_contract_compile_errors_with_per_contract_sections() {
    let project = ProjectBuilder::new("build-diagnostics-aggregate-errors")
        .contract(
            "broken_syntax",
            r"fun onInternalMessage(in: InMessage) {
    val broken = ;
}

fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "missing_symbol",
            r"fun onInternalMessage(in: InMessage) {
    val x = missing_identifier;
}

fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project
        .acton()
        .build()
        .clear_cache()
        .run()
        .failure()
        .assert_stderr_contains("In broken_syntax:")
        .assert_stderr_contains("In missing_symbol:")
        .assert_stderr_contains("Build failed with 2 errors")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_aggregates_multiple_contract_compile_errors_with_per_contract_sections.stderr.txt",
        );
}

#[test]
fn build_reports_dependency_failure_for_parent_when_generated_dependency_file_is_missing() {
    let project = ProjectBuilder::new("build-diagnostics-dependency-cascade")
        .contract(
            "child",
            r"fun onInternalMessage(in: InMessage) {
    val childValue = ;
}

fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "parent",
            r#"import "../gen/child_code.tolk"

fun onInternalMessage(in: InMessage) {
    val code = childCompiledCode();
}

fun onBouncedMessage(_: InMessageBounced) {}
"#,
            vec!["child"],
        )
        .build();

    project
        .acton()
        .build()
        .clear_cache()
        .run()
        .failure()
        .assert_stderr_contains("In child:")
        .assert_stderr_contains("In parent:")
        .assert_stderr_contains("Build failed with 2 errors")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_reports_dependency_failure_for_parent_when_generated_dependency_file_is_missing.stderr.txt",
        );
}

#[test]
fn build_reports_decode_error_for_invalid_precompiled_boc_file() {
    let project = ProjectBuilder::new("build-diagnostics-invalid-precompiled-boc")
        .contract_from_boc("broken_precompiled", INVALID_PRECOMPILED_BOC.to_vec())
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("In broken_precompiled:")
        .assert_stderr_contains("Failed to decode BoC file contracts/broken_precompiled.boc")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_reports_decode_error_for_invalid_precompiled_boc_file.stderr.txt",
        );
}

#[test]
fn build_reports_read_error_for_missing_precompiled_boc_file() {
    let project = ProjectBuilder::new("build-diagnostics-missing-precompiled-boc").build();

    write_boc_contract_manifest(
        project.path(),
        "build-diagnostics-missing-precompiled-boc",
        "missing_precompiled",
        "missing_precompiled",
        "contracts/missing_precompiled.boc",
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("In missing_precompiled:")
        .assert_stderr_contains("Failed to read BoC file contracts/missing_precompiled.boc")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_reports_read_error_for_missing_precompiled_boc_file.stderr.txt",
        );
}

#[test]
fn build_combines_boc_decode_and_compile_errors_with_stable_contract_order() {
    let project = ProjectBuilder::new("build-diagnostics-mixed-errors-stable-order")
        .contract(
            "zz_compile_failure",
            r"fun onInternalMessage(in: InMessage) {
    val broken = ;
}

fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_from_boc("aa_decode_failure", INVALID_PRECOMPILED_BOC.to_vec())
        .build();

    project
        .acton()
        .build()
        .clear_cache()
        .run()
        .failure()
        .assert_stderr_contains("In aa_decode_failure:")
        .assert_stderr_contains("In zz_compile_failure:")
        .assert_stderr_contains("Failed to decode BoC file contracts/aa_decode_failure.boc")
        .assert_stderr_contains("Build failed with 2 errors")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_combines_boc_decode_and_compile_errors_with_stable_contract_order.stderr.txt",
        );
}

#[test]
fn build_reports_original_relative_path_context_for_missing_precompiled_boc_file() {
    let project = ProjectBuilder::new("build-diagnostics-missing-precompiled-context").build();

    write_boc_contract_manifest(
        project.path(),
        "build-diagnostics-missing-precompiled-context",
        "missing_precompiled_with_segments",
        "missing_precompiled_with_segments",
        "./contracts/nested/../missing/missing_precompiled_with_segments.boc",
    );

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("In missing_precompiled_with_segments:")
        .assert_stderr_contains(
            "Failed to read BoC file ./contracts/nested/../missing/missing_precompiled_with_segments.boc",
        )
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_reports_original_relative_path_context_for_missing_precompiled_boc_file.stderr.txt",
        );
}

#[test]
fn build_sorts_multi_error_sections_by_contract_key_and_keeps_messages_consistent() {
    let project = ProjectBuilder::new("build-diagnostics-multi-error-ordering-consistency")
        .contract_with_deps(
            "a_parent",
            r#"import "../gen/child_code.tolk"

fun onInternalMessage(in: InMessage) {
    val code = childCompiledCode();
}

fun onBouncedMessage(_: InMessageBounced) {}
"#,
            vec!["child"],
        )
        .contract(
            "child",
            r"fun onInternalMessage(in: InMessage) {
    val childValue = ;
}

fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_from_boc("m_invalid_boc", INVALID_PRECOMPILED_BOC.to_vec())
        .build();

    let output = project.acton().build().clear_cache().run().failure();
    let stderr = output.get_normalized_stderr();
    let expected_sections = ["In a_parent:", "In child:", "In m_invalid_boc:"];

    assert_substrings_in_order(&stderr, &expected_sections);

    let section_count = stderr.matches("Error: In ").count() + stderr.matches("\nIn ").count();
    assert_eq!(
        section_count,
        expected_sections.len(),
        "Expected one diagnostics section per failing contract, got:\n{stderr}"
    );

    assert!(
        stderr.contains("Failed to import: [NOT_FOUND]"),
        "Expected dependency-import diagnostic in parent section, got:\n{stderr}"
    );
    assert!(
        stderr.contains("error: expected <expression>, got `;`"),
        "Expected compiler syntax diagnostic in child section, got:\n{stderr}"
    );
    assert!(
        stderr.contains("Failed to decode BoC file contracts/m_invalid_boc.boc"),
        "Expected BoC decode diagnostic in boc section, got:\n{stderr}"
    );

    output
        .assert_stderr_contains("Build failed with 3 errors")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_diagnostics_tests/build_sorts_multi_error_sections_by_contract_key_and_keeps_messages_consistent.stderr.txt",
        );
}
