use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

fn read_graph_svg(project_root: &Path, graph_path: &str) -> String {
    let full_path = project_root.join(graph_path);
    fs::read_to_string(&full_path)
        .unwrap_or_else(|err| panic!("failed to read graph '{}': {err}", full_path.display()))
}

fn decode_svg_title(raw_title: &str) -> String {
    raw_title
        .replace("&#45;", "-")
        .replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
}

fn extract_graph_shape(svg: &str) -> (BTreeSet<String>, BTreeSet<(String, String)>) {
    let mut nodes = BTreeSet::new();
    let mut edges = BTreeSet::new();

    for line in svg.lines() {
        let line = line.trim();
        let Some(raw_title) = line
            .strip_prefix("<title>")
            .and_then(|rest| rest.strip_suffix("</title>"))
        else {
            continue;
        };

        let title = decode_svg_title(raw_title);
        if title == "Dependencies" {
            continue;
        }

        if let Some((from, to)) = title.split_once("->") {
            edges.insert((from.to_string(), to.to_string()));
        } else {
            nodes.insert(title);
        }
    }

    (nodes, edges)
}

fn extract_graph_edge_labels(svg: &str) -> BTreeMap<(String, String), String> {
    let mut labels = BTreeMap::new();
    let mut current_edge: Option<(String, String)> = None;

    for line in svg.lines() {
        let line = line.trim();

        if let Some(raw_title) = line
            .strip_prefix("<title>")
            .and_then(|rest| rest.strip_suffix("</title>"))
        {
            let title = decode_svg_title(raw_title);
            current_edge = title
                .split_once("->")
                .map(|(from, to)| (from.to_string(), to.to_string()));
            continue;
        }

        let Some(edge) = current_edge.clone() else {
            continue;
        };
        if !(line.starts_with("<text ") && line.ends_with("</text>")) {
            continue;
        }

        let Some((_, text_with_suffix)) = line.split_once('>') else {
            continue;
        };
        let Some(raw_text) = text_with_suffix.strip_suffix("</text>") else {
            continue;
        };

        labels.insert(edge, decode_svg_title(raw_text).trim().to_string());
        current_edge = None;
    }

    labels
}

fn assert_graph_shape(svg: &str, expected_nodes: &[&str], expected_edges: &[(&str, &str)]) {
    let expected_nodes: BTreeSet<String> = expected_nodes
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    let expected_edges: BTreeSet<(String, String)> = expected_edges
        .iter()
        .map(|(from, to)| ((*from).to_string(), (*to).to_string()))
        .collect();

    let (actual_nodes, actual_edges) = extract_graph_shape(svg);

    assert_eq!(
        actual_nodes, expected_nodes,
        "graph node set mismatch, actual nodes: {actual_nodes:?}"
    );
    assert_eq!(
        actual_edges, expected_edges,
        "graph edge set mismatch, actual edges: {actual_edges:?}"
    );
}

fn assert_graph_edge_labels(svg: &str, expected_labels: &[(&str, &str, &str)]) {
    let expected: BTreeMap<(String, String), String> = expected_labels
        .iter()
        .map(|(from, to, label)| {
            (
                ((*from).to_string(), (*to).to_string()),
                (*label).to_string(),
            )
        })
        .collect();

    let actual = extract_graph_edge_labels(svg);
    assert_eq!(
        actual, expected,
        "graph edge labels mismatch, actual edge labels: {actual:?}"
    );
}

#[test]
fn test_build_graph_default_path_outputs_expected_svg() {
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
        .assert_contains("dependency graph: deps.svg")
        .assert_file_exists("deps.svg")
        .assert_file_snapshot_matches(
            "deps.svg",
            "integration/snapshots/build/build_cmd_output_graph_tests/test_build_graph_default_path_outputs_expected_svg.svg.gen",
        );

    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after SVG generation"
    );
}

#[test]
fn test_build_graph_custom_path_outputs_expected_svg_only() {
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
        .with_graph(Some("custom_graph.svg"))
        .run()
        .success()
        .assert_contains("dependency graph: custom_graph.svg")
        .assert_file_exists("custom_graph.svg")
        .assert_file_snapshot_matches(
            "custom_graph.svg",
            "integration/snapshots/build/build_cmd_output_graph_tests/test_build_graph_custom_path_outputs_expected_svg_only.svg.gen",
        );

    assert!(
        !project.path().join("deps.svg").exists(),
        "deps.svg should not be created when custom graph path is provided"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after SVG generation"
    );
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
        .assert_file_exists("deps.svg");

    let first_svg = fs::read_to_string(project.path().join("deps.svg"))
        .expect("failed to read deps.svg after first build");
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after first SVG generation"
    );

    project
        .acton()
        .build()
        .with_graph(None)
        .run()
        .success()
        .assert_file_exists("deps.svg");

    let second_svg = fs::read_to_string(project.path().join("deps.svg"))
        .expect("failed to read deps.svg after second build");
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after second SVG generation"
    );

    assert_eq!(
        first_svg, second_svg,
        "deps.svg should be byte-for-byte deterministic across repeated builds"
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
        .with_graph(Some("graphs/complex.svg"))
        .run()
        .success();

    output
        .assert_contains("dependency graph: graphs/complex.svg")
        .assert_file_exists("graphs/complex.svg");

    let first_svg = read_graph_svg(project.path(), "graphs/complex.svg");
    assert_graph_shape(
        &first_svg,
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
        first_svg.matches("embed code").count(),
        5,
        "expected one 'embed code' edge label per dependency edge"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after SVG generation"
    );

    let rerun = project
        .acton()
        .build()
        .with_graph(Some("graphs/complex.svg"))
        .run()
        .success();
    rerun.assert_file_exists("graphs/complex.svg");

    let second_svg = read_graph_svg(project.path(), "graphs/complex.svg");
    assert_eq!(
        first_svg, second_svg,
        "complex graph SVG should be byte-for-byte deterministic across reruns"
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
        .assert_contains("dependency graph: deps.svg")
        .assert_file_exists("deps.svg");
    let baseline_svg = read_graph_svg(project.path(), "deps.svg");
    assert_graph_shape(
        &baseline_svg,
        &["base", "left", "right", "root"],
        &[
            ("left", "base"),
            ("right", "base"),
            ("root", "left"),
            ("root", "right"),
        ],
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after default path SVG generation"
    );

    let absolute_path = project
        .path()
        .join("graphs")
        .join("absolute")
        .join("variant-absolute.svg")
        .to_string_lossy()
        .into_owned();
    let variant_paths = vec![
        "graphs/variant-relative.svg".to_string(),
        "./graphs/nested/../variant-normalized.svg".to_string(),
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
        let variant_svg = read_graph_svg(project.path(), path);

        assert_eq!(
            baseline_svg, variant_svg,
            "graph content changed for --graph path variant '{}'",
            path
        );
        assert!(
            !project.path().join("deps.dot").exists(),
            "deps.dot should be cleaned up after generating path variant '{}'",
            path
        );
    }

    assert!(
        project
            .path()
            .join("graphs/variant-normalized.svg")
            .exists(),
        "normalized parent-segment path should resolve to graphs/variant-normalized.svg"
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
        .with_graph(Some("graphs/mixed-kinds.svg"))
        .run()
        .success();

    output
        .assert_contains("dependency graph: graphs/mixed-kinds.svg")
        .assert_file_exists("graphs/mixed-kinds.svg")
        .assert_file_snapshot_matches(
            "graphs/mixed-kinds.svg",
            "integration/snapshots/build/build_cmd_output_graph_tests/test_build_graph_complex_mixed_dependency_kinds_match_expected_artifact_content.svg.gen",
        );

    let svg = read_graph_svg(project.path(), "graphs/mixed-kinds.svg");
    assert_graph_shape(
        &svg,
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
        &svg,
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
        svg.matches("embed code").count(),
        4,
        "expected exactly four 'embed code' edge labels in mixed graph output"
    );
    assert_eq!(
        svg.matches("library ref").count(),
        2,
        "expected exactly two 'library ref' edge labels in mixed graph output"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after mixed dependency graph generation"
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
        .with_graph(Some("graphs/target-only.svg"))
        .run()
        .success();

    output
        .assert_contains("dependency graph: graphs/target-only.svg")
        .assert_file_exists("graphs/target-only.svg");

    let svg = read_graph_svg(project.path(), "graphs/target-only.svg");
    assert_graph_shape(
        &svg,
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
        &svg,
        &[
            ("left", "base", "embed code"),
            ("right", "base", "library ref"),
            ("right", "shared", "embed code"),
            ("target", "left", "embed code"),
            ("target", "right", "library ref"),
        ],
    );
    assert!(
        !svg.contains("<title>outside</title>"),
        "filtered graph output should exclude unrelated contract `outside`"
    );
    assert_eq!(
        svg.matches("embed code").count(),
        3,
        "expected exactly three 'embed code' edge labels in filtered graph output"
    );
    assert_eq!(
        svg.matches("library ref").count(),
        2,
        "expected exactly two 'library ref' edge labels in filtered graph output"
    );
    assert!(
        !project.path().join("deps.dot").exists(),
        "deps.dot should be cleaned up after filtered graph generation"
    );
}
