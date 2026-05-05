use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

fn read_graph_dot(project_root: &Path, graph_path: &str) -> String {
    let full_path = project_root.join(graph_path);
    fs::read_to_string(&full_path)
        .unwrap_or_else(|err| panic!("failed to read graph '{}': {err}", full_path.display()))
}

fn trim_quoted_identifier(value: &str) -> String {
    let value = value.trim();
    let value = value.strip_prefix('"').unwrap_or(value);
    let value = value.strip_suffix('"').unwrap_or(value);
    value.to_string()
}

fn extract_graph_shape(dot: &str) -> (BTreeSet<String>, BTreeSet<(String, String)>) {
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();

    for line in dot.lines() {
        let line = line.trim();
        if let Some((from, rest)) = line.split_once(" -> ")
            && let Some((to, _)) = rest.split_once(" [")
        {
            edges.insert((trim_quoted_identifier(from), trim_quoted_identifier(to)));
            continue;
        }
        if line.starts_with('"')
            && line.contains(" [label=")
            && !line.contains("->")
            && let Some((name, _)) = line.split_once(" [")
        {
            nodes.insert(trim_quoted_identifier(name));
        }
    }

    (nodes, edges)
}

fn extract_graph_edge_labels(dot: &str) -> BTreeMap<(String, String), String> {
    let mut labels = BTreeMap::new();
    for line in dot.lines() {
        let line = line.trim();
        let Some((from, rest)) = line.split_once(" -> ") else {
            continue;
        };
        let Some((to, attrs)) = rest.split_once(" [") else {
            continue;
        };
        let Some((_, after_label)) = attrs.split_once("label=\"") else {
            continue;
        };
        let Some((raw_label, _)) = after_label.split_once('"') else {
            continue;
        };
        labels.insert(
            (trim_quoted_identifier(from), trim_quoted_identifier(to)),
            raw_label.trim().to_string(),
        );
    }

    labels
}

fn assert_graph_shape(dot: &str, expected_nodes: &[&str], expected_edges: &[(&str, &str)]) {
    let expected_nodes: BTreeSet<String> = expected_nodes
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let expected_edges: BTreeSet<(String, String)> = expected_edges
        .iter()
        .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
        .collect();

    let (actual_nodes, actual_edges) = extract_graph_shape(dot);

    assert_eq!(
        actual_nodes, expected_nodes,
        "graph node set mismatch, actual nodes: {actual_nodes:?}"
    );
    assert_eq!(
        actual_edges, expected_edges,
        "graph edge set mismatch, actual edges: {actual_edges:?}"
    );
}

fn assert_graph_edge_labels(dot: &str, expected_labels: &[(&str, &str, &str)]) {
    let expected: BTreeMap<(String, String), String> = expected_labels
        .iter()
        .map(|(from, to, label)| {
            (
                ((*from).to_string(), (*to).to_string()),
                (*label).to_string(),
            )
        })
        .collect();

    let actual = extract_graph_edge_labels(dot);
    assert_eq!(
        actual, expected,
        "graph edge labels mismatch, actual edge labels: {actual:?}"
    );
}

#[test]
fn test_build_graph_default_path_outputs_expected_dot() {
    let project = ProjectBuilder::new("build-cmd-graph-default")
        .contract(
            "base",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "child",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .build();

    project
        .acton()
        .build()
        .with_graph(None)
        .run()
        .success()
        .assert_contains("dependency graph: deps.dot")
        .assert_file_exists("deps.dot");

    let dot = read_graph_dot(project.path(), "deps.dot");
    assert!(!dot.is_empty(), "deps.dot should not be empty");
}

#[test]
fn test_build_graph_custom_path_outputs_expected_dot_only() {
    let project = ProjectBuilder::new("build-cmd-graph-custom")
        .contract(
            "parent",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "child",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["parent"],
        )
        .build();

    project
        .acton()
        .build()
        .with_graph(Some("custom_graph.dot"))
        .run()
        .success()
        .assert_contains("dependency graph: custom_graph.dot")
        .assert_file_exists("custom_graph.dot");

    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should not be created when custom graph path is provided"
    );
}

#[test]
fn test_build_graph_creates_missing_parent_directories() {
    let project = ProjectBuilder::new("build-cmd-graph-missing-parent")
        .contract(
            "parent",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "child",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["parent"],
        )
        .build();

    project
        .acton()
        .build()
        .with_graph(Some("graphs/deps.dot"))
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/build_graph_creates_missing_parent_directories.stdout.txt",
        )
        .assert_file_exists("graphs/deps.dot");
}

#[test]
fn test_build_graph_output_is_deterministic_between_runs() {
    let project = ProjectBuilder::new("build-cmd-graph-deterministic")
        .contract(
            "base",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "child",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .build();

    project
        .acton()
        .build()
        .with_graph(None)
        .run()
        .success()
        .assert_file_exists("deps.dot");

    let first_dot = fs::read_to_string(project.path().join("deps.dot"))
        .expect("failed to read deps.dot after first build");
    assert!(
        !first_dot.is_empty(),
        "deps.dot should not be empty after first build"
    );

    project
        .acton()
        .build()
        .with_graph(None)
        .run()
        .success()
        .assert_file_exists("deps.dot");

    let second_dot = fs::read_to_string(project.path().join("deps.dot"))
        .expect("failed to read deps.dot after second build");
    assert!(
        !second_dot.is_empty(),
        "deps.dot should not be empty after second build"
    );

    assert_eq!(
        first_dot, second_dot,
        "deps.dot should be byte-for-byte deterministic across repeated builds"
    );
}

#[test]
fn test_build_graph_complex_dependency_shape_is_stable() {
    let project = ProjectBuilder::new("build-cmd-graph-complex-shape")
        .contract(
            "core",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "util",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "api",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core"],
        )
        .contract_with_deps(
            "worker",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["core", "util"],
        )
        .contract_with_deps(
            "app",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["api", "worker"],
        )
        .contract(
            "orphan",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();
    fs::create_dir_all(project.path().join("graphs"))
        .expect("failed to create graphs directory for graph output");

    let output = project
        .acton()
        .build()
        .with_graph(Some("graphs/complex.dot"))
        .run()
        .success();

    output
        .assert_contains("dependency graph: graphs/complex.dot")
        .assert_file_exists("graphs/complex.dot");

    let first_dot = read_graph_dot(project.path(), "graphs/complex.dot");
    assert_graph_shape(
        &first_dot,
        &["api", "app", "core", "orphan", "util", "worker"],
        &[
            ("api", "core"),
            ("app", "api"),
            ("app", "worker"),
            ("worker", "core"),
            ("worker", "util"),
        ],
    );
    assert_eq!(
        first_dot.matches("embed code").count(),
        5,
        "expected one 'embed code' edge label per dependency edge"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should not be created when custom graph path is provided"
    );

    let rerun = project
        .acton()
        .build()
        .with_graph(Some("graphs/complex.dot"))
        .run()
        .success();
    rerun.assert_file_exists("graphs/complex.dot");

    let second_dot = read_graph_dot(project.path(), "graphs/complex.dot");
    assert_eq!(
        first_dot, second_dot,
        "complex graph DOT should be byte-for-byte deterministic across reruns"
    );
}

#[test]
fn test_build_graph_path_variants_produce_identical_content() {
    let project = ProjectBuilder::new("build-cmd-graph-path-variants")
        .contract(
            "base",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_deps(
            "left",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .contract_with_deps(
            "right",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["base"],
        )
        .contract_with_deps(
            "root",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec!["left", "right"],
        )
        .build();
    fs::create_dir_all(project.path().join("graphs").join("absolute"))
        .expect("failed to create nested graph output directory");
    fs::create_dir_all(project.path().join("graphs").join("nested"))
        .expect("failed to create graph output directory used for parent segment path");

    let default_output = project.acton().build().with_graph(None).run().success();
    default_output
        .assert_contains("dependency graph: deps.dot")
        .assert_file_exists("deps.dot");
    let baseline_dot = read_graph_dot(project.path(), "deps.dot");
    assert_graph_shape(
        &baseline_dot,
        &["base", "left", "right", "root"],
        &[
            ("left", "base"),
            ("right", "base"),
            ("root", "left"),
            ("root", "right"),
        ],
    );
    let absolute_path = project
        .path()
        .join("graphs")
        .join("absolute")
        .join("variant-absolute.dot")
        .to_string_lossy()
        .into_owned();
    let variant_paths = vec![
        "graphs/variant-relative.dot".to_string(),
        "./graphs/nested/../variant-normalized.dot".to_string(),
        absolute_path,
    ];

    for path in &variant_paths {
        let output = project
            .acton()
            .build()
            .with_graph(Some(path.as_str()))
            .run()
            .success();

        output
            .assert_contains("dependency graph:")
            .assert_file_exists(path);
        let variant_dot = read_graph_dot(project.path(), path);

        assert_eq!(
            baseline_dot, variant_dot,
            "graph content changed for --graph path variant '{path}'"
        );
    }

    assert!(
        project
            .path()
            .join("graphs/variant-normalized.dot")
            .exists(),
        "normalized parent-segment path should resolve to graphs/variant-normalized.dot"
    );
}

#[test]
fn test_build_graph_complex_mixed_dependency_kinds_match_expected_artifact_content() {
    let project = ProjectBuilder::new("build-cmd-graph-mixed-labels")
        .contract(
            "core",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "lib",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "util",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_detailed_deps(
            "api",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![
                ("core", Some("embed_code"), None, None),
                ("lib", Some("library_ref"), None, None),
            ],
        )
        .contract_with_detailed_deps(
            "worker",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![
                ("core", Some("embed_code"), None, None),
                ("util", Some("embed_code"), None, None),
            ],
        )
        .contract_with_detailed_deps(
            "app",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![
                ("api", Some("library_ref"), None, None),
                ("worker", Some("embed_code"), None, None),
            ],
        )
        .contract(
            "orphan",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .build();
    fs::create_dir_all(project.path().join("graphs"))
        .expect("failed to create graph output directory for mixed dependency kinds");

    let output = project
        .acton()
        .build()
        .with_graph(Some("graphs/mixed-kinds.dot"))
        .run()
        .success();

    output
        .assert_contains("dependency graph: graphs/mixed-kinds.dot")
        .assert_file_exists("graphs/mixed-kinds.dot");

    let dot = read_graph_dot(project.path(), "graphs/mixed-kinds.dot");
    assert_graph_shape(
        &dot,
        &["api", "app", "core", "lib", "orphan", "util", "worker"],
        &[
            ("api", "core"),
            ("api", "lib"),
            ("app", "api"),
            ("app", "worker"),
            ("worker", "core"),
            ("worker", "util"),
        ],
    );
    assert_graph_edge_labels(
        &dot,
        &[
            ("api", "core", "embed code"),
            ("api", "lib", "library ref"),
            ("app", "api", "library ref"),
            ("app", "worker", "embed code"),
            ("worker", "core", "embed code"),
            ("worker", "util", "embed code"),
        ],
    );
    assert_eq!(
        dot.matches("embed code").count(),
        4,
        "expected exactly four 'embed code' edge labels in mixed graph output"
    );
    assert_eq!(
        dot.matches("library ref").count(),
        2,
        "expected exactly two 'library ref' edge labels in mixed graph output"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should not be created when custom graph path is provided"
    );
}

#[test]
fn test_build_graph_contract_filter_emits_reachable_subgraph_with_expected_labels() {
    let project = ProjectBuilder::new("build-cmd-graph-filtered-subgraph")
        .contract(
            "base",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract(
            "shared",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
        )
        .contract_with_detailed_deps(
            "left",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![("base", Some("embed_code"), None, None)],
        )
        .contract_with_detailed_deps(
            "right",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![
                ("base", Some("library_ref"), None, None),
                ("shared", Some("embed_code"), None, None),
            ],
        )
        .contract_with_detailed_deps(
            "target",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![
                ("left", Some("embed_code"), None, None),
                ("right", Some("library_ref"), None, None),
            ],
        )
        .contract_with_detailed_deps(
            "outside",
            r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
",
            vec![("shared", Some("embed_code"), None, None)],
        )
        .build();
    fs::create_dir_all(project.path().join("graphs"))
        .expect("failed to create graph output directory for contract-filtered graph");

    let output = project
        .acton()
        .build()
        .contract("target")
        .with_graph(Some("graphs/target-only.dot"))
        .run()
        .success();

    output
        .assert_contains("dependency graph: graphs/target-only.dot")
        .assert_file_exists("graphs/target-only.dot");

    let dot = read_graph_dot(project.path(), "graphs/target-only.dot");
    assert_graph_shape(
        &dot,
        &["base", "left", "right", "shared", "target"],
        &[
            ("left", "base"),
            ("right", "base"),
            ("right", "shared"),
            ("target", "left"),
            ("target", "right"),
        ],
    );
    assert_graph_edge_labels(
        &dot,
        &[
            ("left", "base", "embed code"),
            ("right", "base", "library ref"),
            ("right", "shared", "embed code"),
            ("target", "left", "embed code"),
            ("target", "right", "library ref"),
        ],
    );
    assert!(
        !dot.contains("\"outside\" [label="),
        "filtered graph output should exclude unrelated contract `outside`"
    );
    assert_eq!(
        dot.matches("embed code").count(),
        3,
        "expected exactly three 'embed code' edge labels in filtered graph output"
    );
    assert_eq!(
        dot.matches("library ref").count(),
        2,
        "expected exactly two 'library ref' edge labels in filtered graph output"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should not be created when custom graph path is provided"
    );
}
