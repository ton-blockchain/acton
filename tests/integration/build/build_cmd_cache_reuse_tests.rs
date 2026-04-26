use crate::support::TestOutputExt;
use crate::support::compilation::extract_compiled_contracts;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn rewrite_contract(path: &Path, marker: &str) {
    let updated = format!(
        r"
fun onInternalMessage(_: InMessage) {{
    // {marker}
}}
fun onBouncedMessage(_: InMessageBounced) {{}}
"
    );

    fs::write(path, updated)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", path.display()));
}

fn format_compilation_order(compiled: &[String]) -> String {
    if compiled.is_empty() {
        return "<none>\n".to_string();
    }

    let mut content = compiled.join("\n");
    content.push('\n');
    content
}

fn assert_compilation_matches_snapshot(compiled: &[String], snapshot_path: &str) {
    let actual = format_compilation_order(compiled);
    let expected = fs::read_to_string(snapshot_path)
        .unwrap_or_else(|err| panic!("failed to read snapshot '{snapshot_path}': {err}"));

    assert_eq!(
        actual, expected,
        "Compilation order mismatch for snapshot '{snapshot_path}'"
    );
}

#[test]
fn build_second_run_without_changes_is_full_cache_hit() {
    let project = ProjectBuilder::new("build-cache-reuse-second-run")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let first = project.acton().build().run().success();
    let first_compiled = extract_compiled_contracts(&first.get_normalized_stdout());
    assert_compilation_matches_snapshot(
        &first_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/initial_single_build.compilation-order.txt",
    );

    let second = project.acton().build().run().success();
    let second_compiled = extract_compiled_contracts(&second.get_normalized_stdout());
    assert_compilation_matches_snapshot(
        &second_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/second_single_build_cache_hit.compilation-order.txt",
    );
}

#[test]
fn build_filtered_target_reuses_cache_when_unrelated_contract_changes() {
    let project = ProjectBuilder::new("build-cache-reuse-filtered-target")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("target", SIMPLE_CONTRACT, vec!["base"])
        .contract("unrelated", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();
    rewrite_contract(
        &project.path().join("contracts/unrelated.tolk"),
        "touch unrelated contract only",
    );

    let filtered = project
        .acton()
        .build()
        .contract("target")
        .run()
        .success()
        .get_normalized_stdout();
    let filtered_compiled = extract_compiled_contracts(&filtered);

    assert!(
        !filtered_compiled
            .iter()
            .any(|contract| contract == "unrelated"),
        "unrelated contract should not be built for filtered target build"
    );

    assert_compilation_matches_snapshot(
        &filtered_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/filtered_target_after_unrelated_change.compilation-order.txt",
    );
}

#[test]
fn build_base_change_invalidates_only_transitive_dependents() {
    let project = ProjectBuilder::new("build-cache-reuse-base-change")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("mid", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("target", SIMPLE_CONTRACT, vec!["mid"])
        .contract("unrelated", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();
    rewrite_contract(
        &project.path().join("contracts/base.tolk"),
        "change base contract source",
    );

    let rebuilt = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();
    let rebuilt_compiled = extract_compiled_contracts(&rebuilt);

    assert!(
        !rebuilt_compiled
            .iter()
            .any(|contract| contract == "unrelated"),
        "unrelated contract cache should remain valid"
    );
    assert_compilation_matches_snapshot(
        &rebuilt_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/base_change_transitive_invalidation.compilation-order.txt",
    );
}

#[test]
fn build_leaf_change_recompiles_only_leaf_contract() {
    let project = ProjectBuilder::new("build-cache-reuse-leaf-change")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("mid", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("target", SIMPLE_CONTRACT, vec!["mid"])
        .build();

    project.acton().build().run().success();
    rewrite_contract(
        &project.path().join("contracts/target.tolk"),
        "change only leaf contract source",
    );

    let rebuilt = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();
    let rebuilt_compiled = extract_compiled_contracts(&rebuilt);

    assert_compilation_matches_snapshot(
        &rebuilt_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/leaf_change_only.compilation-order.txt",
    );
}

#[test]
fn build_partial_branch_change_invalidates_only_changed_branch_and_dependents() {
    let project = ProjectBuilder::new("build-cache-reuse-partial-branch-change")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("left", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("right", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("aggregate", SIMPLE_CONTRACT, vec!["left", "right"])
        .contract("unrelated", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();
    rewrite_contract(
        &project.path().join("contracts/left.tolk"),
        "change only left branch source",
    );

    let rebuilt = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();
    let rebuilt_compiled = extract_compiled_contracts(&rebuilt);

    for cached_contract in ["base", "right", "unrelated"] {
        assert!(
            !rebuilt_compiled
                .iter()
                .any(|contract| contract == cached_contract),
            "{cached_contract} should remain cached after left-branch-only change"
        );
    }
    assert_compilation_matches_snapshot(
        &rebuilt_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/partial_branch_invalidation_scope.compilation-order.txt",
    );
}

#[test]
fn build_mixed_dependency_and_dependent_edits_recompile_union_of_affected_contracts() {
    let project = ProjectBuilder::new("build-cache-reuse-mixed-dependency-edits")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("left", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("right", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("aggregate", SIMPLE_CONTRACT, vec!["left", "right"])
        .contract("unrelated", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();
    rewrite_contract(
        &project.path().join("contracts/right.tolk"),
        "change right dependency source",
    );
    rewrite_contract(
        &project.path().join("contracts/aggregate.tolk"),
        "change dependent source in same pass",
    );

    let rebuilt = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();
    let rebuilt_compiled = extract_compiled_contracts(&rebuilt);

    for cached_contract in ["base", "left", "unrelated"] {
        assert!(
            !rebuilt_compiled
                .iter()
                .any(|contract| contract == cached_contract),
            "{cached_contract} should remain cached after mixed dependency edits"
        );
    }

    assert_eq!(
        rebuilt_compiled
            .iter()
            .filter(|contract| contract.as_str() == "aggregate")
            .count(),
        1,
        "aggregate should be compiled exactly once"
    );
    assert_compilation_matches_snapshot(
        &rebuilt_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/mixed_dependency_edits_invalidation_scope.compilation-order.txt",
    );
}

#[test]
fn build_combined_base_and_leaf_edits_settle_to_stable_cache_hits_on_repeated_runs() {
    let project = ProjectBuilder::new("build-cache-reuse-combined-edits-repeated-cache-hit")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("left", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("right", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("aggregate", SIMPLE_CONTRACT, vec!["left", "right"])
        .contract("unrelated", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();
    rewrite_contract(
        &project.path().join("contracts/base.tolk"),
        "change shared base contract source",
    );
    rewrite_contract(
        &project.path().join("contracts/aggregate.tolk"),
        "change aggregate source in same edit batch",
    );

    let rebuilt = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();
    let rebuilt_compiled = extract_compiled_contracts(&rebuilt);

    assert!(
        !rebuilt_compiled
            .iter()
            .any(|contract| contract == "unrelated"),
        "unrelated contract should remain cached after combined base/leaf edits"
    );
    assert_eq!(
        rebuilt_compiled
            .iter()
            .filter(|contract| contract.as_str() == "aggregate")
            .count(),
        1,
        "aggregate should be compiled exactly once after combined base/leaf edits"
    );
    assert_compilation_matches_snapshot(
        &rebuilt_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/combined_base_and_leaf_edits_invalidation_scope.compilation-order.txt",
    );

    let mut repeated_cache_hit_runs = Vec::new();
    for _ in 0..3 {
        let output = project
            .acton()
            .build()
            .run()
            .success()
            .get_normalized_stdout();
        repeated_cache_hit_runs.push(extract_compiled_contracts(&output));
    }

    for (run_idx, compiled) in repeated_cache_hit_runs.iter().enumerate() {
        assert_compilation_matches_snapshot(
            compiled,
            "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/combined_edits_repeated_cache_hit.compilation-order.txt",
        );
        if run_idx > 0 {
            assert_eq!(
                compiled,
                &repeated_cache_hit_runs[0],
                "cache-hit stability changed across repeated runs (run {})",
                run_idx + 1
            );
        }
    }
}

#[test]
fn build_import_change_recompiles_only_importing_contracts() {
    let project = ProjectBuilder::new("build-cache-reuse-import-change")
        .file(
            "shared/utils",
            r"
fun helperValue(): int {
    return 1;
}
",
        )
        .contract(
            "uses_utils",
            r#"
import "../shared/utils"

fun onInternalMessage(_: InMessage) {
    val _unused = helperValue();
}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .contract(
            "independent",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project.acton().build().run().success();
    fs::write(
        project.path().join("shared/utils.tolk"),
        r"
fun helperValue(): int {
    return 2;
}
",
    )
    .expect("failed to update shared/utils.tolk");

    let rebuilt = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();
    let rebuilt_compiled = extract_compiled_contracts(&rebuilt);

    assert!(
        !rebuilt_compiled
            .iter()
            .any(|contract| contract == "independent"),
        "independent contract should remain cached after shared import change"
    );
    assert_compilation_matches_snapshot(
        &rebuilt_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/import_change_invalidation_scope.compilation-order.txt",
    );
}

#[test]
fn build_output_fift_recompiles_when_plain_cache_entry_lacks_fift() {
    let project = ProjectBuilder::new("build-cache-reuse-fift-after-plain")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    let with_fift = project
        .acton()
        .build()
        .with_output_fift("custom-fift")
        .run()
        .success()
        .get_normalized_stdout();
    let with_fift_compiled = extract_compiled_contracts(&with_fift);

    assert_compilation_matches_snapshot(
        &with_fift_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/fift_after_plain_cache_miss.compilation-order.txt",
    );
    assert!(
        project.path().join("custom-fift/simple.fif").exists(),
        "build with --output-fift should write requested Fift artifact after cache miss"
    );

    let second_with_fift = project
        .acton()
        .build()
        .with_output_fift("custom-fift")
        .run()
        .success()
        .get_normalized_stdout();
    let second_with_fift_compiled = extract_compiled_contracts(&second_with_fift);

    assert_compilation_matches_snapshot(
        &second_with_fift_compiled,
        "tests/integration/snapshots/build/build_cmd_cache_reuse_tests/fift_after_plain_second_run_cache_hit.compilation-order.txt",
    );
}
