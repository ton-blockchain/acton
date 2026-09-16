use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use std::fs;

const LEAF: &str = r"
fun onInternalMessage(_: InMessage) {}
get fun version(): int { return 1; }
";

const BRANCH: &str = r#"
import "../gen/leaf.code"
fun onInternalMessage(_: InMessage) {}
get fun childCode(): cell { return leafCompiledCode(); }
"#;

const PARENT: &str = r#"
import "../gen/left.code"
import "../gen/right.code"
fun onInternalMessage(_: InMessage) {}
get fun childCodes(): (cell, cell) { return (leftCompiledCode(), rightCompiledCode()); }
"#;

const CHECK_TREE: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"

fun deployCode(code: cell): address {
    val init = ContractState { code, data: createEmptyCell() };
    val dest = AutoDeployAddress { stateInit: init }.calculateAddress();
    net.send(testing.treasury("deployer").address, createMessage({
        bounce: false,
        value: grams("1"),
        dest: { stateInit: init },
    }));
    return dest;
}

fun leafVersion(branch: cell): int {
    val leaf: cell = net.runGetMethod(deployCode(branch), "childCode");
    return net.runGetMethod<int>(deployCode(leaf), "version");
}

fun main() {
    val code = build("parent");
    val (left, right) = net.runGetMethod<(cell, cell)>(deployCode(code), "childCodes");
    println("left version: {}", leafVersion(left));
    println("right version: {}", leafVersion(right));
    println("repeated build matches: {}", build("parent").hash() == code.hash());
}
"#;

fn tree_project() -> ProjectBuilder {
    ProjectBuilder::new("runtime-build-dependencies")
        .contract("leaf", LEAF)
        .contract_with_deps("left", BRANCH, vec!["leaf"])
        .contract_with_deps("right", BRANCH, vec!["leaf"])
        .contract_with_deps("parent", PARENT, vec!["left", "right"])
        .script_file("check", CHECK_TREE)
}

fn change_leaf(project: &Project) {
    fs::write(
        project.path().join("contracts/leaf.tolk"),
        LEAF.replace("return 1", "return 2"),
    )
    .unwrap();
}

#[test]
fn runtime_build_generates_only_requested_dependency_tree() {
    let project = tree_project()
        .contract_with_deps("unrelated", "invalid tolk", vec!["missing"])
        .build();

    project
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/initial.stdout.txt");
}

#[test]
fn runtime_build_refreshes_transitive_dependencies_after_source_change() {
    let project = tree_project().build();
    project.acton().build().run().success();
    change_leaf(&project);

    project
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/updated.stdout.txt");
}

#[test]
fn runtime_build_refreshes_dependencies_with_cleared_cache() {
    let project = tree_project().build();
    project.acton().build().run().success();
    change_leaf(&project);

    project
        .acton()
        .script("scripts/check.tolk")
        .clear_cache()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/cleared-cache.stdout.txt");
}

#[test]
fn runtime_build_invalidates_earlier_explicit_path_builds() {
    let project = tree_project()
        .script_file(
            "explicit_first",
            &CHECK_TREE.replace(
                "val code = build(\"parent\");",
                r#"val _ = build("parent", "contracts/parent.tolk");
                val _ = build("left", "contracts/left.tolk");
                val _ = build("right", "contracts/right.tolk");
                val code = build("parent");"#,
            ),
        )
        .build();
    project.acton().build().run().success();
    change_leaf(&project);

    project
        .acton()
        .script("scripts/explicit_first.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/updated.stdout.txt");
}

#[test]
fn runtime_build_reuses_unchanged_files_and_cache() {
    let project = tree_project().build();
    project.acton().script("scripts/check.tolk").run().success();

    let files = ["gen", "build/cache"]
        .into_iter()
        .flat_map(|dir| walkdir::WalkDir::new(project.path().join(dir)))
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "tolk" || ext == "json")
        })
        .map(|entry| {
            let modified = entry.metadata().unwrap().modified().unwrap();
            (entry.into_path(), modified)
        })
        .collect::<Vec<_>>();

    project
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/cached.stdout.txt");

    let mut rewritten = files
        .into_iter()
        .filter(|(path, modified)| fs::metadata(path).unwrap().modified().unwrap() != *modified)
        .map(|(path, _)| {
            path.strip_prefix(project.path())
                .unwrap()
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();
    rewritten.sort();
    expect_test::expect![[r"
        []
    "]]
    .assert_debug_eq(&rewritten);
}

#[test]
fn runtime_build_reports_invalid_dependency_graphs() {
    for (name, dependencies, expected) in [
        (
            "missing",
            vec!["missing"],
            "Contract 'missing' not found in Acton.toml",
        ),
        (
            "cycle",
            vec!["parent"],
            "Circular dependency detected in contracts: parent → parent",
        ),
    ] {
        ProjectBuilder::new(name)
            .contract_with_deps("parent", LEAF, dependencies)
            .script_file(
                "check",
                "import \"../../lib/build\"\nfun main() { build(\"parent\"); }",
            )
            .build()
            .acton()
            .script("scripts/check.tolk")
            .run()
            .failure()
            .assert_contains(expected)
            .assert_snapshot_matches(&format!(
                "integration/snapshots/runtime-build/{name}.stdout.txt"
            ));
    }
}

#[test]
fn runtime_build_rejects_broken_dependency_instead_of_reusing_old_code() {
    let project = tree_project().build();
    project.acton().build().run().success();
    fs::write(project.path().join("contracts/leaf.tolk"), "invalid tolk").unwrap();

    project
        .acton()
        .script("scripts/check.tolk")
        .run()
        .failure()
        .assert_contains("Failed to build 'leaf' required by 'parent'")
        .assert_snapshot_matches(
            "integration/snapshots/runtime-build/broken-dependency.stdout.txt",
        );
}

#[test]
fn runtime_build_explicit_path_ignores_manifest_dependencies() {
    ProjectBuilder::new("runtime-build-explicit-path")
        .contract_with_deps("parent", LEAF, vec!["missing"])
        .script_file(
            "check",
            r#"
            import "../../lib/build"
            import "../../lib/io"
            fun main() {
                val _ = build("parent", "contracts/parent.tolk");
                println("explicit path compiled");
            }
            "#,
        )
        .build()
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/explicit-path.stdout.txt");
}

#[test]
fn runtime_build_respects_configured_generated_directory() {
    let project = tree_project().build();
    let manifest = project.path().join("Acton.toml");
    let config = fs::read_to_string(&manifest).unwrap();
    fs::write(
        manifest,
        format!("{config}\n[build]\ngen-dir = \"generated\"\n"),
    )
    .unwrap();
    for name in ["left", "right", "parent"] {
        let path = project.path().join(format!("contracts/{name}.tolk"));
        let source = fs::read_to_string(&path).unwrap();
        fs::write(path, source.replace("../gen/", "../generated/")).unwrap();
    }

    project
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/initial.stdout.txt");
}

const CHECK_DEPENDENCY_CODE: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "@stdlib/exotic-cells"

fun main() {
    val init = ContractState { code: build("parent"), data: createEmptyCell() };
    val dest = AutoDeployAddress { stateInit: init }.calculateAddress();
    net.send(testing.treasury("deployer").address, createMessage({
        bounce: false,
        value: grams("1"),
        dest: { stateInit: init },
    }));
    val embedded: cell = net.runGetMethod(dest, "childCode");
    val expected = build("leaf");
    println("dependency code matches: {}", embedded.hash() == expected.hash());
}
"#;

#[test]
fn runtime_build_refreshes_library_reference_with_custom_helper() {
    let project = ProjectBuilder::new("runtime-build-library-reference")
        .contract("leaf", LEAF)
        .contract_with_detailed_deps(
            "parent",
            &BRANCH
                .replace("../gen/leaf.code", "../helpers/library")
                .replace("leafCompiledCode", "leafCode"),
            vec![(
                "leaf",
                Some("library_ref"),
                Some("leafCode"),
                Some("helpers/library.tolk"),
            )],
        )
        .script_file(
            "check",
            &CHECK_DEPENDENCY_CODE.replace(
                "val expected = build(\"leaf\");",
                "val expected = build(\"leaf\").toLibraryReference();",
            ),
        )
        .build();
    project.acton().build().run().success();
    change_leaf(&project);

    project
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/runtime-build/library-reference.stdout.txt",
        );
}

#[test]
fn runtime_build_embeds_precompiled_dependency() {
    let source = ProjectBuilder::new("runtime-build-boc-source")
        .contract("leaf", LEAF)
        .build();
    source.acton().build().run().success();
    let artifact: serde_json::Value =
        serde_json::from_slice(&fs::read(source.path().join("build/leaf.json")).unwrap()).unwrap();
    let code =
        tycho_types::boc::Boc::decode_base64(artifact["code_boc64"].as_str().unwrap()).unwrap();

    ProjectBuilder::new("runtime-build-boc-dependency")
        .contract_from_boc("leaf", tycho_types::boc::Boc::encode(code))
        .contract_with_deps("parent", BRANCH, vec!["leaf"])
        .script_file("check", CHECK_DEPENDENCY_CODE)
        .build()
        .acton()
        .script("scripts/check.tolk")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/runtime-build/precompiled.stdout.txt");
}
