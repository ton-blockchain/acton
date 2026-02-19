use crate::support::TestOutputExt;
use crate::support::compilation::extract_compiled_contracts;
use crate::support::project::{Project, ProjectBuilder};
use std::collections::HashMap;
use std::fs;

fn assert_order_matches_snapshot(compiled: &[String], snapshot_path: &str) {
    let mut actual = compiled.join("\n");
    actual.push('\n');

    let expected = fs::read_to_string(snapshot_path)
        .unwrap_or_else(|err| panic!("Failed to read snapshot '{snapshot_path}': {err}"));

    assert_eq!(
        actual, expected,
        "Compilation order snapshot mismatch for '{snapshot_path}'"
    );
}

fn assert_before(compiled: &[String], first: &str, second: &str) {
    let positions: HashMap<&str, usize> = compiled
        .iter()
        .enumerate()
        .map(|(idx, contract)| (contract.as_str(), idx))
        .collect();

    let first_pos = positions
        .get(first)
        .unwrap_or_else(|| panic!("Contract '{first}' was not compiled. Got order: {compiled:?}"));
    let second_pos = positions
        .get(second)
        .unwrap_or_else(|| panic!("Contract '{second}' was not compiled. Got order: {compiled:?}"));

    assert!(
        first_pos < second_pos,
        "Expected '{first}' before '{second}', got order: {compiled:?}"
    );
}

fn build_large_layered_graph_project(name: &str) -> Project {
    ProjectBuilder::new(name)
        .contract_with_deps(
            "gateway_b",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["service_b2", "service_shared", "service_b1"],
        )
        .contract(
            "core_time",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "service_a1",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["lib_common", "lib_alpha"],
        )
        .contract_with_deps(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["gateway_b", "gateway_a", "auditor"],
        )
        .contract_with_deps(
            "lib_store",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_db"],
        )
        .contract(
            "core_db",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "service_b2",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["lib_delta", "lib_common"],
        )
        .contract_with_deps(
            "auditor",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["reporter", "service_shared"],
        )
        .contract(
            "orphan_root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "reporter",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["lib_store", "lib_telemetry"],
        )
        .contract_with_deps(
            "lib_common",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_db", "core_math"],
        )
        .contract_with_deps(
            "service_shared",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["lib_telemetry", "lib_common"],
        )
        .contract(
            "core_math",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "gateway_a",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["service_a2", "service_shared", "service_a1"],
        )
        .contract_with_deps(
            "lib_beta",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_math"],
        )
        .contract(
            "core_crypto",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "service_a2",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["lib_beta", "lib_common"],
        )
        .contract_with_deps(
            "lib_telemetry",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_time"],
        )
        .contract_with_deps(
            "lib_delta",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_crypto"],
        )
        .contract_with_deps(
            "service_b1",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["lib_gamma", "lib_common"],
        )
        .contract_with_deps(
            "orphan_leaf",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["orphan_root"],
        )
        .contract_with_deps(
            "lib_alpha",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_math"],
        )
        .contract_with_deps(
            "lib_gamma",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_crypto"],
        )
        .build()
}

#[test]
fn build_orders_cross_graph_topologically() {
    let project = ProjectBuilder::new("build-order-topological-cross-graph")
        .contract_with_deps(
            "delta",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["beta"],
        )
        .contract(
            "core",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["gamma", "delta"],
        )
        .contract(
            "orphan",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "alpha",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core"],
        )
        .contract_with_deps(
            "gamma",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["alpha", "beta"],
        )
        .contract_with_deps(
            "beta",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core"],
        )
        .build();

    let output = project.acton().build().clear_cache().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_before(&compiled, "core", "alpha");
    assert_before(&compiled, "core", "beta");
    assert_before(&compiled, "alpha", "gamma");
    assert_before(&compiled, "beta", "delta");
    assert_before(&compiled, "beta", "gamma");
    assert_before(&compiled, "delta", "root");
    assert_before(&compiled, "gamma", "root");

    assert_order_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/topological_cross_graph.compilation-order.txt",
    );
}

#[test]
fn build_order_is_deterministic_with_scrambled_declarations() {
    let project = ProjectBuilder::new("build-order-deterministic-scrambled")
        .contract_with_deps(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mid", "b"],
        )
        .contract(
            "b",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "c",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "mid",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["c", "a"],
        )
        .contract(
            "a",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    let first_output = project.acton().build().clear_cache().run().success();
    let first_order = extract_compiled_contracts(&first_output.get_normalized_stdout());

    let second_output = project.acton().build().clear_cache().run().success();
    let second_order = extract_compiled_contracts(&second_output.get_normalized_stdout());

    assert_eq!(
        first_order, second_order,
        "Compilation order changed between equivalent clear-cache runs"
    );

    assert_order_matches_snapshot(
        &first_order,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/deterministic_scrambled_declarations.compilation-order.txt",
    );
}

#[test]
fn build_contract_filter_keeps_target_dependency_chain_order() {
    let project = ProjectBuilder::new("build-order-filtered-target-chain")
        .contract_with_deps(
            "cli",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["api"],
        )
        .contract_with_deps(
            "api",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["util"],
        )
        .contract_with_deps(
            "util",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["shared"],
        )
        .contract(
            "shared",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "unrelated",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "unrelated_child",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["unrelated"],
        )
        .build();

    let output = project
        .acton()
        .build()
        .clear_cache()
        .contract("cli")
        .run()
        .success();

    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert!(!compiled.iter().any(|contract| contract == "unrelated"));
    assert!(
        !compiled
            .iter()
            .any(|contract| contract == "unrelated_child")
    );

    assert_before(&compiled, "shared", "util");
    assert_before(&compiled, "util", "api");
    assert_before(&compiled, "api", "cli");

    assert_order_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/filtered_target_chain.compilation-order.txt",
    );
}

#[test]
fn build_duplicate_dependency_entries_do_not_duplicate_compilation_regression() {
    let project = ProjectBuilder::new("build-order-duplicate-dependency-regression")
        .contract(
            "base",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "consumer",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base", "base"],
        )
        .build();

    let output = project.acton().build().clear_cache().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(
        compiled
            .iter()
            .filter(|name| name.as_str() == "base")
            .count(),
        1,
        "base dependency should be compiled exactly once"
    );
    assert_before(&compiled, "base", "consumer");

    assert_order_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/duplicate_dependency_entries.compilation-order.txt",
    );
}

#[test]
fn build_orders_complex_multi_branch_dag_topologically() {
    let project = ProjectBuilder::new("build-order-complex-multi-branch-dag")
        .contract_with_deps(
            "join_left",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mid_left", "mid_mixed"],
        )
        .contract_with_deps(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["join_right", "join_left"],
        )
        .contract(
            "core_c",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "mid_left",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_a"],
        )
        .contract_with_deps(
            "join_right",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mid_right", "mid_mixed"],
        )
        .contract_with_deps(
            "mid_mixed",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_c", "core_a", "core_b"],
        )
        .contract(
            "core_a",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "observer",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_c"],
        )
        .contract_with_deps(
            "mid_right",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core_b"],
        )
        .contract(
            "core_b",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    let output = project.acton().build().clear_cache().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_before(&compiled, "core_a", "mid_left");
    assert_before(&compiled, "core_a", "mid_mixed");
    assert_before(&compiled, "core_b", "mid_right");
    assert_before(&compiled, "core_b", "mid_mixed");
    assert_before(&compiled, "core_c", "mid_mixed");
    assert_before(&compiled, "core_c", "observer");
    assert_before(&compiled, "mid_left", "join_left");
    assert_before(&compiled, "mid_mixed", "join_left");
    assert_before(&compiled, "mid_right", "join_right");
    assert_before(&compiled, "mid_mixed", "join_right");
    assert_before(&compiled, "join_left", "root");
    assert_before(&compiled, "join_right", "root");

    assert_order_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/complex_multi_branch_dag.compilation-order.txt",
    );
}

#[test]
fn build_orders_large_layered_dependency_graph_topologically() {
    let project = build_large_layered_graph_project("build-order-large-layered-graph");

    let output = project.acton().build().clear_cache().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());

    assert_eq!(
        compiled.len(),
        23,
        "Expected the entire large graph to compile exactly once"
    );

    assert_before(&compiled, "core_crypto", "lib_delta");
    assert_before(&compiled, "core_crypto", "lib_gamma");
    assert_before(&compiled, "core_db", "lib_common");
    assert_before(&compiled, "core_db", "lib_store");
    assert_before(&compiled, "core_math", "lib_alpha");
    assert_before(&compiled, "core_math", "lib_beta");
    assert_before(&compiled, "core_math", "lib_common");
    assert_before(&compiled, "core_time", "lib_telemetry");
    assert_before(&compiled, "lib_common", "service_a1");
    assert_before(&compiled, "lib_common", "service_a2");
    assert_before(&compiled, "lib_common", "service_b1");
    assert_before(&compiled, "lib_common", "service_b2");
    assert_before(&compiled, "lib_common", "service_shared");
    assert_before(&compiled, "lib_store", "reporter");
    assert_before(&compiled, "lib_telemetry", "reporter");
    assert_before(&compiled, "lib_telemetry", "service_shared");
    assert_before(&compiled, "service_a1", "gateway_a");
    assert_before(&compiled, "service_a2", "gateway_a");
    assert_before(&compiled, "service_shared", "gateway_a");
    assert_before(&compiled, "service_b1", "gateway_b");
    assert_before(&compiled, "service_b2", "gateway_b");
    assert_before(&compiled, "service_shared", "gateway_b");
    assert_before(&compiled, "reporter", "auditor");
    assert_before(&compiled, "service_shared", "auditor");
    assert_before(&compiled, "auditor", "root");
    assert_before(&compiled, "gateway_a", "root");
    assert_before(&compiled, "gateway_b", "root");
    assert_before(&compiled, "orphan_root", "orphan_leaf");

    assert_order_matches_snapshot(
        &compiled,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/large_layered_dependency_graph.compilation-order.txt",
    );
}

#[test]
fn build_large_dependency_graph_order_is_stable_across_repeated_builds() {
    let project = build_large_layered_graph_project("build-order-large-layered-graph-repeated");

    let mut runs = Vec::new();
    for _ in 0..4 {
        let output = project.acton().build().clear_cache().run().success();
        runs.push(extract_compiled_contracts(&output.get_normalized_stdout()));
    }

    let baseline = &runs[0];
    for (run_idx, run) in runs.iter().enumerate().skip(1) {
        assert_eq!(
            run,
            baseline,
            "Compilation order changed on repeated clear-cache build run {}",
            run_idx + 1
        );
    }

    assert_order_matches_snapshot(
        baseline,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/large_layered_dependency_graph.compilation-order.txt",
    );
}

#[test]
fn build_order_tie_breaks_are_deterministic_for_newly_ready_siblings() {
    let first_project = ProjectBuilder::new("build-order-tie-break-first")
        .contract_with_deps(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["right", "shared", "left"],
        )
        .contract(
            "z_orphan",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "right",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["shared"],
        )
        .contract(
            "a_orphan",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "left",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["shared"],
        )
        .contract(
            "shared",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    let second_project = ProjectBuilder::new("build-order-tie-break-second")
        .contract(
            "shared",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "left",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["shared"],
        )
        .contract(
            "a_orphan",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["left", "shared", "right"],
        )
        .contract(
            "z_orphan",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "right",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["shared"],
        )
        .build();

    let first_output = first_project.acton().build().clear_cache().run().success();
    let first_order = extract_compiled_contracts(&first_output.get_normalized_stdout());

    let second_output = second_project.acton().build().clear_cache().run().success();
    let second_order = extract_compiled_contracts(&second_output.get_normalized_stdout());

    assert_eq!(
        first_order, second_order,
        "Tie-break ordering changed for equivalent dependency graphs"
    );

    assert_before(&first_order, "shared", "left");
    assert_before(&first_order, "shared", "right");
    assert_before(&first_order, "left", "right");
    assert_before(&first_order, "left", "root");
    assert_before(&first_order, "right", "root");

    assert_order_matches_snapshot(
        &first_order,
        "tests/integration/snapshots/build/build_cmd_dependencies_order_tests/deterministic_newly_ready_siblings.compilation-order.txt",
    );
}
