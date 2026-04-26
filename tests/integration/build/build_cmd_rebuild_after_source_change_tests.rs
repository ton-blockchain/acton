use crate::support::TestOutputExt;
use crate::support::compilation::extract_compiled_contracts;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::Path;

fn read_artifact(project_path: &Path, contract_name: &str) -> String {
    let artifact_path = project_path
        .join("build")
        .join(format!("{contract_name}.json"));
    fs::read_to_string(&artifact_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read artifact '{}': {err}",
            artifact_path.display()
        )
    })
}

fn assert_compilation_matches_snapshot(compiled: &[String], snapshot_path: &str) {
    let mut actual = compiled.join("\n");
    actual.push('\n');

    let expected = fs::read_to_string(snapshot_path)
        .unwrap_or_else(|err| panic!("Failed to read snapshot '{snapshot_path}': {err}"));

    assert_eq!(
        actual, expected,
        "Compilation order snapshot mismatch for '{snapshot_path}'"
    );
}

#[test]
fn source_change_rebuilds_only_changed_contract() {
    let project = ProjectBuilder::new("build-rebuild-source-change-only-changed")
        .contract(
            "changed",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "stable",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project.acton().build().run().success();
    let stable_before = read_artifact(project.path(), "stable");

    let cache_hit_output = project.acton().build().run().success();
    let cache_hit_compiled = extract_compiled_contracts(&cache_hit_output.get_normalized_stdout());
    assert!(
        cache_hit_compiled.is_empty(),
        "Expected cache hit before source change"
    );

    fs::write(
        project.path().join("contracts/changed.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update changed contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(compiled, vec!["changed".to_string()]);
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_rebuilds_only_changed_contract.compilation-order.txt",
    );

    let stable_after = read_artifact(project.path(), "stable");
    assert_eq!(
        stable_before, stable_after,
        "Unchanged contract artifact should remain byte-identical after rebuilding a different contract"
    );
}

#[test]
fn source_change_in_dependent_keeps_base_and_unaffected_stable() {
    let project = ProjectBuilder::new("build-rebuild-source-change-dependent-only")
        .contract(
            "base",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "dependent",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .contract(
            "unaffected",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project.acton().build().run().success();
    let base_before = read_artifact(project.path(), "base");
    let unaffected_before = read_artifact(project.path(), "unaffected");

    fs::write(
        project.path().join("contracts/dependent.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update dependent contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(compiled, vec!["dependent".to_string()]);
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_in_dependent_keeps_base_and_unaffected_stable.compilation-order.txt",
    );

    let base_after = read_artifact(project.path(), "base");
    let unaffected_after = read_artifact(project.path(), "unaffected");
    assert_eq!(
        base_before, base_after,
        "Base contract artifact should stay unchanged when only dependent source changes"
    );
    assert_eq!(
        unaffected_before, unaffected_after,
        "Unrelated contract artifact should stay unchanged when only dependent source changes"
    );
}

#[test]
fn source_change_in_base_rebuilds_base_and_dependent_only() {
    let project = ProjectBuilder::new("build-rebuild-source-change-base-and-dependent")
        .contract(
            "library",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "app",
            r#"
import "../gen/library.code.tolk"

fun onInternalMessage(in: InMessage) {
    val code = libraryCompiledCode();
    assert (in.body.isEmpty()) throw 200;
}
fun onBouncedMessage(_: InMessageBounced) {}
"#,
            vec!["library"],
        )
        .contract(
            "unaffected",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project.acton().build().run().success();
    let unaffected_before = read_artifact(project.path(), "unaffected");

    fs::write(
        project.path().join("contracts/library.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update base contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(compiled, vec!["library".to_string(), "app".to_string()]);
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_in_base_rebuilds_base_and_dependent_only.compilation-order.txt",
    );

    let unaffected_after = read_artifact(project.path(), "unaffected");
    assert_eq!(
        unaffected_before, unaffected_after,
        "Unrelated contract artifact should stay unchanged when base/dependency source changes"
    );
}

#[test]
fn source_change_in_chain_base_rebuilds_multi_hop_dependents_only() {
    let project = ProjectBuilder::new("build-rebuild-source-change-chain-base")
        .contract(
            "base",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "mid",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .contract_with_deps(
            "top",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mid"],
        )
        .contract(
            "side_base",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "side_leaf",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["side_base"],
        )
        .build();

    project.acton().build().run().success();
    let side_base_before = read_artifact(project.path(), "side_base");
    let side_leaf_before = read_artifact(project.path(), "side_leaf");

    fs::write(
        project.path().join("contracts/base.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update base contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(
        compiled,
        vec!["base".to_string(), "mid".to_string(), "top".to_string()]
    );
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_in_chain_base_rebuilds_multi_hop_dependents_only.compilation-order.txt",
    );

    let side_base_after = read_artifact(project.path(), "side_base");
    let side_leaf_after = read_artifact(project.path(), "side_leaf");
    assert_eq!(
        side_base_before, side_base_after,
        "Contracts outside the changed dependency chain should remain unchanged"
    );
    assert_eq!(
        side_leaf_before, side_leaf_after,
        "Transitive dependents of unrelated contracts should remain unchanged"
    );
}

#[test]
fn source_change_in_chain_middle_rebuilds_downstream_only() {
    let project = ProjectBuilder::new("build-rebuild-source-change-chain-middle")
        .contract(
            "base",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "mid",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .contract_with_deps(
            "top",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mid"],
        )
        .contract_with_deps(
            "sibling",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .contract(
            "detached",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    project.acton().build().run().success();
    let base_before = read_artifact(project.path(), "base");
    let sibling_before = read_artifact(project.path(), "sibling");
    let detached_before = read_artifact(project.path(), "detached");

    fs::write(
        project.path().join("contracts/mid.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update middle contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(compiled, vec!["mid".to_string(), "top".to_string()]);
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_in_chain_middle_rebuilds_downstream_only.compilation-order.txt",
    );

    let base_after = read_artifact(project.path(), "base");
    let sibling_after = read_artifact(project.path(), "sibling");
    let detached_after = read_artifact(project.path(), "detached");
    assert_eq!(
        base_before, base_after,
        "Upstream contract should remain unchanged when modifying a middle dependency"
    );
    assert_eq!(
        sibling_before, sibling_after,
        "Sibling dependent should remain unchanged when a different branch changes"
    );
    assert_eq!(
        detached_before, detached_after,
        "Disconnected contract should remain unchanged when another chain changes"
    );
}

#[test]
fn source_change_in_branch_leaf_rebuilds_leaf_only_and_keeps_sibling_branches_stable() {
    let project = ProjectBuilder::new("build-rebuild-source-change-branch-leaf")
        .contract(
            "root",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "left_mid",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["root"],
        )
        .contract_with_deps(
            "left_leaf",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["left_mid"],
        )
        .contract_with_deps(
            "right_mid",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["root"],
        )
        .contract_with_deps(
            "right_leaf",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["right_mid"],
        )
        .build();

    project.acton().build().run().success();
    let root_before = read_artifact(project.path(), "root");
    let left_mid_before = read_artifact(project.path(), "left_mid");
    let right_mid_before = read_artifact(project.path(), "right_mid");
    let right_leaf_before = read_artifact(project.path(), "right_leaf");

    fs::write(
        project.path().join("contracts/left_leaf.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update branch leaf contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(compiled, vec!["left_leaf".to_string()]);
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_in_branch_leaf_rebuilds_leaf_only_and_keeps_sibling_branches_stable.compilation-order.txt",
    );

    let root_after = read_artifact(project.path(), "root");
    let left_mid_after = read_artifact(project.path(), "left_mid");
    let right_mid_after = read_artifact(project.path(), "right_mid");
    let right_leaf_after = read_artifact(project.path(), "right_leaf");
    assert_eq!(
        root_before, root_after,
        "Shared root should remain unchanged when editing one branch leaf"
    );
    assert_eq!(
        left_mid_before, left_mid_after,
        "Upstream contract should remain unchanged when editing a downstream branch leaf"
    );
    assert_eq!(
        right_mid_before, right_mid_after,
        "Sibling branch root should remain unchanged when editing a different branch leaf"
    );
    assert_eq!(
        right_leaf_before, right_leaf_after,
        "Sibling branch dependent should remain unchanged when editing a different branch leaf"
    );
}

#[test]
fn source_change_in_branch_middle_rebuilds_branch_descendants_and_keeps_sibling_branch_stable() {
    let project = ProjectBuilder::new("build-rebuild-source-change-branch-middle")
        .contract(
            "root",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "left_mid",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["root"],
        )
        .contract_with_deps(
            "left_leaf",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["left_mid"],
        )
        .contract_with_deps(
            "right_mid",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["root"],
        )
        .contract_with_deps(
            "right_leaf",
            r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 100;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["right_mid"],
        )
        .build();

    project.acton().build().run().success();
    let root_before = read_artifact(project.path(), "root");
    let right_mid_before = read_artifact(project.path(), "right_mid");
    let right_leaf_before = read_artifact(project.path(), "right_leaf");

    fs::write(
        project.path().join("contracts/left_mid.tolk"),
        r"fun onInternalMessage(in: InMessage) {
    assert (in.body.isEmpty()) throw 101;
}
fun onBouncedMessage(_: InMessageBounced) {}
",
    )
    .expect("Failed to update branch middle contract source");

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(
        compiled,
        vec!["left_mid".to_string(), "left_leaf".to_string()]
    );
    assert_compilation_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_rebuild_after_source_change_tests/source_change_in_branch_middle_rebuilds_branch_descendants_and_keeps_sibling_branch_stable.compilation-order.txt",
    );

    let root_after = read_artifact(project.path(), "root");
    let right_mid_after = read_artifact(project.path(), "right_mid");
    let right_leaf_after = read_artifact(project.path(), "right_leaf");
    assert_eq!(
        root_before, root_after,
        "Shared root should remain unchanged when editing one branch middle contract"
    );
    assert_eq!(
        right_mid_before, right_mid_after,
        "Sibling branch root should remain unchanged when a different branch middle contract changes"
    );
    assert_eq!(
        right_leaf_before, right_leaf_after,
        "Sibling branch dependent should remain unchanged when a different branch middle contract changes"
    );
}
