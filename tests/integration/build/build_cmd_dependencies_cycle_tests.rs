use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn build_three_contract_cycle_reports_closed_path() {
    let project = ProjectBuilder::new("build-cycle-three-contracts")
        .contract_with_deps(
            "a",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["b"],
        )
        .contract_with_deps(
            "b",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["c"],
        )
        .contract_with_deps(
            "c",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["a"],
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("Circular dependency detected in contracts")
        .assert_stderr_contains("b → c → a → b")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_three_contract_cycle_reports_closed_path.stderr.txt",
        );
}

#[test]
fn build_self_cycle_reports_repeated_contract_path() {
    let project = ProjectBuilder::new("build-cycle-self-reference")
        .contract_with_deps(
            "self_ref",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["self_ref"],
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("Circular dependency detected in contracts")
        .assert_stderr_contains("self_ref → self_ref")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_self_cycle_reports_repeated_contract_path.stderr.txt",
        );
}

#[test]
fn build_contract_filter_reports_cycle_in_target_dependencies() {
    let project = ProjectBuilder::new("build-cycle-filtered-target-subgraph")
        .contract_with_deps(
            "target",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["alpha"],
        )
        .contract_with_deps(
            "alpha",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["beta"],
        )
        .contract_with_deps(
            "beta",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["alpha"],
        )
        .build();

    project
        .acton()
        .build()
        .contract("target")
        .run()
        .failure()
        .assert_stderr_contains("Circular dependency detected in contracts")
        .assert_stderr_contains("beta → alpha → beta")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_contract_filter_reports_cycle_in_target_dependencies.stderr.txt",
        );
}

#[test]
fn build_contract_filter_fails_on_unrelated_cycle_component_bug() {
    let project = ProjectBuilder::new("build-cycle-filtered-target-unrelated-cycle")
        .contract(
            "leaf",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "target",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["leaf"],
        )
        .contract_with_deps(
            "x",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["y"],
        )
        .contract_with_deps(
            "y",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["x"],
        )
        .build();

    project
        .acton()
        .build()
        .contract("target")
        .run()
        .failure()
        // BUG: `build <contract>` fails on a cycle from an unrelated component.
        .assert_stderr_contains("y → x → y")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_contract_filter_fails_on_unrelated_cycle_component_bug.stderr.txt",
        );
}

#[test]
fn build_cycle_diagnostics_excludes_acyclic_prefix_from_reported_loop() {
    let project = ProjectBuilder::new("build-cycle-shape-tail-trimmed")
        .contract_with_deps(
            "a",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["b"],
        )
        .contract_with_deps(
            "b",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["c"],
        )
        .contract_with_deps(
            "c",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["d"],
        )
        .contract_with_deps(
            "d",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["b"],
        )
        .build();

    let output = project.acton().build().run().failure();
    let stderr = output.get_normalized_stderr();

    assert!(
        stderr.contains("Circular dependency detected in contracts"),
        "Expected circular dependency header, got:\n{stderr}"
    );
    assert!(
        stderr.contains("c → d → b → c"),
        "Expected closed cycle path without acyclic prefix, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("a →"),
        "Acyclic prefix contract should not appear in cycle path, got:\n{stderr}"
    );

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_cycle_diagnostics_excludes_acyclic_prefix_from_reported_loop.stderr.txt",
    );
}

#[test]
fn build_cycle_diagnostics_reports_first_cycle_component_in_key_order() {
    let project = ProjectBuilder::new("build-cycle-order-multi-component")
        .contract_with_deps(
            "x",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["y"],
        )
        .contract_with_deps(
            "y",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["x"],
        )
        .contract_with_deps(
            "alpha",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["beta"],
        )
        .contract_with_deps(
            "beta",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["gamma"],
        )
        .contract_with_deps(
            "gamma",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["alpha"],
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_contains("Circular dependency detected in contracts")
        .assert_stderr_contains("beta → gamma → alpha → beta")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_cycle_diagnostics_reports_first_cycle_component_in_key_order.stderr.txt",
        );
}

#[test]
fn build_contract_filter_reports_first_unrelated_cycle_component_bug() {
    let project = ProjectBuilder::new("build-cycle-filtered-target-multi-unrelated-cycles")
        .contract(
            "leaf",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "target",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["leaf"],
        )
        .contract_with_deps(
            "x",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["y"],
        )
        .contract_with_deps(
            "y",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["x"],
        )
        .contract_with_deps(
            "a",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["b"],
        )
        .contract_with_deps(
            "b",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["a"],
        )
        .build();

    project
        .acton()
        .build()
        .contract("target")
        .run()
        .failure()
        // BUG: `build <contract>` still resolves cycles globally and picks the first unrelated cycle.
        .assert_stderr_contains("b → a → b")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_contract_filter_reports_first_unrelated_cycle_component_bug.stderr.txt",
        );
}

#[test]
fn build_cycle_diagnostics_stress_reports_deterministic_cycle_across_independent_components() {
    let project = ProjectBuilder::new("build-cycle-diagnostics-stress-independent-components")
        .contract_with_deps(
            "zz1",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["zz2"],
        )
        .contract_with_deps(
            "zz2",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["zz3"],
        )
        .contract_with_deps(
            "zz3",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["zz1"],
        )
        .contract_with_deps(
            "mm1",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mm2"],
        )
        .contract_with_deps(
            "mm2",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["mm1"],
        )
        .contract_with_deps(
            "aa1",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["aa2"],
        )
        .contract_with_deps(
            "aa2",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["aa3"],
        )
        .contract_with_deps(
            "aa3",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["aa1"],
        )
        .contract_with_deps(
            "ab_leaf",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["aa1"],
        )
        .contract(
            "root",
            r"fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();

    let output = project.acton().build().run().failure();
    let stderr = output.get_normalized_stderr();

    assert!(
        stderr.contains("Circular dependency detected in contracts"),
        "Expected circular dependency header, got:\n{stderr}"
    );
    assert!(
        stderr.contains("aa2 → aa3 → aa1 → aa2"),
        "Expected deterministic diagnostics cycle from the first key-ordered component, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("mm2 → mm1 → mm2"),
        "Diagnostics should report a deterministic first component cycle, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("zz2 → zz3 → zz1 → zz2"),
        "Diagnostics should report a deterministic first component cycle, got:\n{stderr}"
    );

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/build/build_cmd_dependencies_cycle_tests/build_cycle_diagnostics_stress_reports_deterministic_cycle_across_independent_components.stderr.txt",
    );
}
