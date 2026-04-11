use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::compilation::{CompilationOrder, extract_compiled_contracts};
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output;
use std::fs;
use std::path::{Path, PathBuf};
use tycho_types::boc::Boc;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn test_build_simple_contract() {
    let project = ProjectBuilder::new("build-simple")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling contracts")
        .assert_contains("Finished");

    let gen_dir = project.path().join("gen");
    assert!(
        !gen_dir.exists(),
        "gen directory should not be created without dependencies"
    );
}

#[test]
fn test_build_ensure_latest_uses_project_root_from_nested_directory() {
    let project = ProjectBuilder::new("build-ensure-latest-project-root")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_stdlib = project.path().join(".acton/tolk-stdlib");
    let nested_stdlib = nested_dir.join(".acton/tolk-stdlib");
    assert!(
        !root_stdlib.exists(),
        "stdlib must not exist before build command"
    );
    assert!(
        !nested_stdlib.exists(),
        "stdlib must not exist in nested cwd before build command"
    );

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .build()
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_stdlib.exists(),
        "stdlib should be installed in project root"
    );
    assert!(
        !nested_stdlib.exists(),
        "stdlib must not be installed in nested cwd"
    );
}

#[test]
fn test_build_with_dependency() {
    let project = ProjectBuilder::new("build-with-dep")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps(
            "parent",
            r#"
            import "../gen/child_code.tolk"

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
        .run()
        .success()
        .assert_contains("Compiling child")
        .assert_contains("Compiling parent")
        .assert_contains("Finished");

    let gen_file = project.path().join("gen/child_code.tolk");
    assert!(gen_file.exists(), "gen/child_code.tolk should be created");

    let content = fs::read_to_string(&gen_file).expect("Should read gen file");
    assert!(
        content.contains("fun childCompiledCode(): cell asm"),
        "Should contain asm function"
    );
    assert!(
        content.contains("Auto-generated dependency code"),
        "Should contain header comment"
    );
}

#[test]
fn test_build_compilation_order() {
    let project = ProjectBuilder::new("build-order")
        .contract("level1", SIMPLE_CONTRACT)
        .contract_with_deps("level2", SIMPLE_CONTRACT, vec!["level1"])
        .contract_with_deps("level3", SIMPLE_CONTRACT, vec!["level2"])
        .contract_with_deps("level4", SIMPLE_CONTRACT, vec!["level3"])
        .build();

    let output = project.acton().build().run().success();

    let order = CompilationOrder::from_stdout(&output.get_normalized_stdout());
    order.assert_chain(&["level1", "level2", "level3", "level4"]);
}

#[test]
fn test_build_diamond_dependency() {
    let project = ProjectBuilder::new("build-diamond")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("left", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("right", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("top", SIMPLE_CONTRACT, vec!["left", "right"])
        .build();

    let output = project.acton().build().run().success();

    let order = CompilationOrder::from_stdout(&output.get_normalized_stdout());

    // base should be compiled before left and right
    order.assert_before("base", "left");
    order.assert_before("base", "right");

    // left and right should be compiled before top
    order.assert_before("left", "top");
    order.assert_before("right", "top");

    assert!(project.path().join("gen/base_code.tolk").exists());
    assert!(project.path().join("gen/left_code.tolk").exists());
    assert!(project.path().join("gen/right_code.tolk").exists());
}

#[test]
fn test_build_circular_dependency_error() {
    let project = ProjectBuilder::new("build-circular")
        .contract_with_deps("a", SIMPLE_CONTRACT, vec!["b"])
        .contract_with_deps("b", SIMPLE_CONTRACT, vec!["c"])
        .contract_with_deps("c", SIMPLE_CONTRACT, vec!["a"])
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_contains("Circular dependency detected")
        .assert_contains("b → c → a → b");
}

#[test]
fn test_build_missing_dependency_error() {
    let project = ProjectBuilder::new("build-missing")
        .contract_with_deps("parent", SIMPLE_CONTRACT, vec!["nonexistent"])
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_contains("depends on 'nonexistent'")
        .assert_contains("not defined in Acton.toml")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_missing_dependency_error.stderr.txt",
        );
}

#[test]
fn test_build_compilation_error() {
    let project = ProjectBuilder::new("build-error")
        .contract(
            "broken",
            r"
            fun onInternalMessage(in: InMessage) {
                val x = nonexistent; // This will cause compilation error
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_contains("error: undefined symbol `nonexistent`")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_compilation_error.stderr.txt",
        );
}

#[test]
fn test_build_gen_file_content() {
    let project = ProjectBuilder::new("build-gen-content")
        .contract("dependency", SIMPLE_CONTRACT)
        .contract_with_deps(
            "main",
            r#"
            import "../gen/dependency_code.tolk"

            fun onInternalMessage(in: InMessage) {}
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec!["dependency"],
        )
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/dependency_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/test_build_gen_file_content.tolk.gen"),
    );
}

#[test]
fn test_build_multiple_dependencies() {
    let project = ProjectBuilder::new("build-multi-deps")
        .contract("utils", SIMPLE_CONTRACT)
        .contract("storage", SIMPLE_CONTRACT)
        .contract("math", SIMPLE_CONTRACT)
        .contract_with_deps("main", SIMPLE_CONTRACT, vec!["utils", "storage", "math"])
        .build();

    project.acton().build().run().success();

    assert!(project.path().join("gen/utils_code.tolk").exists());
    assert!(project.path().join("gen/storage_code.tolk").exists());
    assert!(project.path().join("gen/math_code.tolk").exists());
}

#[test]
fn test_build_cache_hit() {
    let project = ProjectBuilder::new("build-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First build should compile from sources
    let first_output = project.acton().build().run().success();
    let first_stdout = first_output.get_normalized_stdout();
    let first_compiled = extract_compiled_contracts(&first_stdout);

    assert_eq!(
        first_compiled.len(),
        1,
        "Should compile exactly one contract"
    );
    assert_eq!(
        first_compiled[0], "simple",
        "Should compile 'simple' contract"
    );
    assert!(first_stdout.contains("Finished"));

    // Second build should use cache
    let second_output = project.acton().build().run().success();
    let second_stdout = second_output.get_normalized_stdout();
    let second_compiled = extract_compiled_contracts(&second_stdout);

    assert_eq!(
        second_compiled.len(),
        0,
        "Should not compile any contracts (cache hit)"
    );
    assert!(
        second_stdout.contains("Finished"),
        "Should still show 'Finished'"
    );
}

#[test]
fn test_build_complex_graph() {
    // Test a more complex dependency graph
    //        top
    //       /   \
    //    mid1   mid2
    //     / \   /  \
    //  low1 low2 low3
    //    \  |  /
    //     base

    let project = ProjectBuilder::new("build-complex")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("low1", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("low2", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("low3", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("mid1", SIMPLE_CONTRACT, vec!["low1", "low2"])
        .contract_with_deps("mid2", SIMPLE_CONTRACT, vec!["low2", "low3"])
        .contract_with_deps("top", SIMPLE_CONTRACT, vec!["mid1", "mid2"])
        .build();

    let output = project.acton().build().run().success();

    let order = CompilationOrder::from_stdout(&output.get_normalized_stdout());

    // Base before all low-level contracts
    order.assert_before("base", "low1");
    order.assert_before("base", "low2");
    order.assert_before("base", "low3");

    // Low before mid
    order.assert_before("low1", "mid1");
    order.assert_before("low2", "mid1");
    order.assert_before("low2", "mid2");
    order.assert_before("low3", "mid2");

    // Mid before top
    order.assert_before("mid1", "top");
    order.assert_before("mid2", "top");
}

#[test]
fn test_build_with_boc_output() {
    let project = ProjectBuilder::new("build-boc-output")
        .contract_with_output("simple", SIMPLE_CONTRACT, "simple.boc")
        .build();

    project.acton().build().run().success();

    let boc_file = project.path().join("simple.boc");
    assert!(boc_file.exists(), "BoC file should be created");

    let content = fs::read(&boc_file).expect("Should read boc file");
    let hex = Boc::encode_hex(Boc::decode(content).expect("Should decode boc file"));

    assertion().eq(
        hex,
        snapbox::file!("snapshots/test_build_with_boc_output.boc.gen"),
    );
}

#[test]
fn test_build_with_boc_output_to_nonexistent_directory() {
    let project = ProjectBuilder::new("build-boc-output")
        .contract_with_output("simple", SIMPLE_CONTRACT, "nested/dir/simple.boc")
        .build();

    project.acton().build().run().success();

    let boc_file = project.path().join("nested").join("dir").join("simple.boc");
    assert!(boc_file.exists(), "BoC file should be created");

    let content = fs::read(&boc_file).expect("Should read boc file");
    let hex = Boc::encode_hex(Boc::decode(content).expect("Should decode boc file"));

    assertion().eq(
        hex,
        snapbox::file!("snapshots/test_build_with_boc_output_to_nonexistent_directory.boc.gen"),
    );
}

#[test]
fn test_build_no_contracts() {
    let project = ProjectBuilder::new("build-empty").build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("No contracts section found in Acton.toml.");
}

#[test]
fn test_build_self_dependency_error() {
    let project = ProjectBuilder::new("build-self-dep")
        .contract_with_deps("self_ref", SIMPLE_CONTRACT, vec!["self_ref"])
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_contains("Circular dependency");
}

#[test]
fn test_build_gen_file_naming() {
    let project = ProjectBuilder::new("build-naming")
        .contract("my-contract", SIMPLE_CONTRACT)
        .contract("my_contract_2", SIMPLE_CONTRACT)
        .contract("my-contract-3", SIMPLE_CONTRACT)
        .contract_with_deps(
            "main",
            SIMPLE_CONTRACT,
            vec!["my_contract", "my_contract_2", "my_contract_3"],
        )
        .build();

    project.acton().build().run().success();

    assert!(project.path().join("gen/my_contract_code.tolk").exists());
    assert!(project.path().join("gen/my_contract_2_code.tolk").exists());
    assert!(project.path().join("gen/my_contract_3_code.tolk").exists());

    let file1 = fs::read_to_string(project.path().join("gen/my_contract_code.tolk")).unwrap();
    assert!(file1.contains("fun myContractCompiledCode()"));

    let file2 = fs::read_to_string(project.path().join("gen/my_contract_2_code.tolk")).unwrap();
    assert!(file2.contains("fun myContract2CompiledCode()"));

    let file3 = fs::read_to_string(project.path().join("gen/my_contract_3_code.tolk")).unwrap();
    assert!(file3.contains("fun myContract3CompiledCode()"));
}

// ========================================
// CLI Flags Tests
// ========================================

#[test]
fn test_build_with_contract_filter() {
    let project = ProjectBuilder::new("build-filter")
        .contract("base", SIMPLE_CONTRACT)
        .contract("independent", SIMPLE_CONTRACT)
        .contract_with_deps("dependent", SIMPLE_CONTRACT, vec!["base"])
        .build();

    let output = project
        .acton()
        .build()
        .contract("dependent")
        .run()
        .success();

    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    assert!(
        compiled.contains(&"base".to_string()),
        "Should compile base"
    );
    assert!(
        compiled.contains(&"dependent".to_string()),
        "Should compile dependent"
    );
    assert!(
        !compiled.contains(&"independent".to_string()),
        "Should NOT compile independent"
    );
}

#[test]
fn test_build_with_contract_filter_only_target() {
    let project = ProjectBuilder::new("build-filter-single")
        .contract("first", SIMPLE_CONTRACT)
        .contract("second", SIMPLE_CONTRACT)
        .contract("third", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().build().contract("second").run().success();

    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    assert_eq!(compiled.len(), 1, "Should compile exactly one contract");
    assert_eq!(compiled[0], "second", "Should compile 'second'");
}

#[test]
fn test_build_with_contract_nonexistent() {
    let project = ProjectBuilder::new("build-filter-error")
        .contract("existing", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .contract("nonexistent")
        .run()
        .failure()
        .assert_contains("Contract nonexistent not found in Acton.toml");
}

#[test]
fn test_build_with_clear_cache() {
    let project = ProjectBuilder::new("build-clear-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    // Second build should use cache
    let output = project.acton().build().run().success();
    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);
    assert_eq!(compiled.len(), 0, "Should use cache");

    // Third build with --clear-cache should recompile
    let output = project.acton().build().clear_cache().run().success();
    let stdout = output.get_normalized_stdout();

    assert!(
        stdout.contains("Cache cleared"),
        "Should show cache cleared"
    );

    let compiled = extract_compiled_contracts(&stdout);
    assert_eq!(compiled.len(), 1, "Should recompile after cache clear");
    assert_eq!(compiled[0], "simple", "Should compile 'simple'");
}

#[test]
fn test_build_with_graph_default_path() {
    let project = ProjectBuilder::new("build-graph-default")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("child", SIMPLE_CONTRACT, vec!["base"])
        .build();

    project
        .acton()
        .build()
        .with_graph(None)
        .run()
        .success()
        .assert_contains("dependency graph");

    let dot_file = project.path().join("deps.dot");
    assert!(dot_file.exists(), "deps.dot should be created");

    let content = fs::read_to_string(&dot_file).expect("Should read DOT");
    assert!(!content.is_empty(), "deps.dot should not be empty");
}

#[test]
fn test_build_with_graph_custom_path() {
    let project = ProjectBuilder::new("build-graph-custom")
        .contract("parent", SIMPLE_CONTRACT)
        .contract_with_deps("child", SIMPLE_CONTRACT, vec!["parent"])
        .build();

    project
        .acton()
        .build()
        .with_graph(Some("custom_graph.dot"))
        .run()
        .success();

    let dot_file = project.path().join("custom_graph.dot");
    assert!(dot_file.exists(), "custom_graph.dot should be created");

    let default_dot = project.path().join("deps.dot");
    assert!(!default_dot.exists(), "deps.dot should not be created");

    let content = fs::read_to_string(&dot_file).expect("Should read DOT");
    assert!(!content.is_empty(), "deps.dot should not be empty");
}

#[test]
fn test_build_combined_flags() {
    let project = ProjectBuilder::new("build-combined")
        .contract("base", SIMPLE_CONTRACT)
        .contract("independent", SIMPLE_CONTRACT)
        .contract_with_deps("target", SIMPLE_CONTRACT, vec!["base"])
        .build();

    project.acton().build().run().success();

    let output = project
        .acton()
        .build()
        .clear_cache()
        .contract("target")
        .with_graph(Some("filtered.dot"))
        .run()
        .success();

    output.assert_contains("Cache cleared");

    let stdout = output.get_normalized_stdout();

    let compiled = extract_compiled_contracts(&stdout);
    assert!(compiled.contains(&"base".to_string()));
    assert!(compiled.contains(&"target".to_string()));
    assert!(!compiled.contains(&"independent".to_string()));

    let dot_file = project.path().join("filtered.dot");
    assert!(dot_file.exists(), "filtered.dot should be created");
}

// ========================================
// DependencyKind Tests
// ========================================

#[test]
fn test_build_dependency_embed_code() {
    let project = ProjectBuilder::new("dep-embed")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "parent",
            r#"
            import "../gen/child_code.tolk"

            fun onInternalMessage(in: InMessage) {
                val code = childCompiledCode();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![("child", Some("embed_code"), None, None)],
        )
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/child_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assert!(
        content.contains("base64>B B>boc PUSHREF"),
        "Should use EmbedCode ASM (PUSHREF)"
    );
    assert!(
        !content.contains("hashu"),
        "Should NOT use LibraryRef ASM (hashu)"
    );
}

#[test]
fn test_build_dependency_library_ref() {
    let project = ProjectBuilder::new("dep-lib")
        .contract("lib", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            r#"
            import "../gen/lib_code.tolk"

            fun onInternalMessage(in: InMessage) {
                val code = libCompiledCode();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![("lib", Some("library_ref"), None, None)],
        )
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/lib_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assert!(
        content.contains("base64>B B>boc hashu"),
        "Should use LibraryRef ASM (hashu)"
    );
    assert!(
        content.contains("2 8 u, swap 256 u, b>spec PUSHREF"),
        "Should use LibraryRef ASM (spec)"
    );
}

#[test]
fn test_build_dependency_mixed_kinds() {
    let project = ProjectBuilder::new("dep-mixed")
        .contract("embed_dep", SIMPLE_CONTRACT)
        .contract("lib_dep", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            r#"
            import "../gen/embed_dep_code.tolk"
            import "../gen/lib_dep_code.tolk"

            fun onInternalMessage(in: InMessage) {
                val code1 = embedDepCompiledCode();
                val code2 = libDepCompiledCode();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![
                ("embed_dep", Some("embed_code"), None, None),
                ("lib_dep", Some("library_ref"), None, None),
            ],
        )
        .build();

    project.acton().build().run().success();

    let embed_file = project.path().join("gen/embed_dep_code.tolk");
    let embed_content = fs::read_to_string(&embed_file).expect("Should read embed file");
    assert!(
        embed_content.contains("base64>B B>boc PUSHREF"),
        "embed_dep should use EmbedCode"
    );
    assert!(
        !embed_content.contains("hashu"),
        "embed_dep should not use hashu"
    );

    let lib_file = project.path().join("gen/lib_dep_code.tolk");
    let lib_content = fs::read_to_string(&lib_file).expect("Should read lib file");
    assert!(
        lib_content.contains("hashu"),
        "lib_dep should use LibraryRef"
    );
}

#[test]
fn test_build_dependency_custom_function_name() {
    let project = ProjectBuilder::new("dep-custom-func")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "parent",
            r#"
            import "../gen/child_code.tolk"

            fun onInternalMessage(in: InMessage) {
                val code = myCustomFunction();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![("child", None, Some("myCustomFunction"), None)],
        )
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/child_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assert!(
        content.contains("fun myCustomFunction(): cell asm"),
        "Should use custom function name"
    );
    assert!(
        !content.contains("childCompiledCode"),
        "Should NOT use default function name"
    );
}

#[test]
fn test_build_dependency_custom_output_path() {
    let project = ProjectBuilder::new("dep-custom-path")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "parent",
            r#"
            import "../custom/mypath.tolk"

            fun onInternalMessage(in: InMessage) {}
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![("child", None, None, Some("custom/mypath.tolk"))],
        )
        .build();

    project.acton().build().run().success();

    let custom_file = project.path().join("custom/mypath.tolk");
    assert!(custom_file.exists(), "Should create file at custom path");

    let default_file = project.path().join("gen/child_code.tolk");
    assert!(
        !default_file.exists(),
        "Should NOT create file at default path"
    );
}

#[test]
fn test_build_dependency_all_custom_options() {
    let project = ProjectBuilder::new("dep-all-custom")
        .contract("lib", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            r#"
            import "../output/library.tolk"

            fun onInternalMessage(in: InMessage) {
                val code = getLibCode();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec![(
                "lib",
                Some("library_ref"),
                Some("getLibCode"),
                Some("output/library.tolk"),
            )],
        )
        .build();

    project.acton().build().run().success();

    let custom_file = project.path().join("output/library.tolk");
    assert!(custom_file.exists(), "Should create file at custom path");

    let content = fs::read_to_string(&custom_file).expect("Should read file");

    assert!(
        content.contains("fun getLibCode(): cell asm"),
        "Should use custom function name"
    );

    assert!(content.contains("hashu"), "Should use LibraryRef ASM");
}

// ========================================
// BoC Source Files Tests
// ========================================

#[test]
fn test_build_contract_from_boc() {
    let temp_project = ProjectBuilder::new("temp")
        .contract("source", SIMPLE_CONTRACT)
        .build();

    temp_project.acton().build().run().success();

    let boc_bytes = fs::read("tests/integration/testdata/child.boc").unwrap();

    let project = ProjectBuilder::new("boc-source")
        .contract_from_boc("precompiled", boc_bytes)
        .build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Finished");

    let acton_toml =
        fs::read_to_string(project.path().join("Acton.toml")).expect("Should read Acton.toml");
    assert!(
        acton_toml.contains("src = \"contracts/precompiled.boc\""),
        "Should reference .boc file"
    );
}

#[test]
fn test_build_contract_from_invalid_boc() {
    let invalid_boc = vec![0xFF, 0xFF, 0xFF, 0xFF]; // Invalid BoC data

    let project = ProjectBuilder::new("invalid-boc")
        .contract_from_boc("broken", invalid_boc)
        .build();

    project.acton().build().run().failure();
}

#[test]
fn test_build_mixed_boc_and_tolk() {
    let temp_project = ProjectBuilder::new("temp2")
        .contract("lib", SIMPLE_CONTRACT)
        .build();
    temp_project.acton().build().run().success();

    let boc_bytes = fs::read("tests/integration/testdata/child.boc").unwrap();

    let project = ProjectBuilder::new("mixed")
        .contract_from_boc("from_boc", boc_bytes)
        .contract("from_tolk", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("Compiling from_tolk")
        .assert_contains("Finished");

    let stdout = project
        .acton()
        .build()
        .run()
        .success()
        .get_normalized_stdout();

    assert!(
        !stdout.contains("Compiling from_boc"),
        "Should not compile BoC source"
    );
}

// ========================================
// License Header Tests
// ========================================

#[test]
fn test_build_gen_file_with_license() {
    let project = ProjectBuilder::new("license-test")
        .with_license(Some("Apache-2.0"))
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps("parent", SIMPLE_CONTRACT, vec!["child"])
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/child_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assert!(
        content.starts_with("// SPDX-License-Identifier: Apache-2.0"),
        "Should contain license header"
    );
}

#[test]
fn test_build_gen_file_without_license() {
    let project = ProjectBuilder::new("no-license")
        .with_license(None)
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps("parent", SIMPLE_CONTRACT, vec!["child"])
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/child_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assert!(
        !content.contains("SPDX-License-Identifier"),
        "Should NOT contain license header"
    );
    assert!(
        content.starts_with("// Auto-generated"),
        "Should start with auto-generated comment"
    );
}

#[test]
fn test_build_gen_file_default_mit_license() {
    let project = ProjectBuilder::new("default-license")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps("parent", SIMPLE_CONTRACT, vec!["child"])
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/child_code.tolk");
    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assert!(
        content.contains("// SPDX-License-Identifier: MIT"),
        "Should contain default MIT license"
    );
}

// ========================================
// Edge Cases and Error Handling Tests
// ========================================

#[test]
fn test_build_missing_boc_file() {
    let project = ProjectBuilder::new("missing-boc").build();

    let toml_content = r#"[package]
name = "missing-boc"
description = ""
version = "0.1.0"

[contracts.broken]
display-name = "broken"
src = "contracts/missing.boc"
depends = []
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_missing_boc_file.stderr.txt",
        );
}

#[test]
fn test_build_missing_acton_toml() {
    let project = ProjectBuilder::new("build-missing-toml")
        .without_acton_toml()
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_missing_acton_toml.stderr.txt",
        )
        .assert_contains("Acton.toml not found");
}

#[test]
fn test_build_invalid_acton_toml() {
    let project = ProjectBuilder::new("build-invalid-toml").build();

    fs::write(
        project.path().join("Acton.toml"),
        r#"
[package
name = "invalid-toml"
description = ""
version = "0.1.0"

[contracts]
missing_closing_bracket = { display-name = "test", src = "contracts/test.tolk" }
"#,
    )
    .expect("Failed to write invalid TOML");

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_invalid_acton_toml.stderr.txt",
        )
        .assert_contains("TOML parse error");
}

#[test]
fn test_build_contract_source_file_not_found() {
    let project = ProjectBuilder::new("contract-file-missing").build();

    let toml_content = r#"[package]
name = "contract-file-missing"
description = ""
version = "0.1.0"

[contracts.missing]
display-name = "missing"
src = "contracts/missing_file.tolk"
depends = []
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_contract_source_file_not_found.stderr.txt",
        );
}

#[test]
fn test_build_contract_source_file_not_found_with_abs_path() {
    let project = ProjectBuilder::new("contract-file-missing").build();

    let toml_content = r#"[package]
name = "contract-file-missing"
description = ""
version = "0.1.0"

[contracts.missing]
display-name = "missing"
src = "/contracts/missing_file.tolk"
depends = []
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_contract_source_file_not_found_with_abs_path.stderr.txt",
        );
}

#[test]
fn test_build_contract_invalid_file_extension() {
    let project = ProjectBuilder::new("invalid-extension")
        .raw_file("contracts/simple.txt", SIMPLE_CONTRACT)
        .build();

    let toml_content = r#"[package]
name = "invalid-extension"
description = ""
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple.txt"
depends = []
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_contract_invalid_file_extension.stdout.txt",
        );
}

#[test]
fn test_build_output_boc_write_error() {
    let project = ProjectBuilder::new("boc-write-error")
        .contract_with_output("simple", SIMPLE_CONTRACT, "readonly/output.boc")
        .build();

    // Create a readonly directory to simulate write error
    let readonly_dir = project.path().join("readonly");
    fs::create_dir(&readonly_dir).expect("Create readonly dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444); // readonly
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_output_boc_write_error.stderr.txt",
        );
}

#[test]
fn test_build_no_contracts_section() {
    let project = ProjectBuilder::new("contracts-missing").build();

    let toml_content = r#"[package]
name = "contract-file-missing"
description = ""
version = "0.1.0"

"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_build_no_contracts_section.stdout.txt",
        );
}

#[test]
fn test_build_empty_contracts_section() {
    let project = ProjectBuilder::new("empty-contracts").build();

    let toml_content = r#"[package]
name = "empty-contracts"
description = ""
version = "0.1.0"

[contracts]
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_contains("No contracts to build.");
}

#[test]
fn test_build_dependency_custom_path_write_error() {
    let project = ProjectBuilder::new("dep-path-error")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "parent",
            SIMPLE_CONTRACT,
            vec![("child", None, None, Some("readonly/child_code.tolk"))],
        )
        .build();

    let readonly_dir = project.path().join("readonly");
    fs::create_dir(&readonly_dir).expect("Create readonly dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444); // readonly
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_dependency_custom_path_write_error.stderr.txt",
        );
}

#[test]
fn test_build_contract_with_special_characters_in_path() {
    let project = ProjectBuilder::new("special-chars-path")
        .contract("simple file", SIMPLE_CONTRACT)
        .build();

    let toml_content = r#"[package]
name = "special-chars-path"
description = ""
version = "0.1.0"

[contracts.simple]
display-name = "simple"
src = "contracts/simple file.tolk"
depends = []
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_build_contract_with_special_characters_in_path.stdout.txt",
        );
}

#[test]
fn test_build_corrupted_cache_file() {
    let project = ProjectBuilder::new("corrupted-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First build to create cache
    project.acton().build().run().success();

    // Manually corrupt the cache file by writing invalid base64
    let cache_dir = project.path().join("build/cache");
    if cache_dir.exists() {
        let cache_file = first_cache_json_file(&cache_dir);
        fs::write(&cache_file, "invalid base64 data!!!").unwrap();
    }

    // Second build should recompile from source instead of using the broken cache entry
    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());
    assert_eq!(
        compiled,
        vec!["simple"],
        "Should recompile after cache corruption"
    );
}

#[test]
fn test_build_ignores_unrelated_corrupted_cache_file_and_keeps_it() {
    let project = ProjectBuilder::new("build-unrelated-corrupted-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let cache_dir = project.path().join("build/cache");
    fs::create_dir_all(&cache_dir).unwrap();
    let broken_path = cache_dir.join("broken.json");
    fs::write(&broken_path, "not-json").unwrap();

    project.acton().build().run().success();

    assert!(
        broken_path.exists(),
        "Unrelated corrupted cache entry should not be eagerly removed"
    );

    let output = project.acton().build().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());
    assert!(
        compiled.is_empty(),
        "Existing valid cache entries should still be reused with unrelated junk present"
    );
}

#[test]
fn test_build_clear_cache_removes_nested_cache_subdirectories() {
    let project = ProjectBuilder::new("build-clear-cache-removes-subdirs")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    let cache_dir = project.path().join("build/cache");
    let debug_dir = cache_dir.join("debug");
    let nested_dir = cache_dir.join("nested");
    fs::create_dir_all(&debug_dir).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(debug_dir.join("junk.json"), "junk").unwrap();
    fs::write(nested_dir.join("junk.txt"), "junk").unwrap();

    let output = project.acton().build().clear_cache().run().success();
    let compiled = extract_compiled_contracts(&output.get_normalized_stdout());
    assert_eq!(
        compiled,
        vec!["simple"],
        "clear-cache should force recompilation after removing nested cache dirs"
    );
    assert!(
        !debug_dir.exists(),
        "clear-cache should remove nested debug cache directory"
    );
    assert!(
        !nested_dir.exists(),
        "clear-cache should remove arbitrary nested cache directory"
    );
}

fn first_cache_json_file(cache_dir: &Path) -> PathBuf {
    fs::read_dir(cache_dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
        .unwrap_or_else(|| panic!("No cache json file found in {}", cache_dir.display()))
}

#[test]
fn test_build_contract_with_invalid_output_path() {
    let project = ProjectBuilder::new("invalid-output-path")
        .contract_with_output("simple", SIMPLE_CONTRACT, "")
        .build();

    project.acton().build().run().success(); // Empty output path should be ignored
}

#[test]
fn test_build_contract_with_numeric_name_dependency() {
    let project = ProjectBuilder::new("numeric-name-dep")
        .contract("123contract", SIMPLE_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            SIMPLE_CONTRACT,
            vec![("123contract", None, None, None)],
        )
        .build();

    project.acton().build().run().success();

    let gen_file = project.path().join("gen/123contract_code.tolk");
    assert!(
        gen_file.exists(),
        "Should create file for numeric contract name"
    );

    let content = fs::read_to_string(&gen_file).expect("Should read gen file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/test_build_contract_with_numeric_name_dependency.tolk.gen"),
    );
}

#[test]
fn test_build_contract_syntax_error() {
    let project = ProjectBuilder::new("syntax-error")
        .contract(
            "broken",
            r"
            fun onInternalMessage(in: InMessage) {
                val x = ;
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_contract_syntax_error.stderr.txt",
        );
}

#[test]
fn test_build_several_contracts_with_syntax_error() {
    let project = ProjectBuilder::new("syntax-error")
        .contract(
            "broken1",
            r"
            fun onInternalMessage(in: InMessage) {
                val x = ;
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .contract(
            "broken2",
            r"
            fun onInternalMessage(in: InMessage) {
                val x;
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_several_contracts_with_syntax_error.stderr.txt",
        );
}

#[test]
fn test_build_good_and_bad_contracts_with_syntax_error() {
    let project = ProjectBuilder::new("syntax-error")
        .contract(
            "broken1",
            r"
            fun onInternalMessage(in: InMessage) {
                val x = 10;
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .contract(
            "ok2",
            r"
            fun onInternalMessage(in: InMessage) {
                val x =;
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .build();

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_good_and_bad_contracts_with_syntax_error.stderr.txt",
        );
}

#[test]
fn test_build_corrupted_boc_file() {
    let project = ProjectBuilder::new("corrupted-boc")
        .raw_file("contracts/corrupted.boc", "not a valid boc file!!!")
        .build();

    let toml_content = r#"[package]
name = "corrupted-boc"
description = ""
version = "0.1.0"

[contracts.corrupted]
display-name = "corrupted"
src = "contracts/corrupted.boc"
depends = []
"#;
    fs::write(project.path().join("Acton.toml"), toml_content).expect("Write Acton.toml");

    project
        .acton()
        .build()
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_corrupted_boc_file.stderr.txt",
        );
}

#[test]
fn test_build_contract_filter_nonexistent() {
    let project = ProjectBuilder::new("filter-nonexistent")
        .contract("existing", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .contract("nonexistent")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_contract_filter_nonexistent.stderr.txt",
        );
}

#[test]
fn test_build_with_default_out_dir() {
    let project = ProjectBuilder::new("build-default-out-dir")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    let json_file = project.path().join("build/simple.json");
    assert!(json_file.exists(), "build/simple.json should be created");

    let content = fs::read_to_string(&json_file).expect("Should read JSON file");
    let json: serde_json::Value = serde_json::from_str(&content).expect("Should parse JSON");

    assert!(
        json.get("code_boc64").is_some(),
        "Should contain code_boc64 field"
    );
    assert!(json.get("hash").is_some(), "Should contain hash field");

    let _ = json["code_boc64"]
        .as_str()
        .expect("code_boc64 should be string");
    let hash = json["hash"].as_str().expect("hash should be string");

    // Verify hash is a valid hex string (64 characters for SHA-256)
    assert_eq!(hash.len(), 64, "Hash should be 64 hex characters");
    assert!(
        hash.chars().all(|c| c.is_ascii_hexdigit()),
        "Hash should contain only hex digits"
    );
}

#[test]
fn test_build_with_custom_out_dir() {
    let project = ProjectBuilder::new("build-custom-out-dir")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_out_dir("artifacts")
        .run()
        .success();

    let json_file = project.path().join("artifacts/simple.json");
    assert!(
        json_file.exists(),
        "artifacts/simple.json should be created"
    );

    // Default build directory should not be created
    let default_json = project.path().join("build/simple.json");
    assert!(!default_json.exists(), "build/simple.json should not exist");

    let content = fs::read_to_string(&json_file).expect("Should read JSON file");
    let json: serde_json::Value = serde_json::from_str(&content).expect("Should parse JSON");

    assert!(
        json.get("code_boc64").is_some(),
        "Should contain code_boc64 field"
    );
    assert!(json.get("hash").is_some(), "Should contain hash field");
}

#[test]
fn test_build_with_out_dir_from_config() {
    let project = ProjectBuilder::new("build-out-dir-config")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("Should read Acton.toml");
    acton_toml.push_str("\n[build]\nout-dir = \"config-artifacts\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("Should write Acton.toml");

    project.acton().build().run().success();

    let config_json = project.path().join("config-artifacts/simple.json");
    assert!(
        config_json.exists(),
        "config-artifacts/simple.json should be created"
    );
    assert!(
        !project.path().join("build/simple.json").exists(),
        "build/simple.json should not be created when [build].out-dir is set"
    );
}

#[test]
fn test_build_with_out_dir_cli_overrides_config() {
    let project = ProjectBuilder::new("build-out-dir-cli-overrides-config")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("Should read Acton.toml");
    acton_toml.push_str("\n[build]\nout-dir = \"config-artifacts\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("Should write Acton.toml");

    project
        .acton()
        .build()
        .with_out_dir("cli-artifacts")
        .run()
        .success();

    assert!(
        project.path().join("cli-artifacts/simple.json").exists(),
        "cli-artifacts/simple.json should be created"
    );
    assert!(
        !project.path().join("config-artifacts/simple.json").exists(),
        "config-artifacts/simple.json should not be created when CLI override is used"
    );
}

#[test]
fn test_build_without_output_fift_does_not_emit_fift_by_default() {
    let project = ProjectBuilder::new("build-output-fift-default-off")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    assert!(
        !project.path().join("build/fift/simple.fif").exists(),
        "build/fift/simple.fif should not be created when output-fift is not configured"
    );
}

#[test]
fn test_build_with_output_fift_cli() {
    let project = ProjectBuilder::new("build-output-fift-cli")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_output_fift("build/fift")
        .run()
        .success();

    let fift_file = project.path().join("build/fift/simple.fif");
    assert!(
        fift_file.exists(),
        "build/fift/simple.fif should be created"
    );

    let content = fs::read_to_string(&fift_file).expect("Should read Fift file");
    assert!(!content.is_empty(), "Fift file should not be empty");
}

#[test]
fn test_build_with_output_fift_from_config() {
    let project = ProjectBuilder::new("build-output-fift-config")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("Should read Acton.toml");
    acton_toml.push_str("\n[build]\noutput-fift = \"build/fift\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("Should write Acton.toml");

    project.acton().build().run().success();

    let fift_file = project.path().join("build/fift/simple.fif");
    assert!(
        fift_file.exists(),
        "build/fift/simple.fif should be created"
    );
}

#[test]
fn test_build_with_output_fift_cli_overrides_config() {
    let project = ProjectBuilder::new("build-output-fift-override")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("Should read Acton.toml");
    acton_toml.push_str("\n[build]\noutput-fift = \"config/fift\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("Should write Acton.toml");

    project
        .acton()
        .build()
        .with_output_fift("cli/fift")
        .run()
        .success();

    let cli_fift_file = project.path().join("cli/fift/simple.fif");
    assert!(
        cli_fift_file.exists(),
        "cli/fift/simple.fif should be created"
    );

    let config_fift_file = project.path().join("config/fift/simple.fif");
    assert!(
        !config_fift_file.exists(),
        "config/fift/simple.fif should not be created when CLI override is used"
    );
}

#[test]
fn test_build_with_output_fift_for_multiple_contracts() {
    let project = ProjectBuilder::new("build-output-fift-multi")
        .contract("first", SIMPLE_CONTRACT)
        .contract("second", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_output_fift("build/fift")
        .run()
        .success();

    let first_fift_file = project.path().join("build/fift/first.fif");
    let second_fift_file = project.path().join("build/fift/second.fif");

    assert!(
        first_fift_file.exists(),
        "build/fift/first.fif should be created"
    );
    assert!(
        second_fift_file.exists(),
        "build/fift/second.fif should be created"
    );
}

#[test]
fn test_build_with_output_fift_write_error_is_non_zero() {
    let project = ProjectBuilder::new("build-output-fift-write-error")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let readonly_dir = project.path().join("readonly");
    fs::create_dir(&readonly_dir).expect("Create readonly dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }

    project
        .acton()
        .build()
        .with_output_fift("readonly")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_output_fift_write_error.stderr.txt",
        );
}

#[test]
fn test_build_with_out_dir_write_error_is_non_zero() {
    let project = ProjectBuilder::new("build-out-dir-write-error")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let readonly_dir = project.path().join("readonly");
    fs::create_dir(&readonly_dir).expect("Create readonly dir");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&readonly_dir, perms).unwrap();
    }

    project
        .acton()
        .build()
        .with_out_dir("readonly")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_out_dir_write_error.stderr.txt",
        );
}

#[test]
fn test_build_with_output_fift_skips_boc_sources() {
    let boc_bytes = fs::read("tests/integration/testdata/child.boc").unwrap();

    let project = ProjectBuilder::new("build-output-fift-mixed")
        .contract_from_boc("from_boc", boc_bytes)
        .contract("from_tolk", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_output_fift("build/fift")
        .run()
        .success();

    let tolk_fift_file = project.path().join("build/fift/from_tolk.fif");
    assert!(
        tolk_fift_file.exists(),
        "build/fift/from_tolk.fif should be created"
    );

    let boc_fift_file = project.path().join("build/fift/from_boc.fif");
    assert!(
        !boc_fift_file.exists(),
        "build/fift/from_boc.fif should not be created for precompiled .boc sources"
    );
}

#[test]
fn test_build_with_gen_dir_from_config() {
    let project = ProjectBuilder::new("build-gen-dir-config")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps(
            "parent",
            r#"
            import "../custom-gen/child_code.tolk"

            fun onInternalMessage(in: InMessage) {
                val code = childCompiledCode();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec!["child"],
        )
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("Should read Acton.toml");
    acton_toml.push_str("\n[build]\ngen-dir = \"custom-gen\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("Should write Acton.toml");

    project.acton().build().run().success();

    assert!(
        project.path().join("custom-gen/child_code.tolk").exists(),
        "custom-gen/child_code.tolk should be created"
    );
    assert!(
        !project.path().join("gen/child_code.tolk").exists(),
        "default gen/child_code.tolk should not be created when [build].gen-dir is set"
    );
}

#[test]
fn test_build_with_gen_dir_cli_overrides_config() {
    let project = ProjectBuilder::new("build-gen-dir-cli-overrides-config")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps(
            "parent",
            r#"
            import "../cli-gen/child_code.tolk"

            fun onInternalMessage(in: InMessage) {
                val code = childCompiledCode();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
            vec!["child"],
        )
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_toml_path).expect("Should read Acton.toml");
    acton_toml.push_str("\n[build]\ngen-dir = \"config-gen\"\n");
    fs::write(&acton_toml_path, acton_toml).expect("Should write Acton.toml");

    project
        .acton()
        .build()
        .with_gen_dir("cli-gen")
        .run()
        .success();

    assert!(
        project.path().join("cli-gen/child_code.tolk").exists(),
        "cli-gen/child_code.tolk should be created"
    );
    assert!(
        !project.path().join("config-gen/child_code.tolk").exists(),
        "config-gen/child_code.tolk should not be created when CLI override is used"
    );
}

#[test]
fn test_build_with_gen_dir_cli() {
    let project = ProjectBuilder::new("build-gen-dir-cli")
        .contract("child", SIMPLE_CONTRACT)
        .contract_with_deps(
            "parent",
            r#"
            import "../cli-gen/child_code.tolk"

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
        .with_gen_dir("cli-gen")
        .run()
        .success();

    assert!(
        project.path().join("cli-gen/child_code.tolk").exists(),
        "cli-gen/child_code.tolk should be created"
    );
    assert!(
        !project.path().join("gen/child_code.tolk").exists(),
        "default gen/child_code.tolk should not be created when CLI --gen-dir is set"
    );
}

#[test]
fn test_build_multiple_contracts_artifacts() {
    let project = ProjectBuilder::new("build-multi-artifacts")
        .contract("contract1", SIMPLE_CONTRACT)
        .contract("contract2", SIMPLE_CONTRACT)
        .contract("contract3", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    // Check that all JSON files are created
    let json1 = project.path().join("build/contract1.json");
    let json2 = project.path().join("build/contract2.json");
    let json3 = project.path().join("build/contract3.json");

    assert!(json1.exists(), "build/contract1.json should be created");
    assert!(json2.exists(), "build/contract2.json should be created");
    assert!(json3.exists(), "build/contract3.json should be created");

    // Verify all files contain valid JSON with required fields
    for (path, name) in [
        (json1, "contract1"),
        (json2, "contract2"),
        (json3, "contract3"),
    ] {
        let content = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(
            json.get("code_boc64").is_some(),
            "Should contain code_boc64 field for {name}"
        );
        assert!(
            json.get("hash").is_some(),
            "Should contain hash field for {name}"
        );
    }
}

#[test]
fn test_build_artifacts_with_dependencies() {
    let project = ProjectBuilder::new("build-artifacts-deps")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("dependent", SIMPLE_CONTRACT, vec!["base"])
        .build();

    project.acton().build().run().success();

    // Check both artifacts are created
    let base_json = project.path().join("build/base.json");
    let dependent_json = project.path().join("build/dependent.json");

    assert!(base_json.exists(), "build/base.json should be created");
    assert!(
        dependent_json.exists(),
        "build/dependent.json should be created"
    );

    // Verify JSON structure
    for path in [base_json, dependent_json] {
        let content = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(
            json.get("code_boc64").is_some(),
            "Should contain code_boc64 field"
        );
        assert!(json.get("hash").is_some(), "Should contain hash field");

        let _ = json["code_boc64"].as_str().unwrap();
        let hash = json["hash"].as_str().unwrap();

        // Verify hash is a valid hex string (64 characters for SHA-256)
        assert_eq!(hash.len(), 64, "Hash should be 64 hex characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Hash should contain only hex digits"
        );
    }
}

#[test]
fn test_build_artifacts_nested_directory() {
    let project = ProjectBuilder::new("build-nested-dir")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_out_dir("dist/artifacts/build")
        .run()
        .success();

    let json_file = project.path().join("dist/artifacts/build/simple.json");
    assert!(
        json_file.exists(),
        "Nested directory structure should be created"
    );

    let content = fs::read_to_string(&json_file).expect("Should read JSON file");
    let json: serde_json::Value = serde_json::from_str(&content).expect("Should parse JSON");

    assert!(
        json.get("code_boc64").is_some(),
        "Should contain code_boc64 field"
    );
    assert!(json.get("hash").is_some(), "Should contain hash field");
}

#[test]
fn test_build_artifacts_created_with_cache() {
    let project = ProjectBuilder::new("build-artifacts-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First build, create cache and artifacts
    project.acton().build().run().success();

    let json_file = project.path().join("build/simple.json");
    assert!(
        json_file.exists(),
        "build/simple.json should be created on first build"
    );

    // Verify JSON content is valid
    let content = fs::read_to_string(&json_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        json.get("code_boc64").is_some(),
        "Should contain code_boc64 field"
    );
    assert!(json.get("hash").is_some(), "Should contain hash field");

    // Delete the JSON artifact
    fs::remove_file(&json_file).expect("Should delete JSON file");
    assert!(!json_file.exists(), "JSON file should be deleted");

    // Second build, should use cache but still create artifacts
    let output = project.acton().build().run().success();
    let stdout = output.get_normalized_stdout();
    let compiled = extract_compiled_contracts(&stdout);

    // Should not compile (cache hit)
    assert_eq!(compiled.len(), 0, "Should not recompile (cache hit)");

    // But JSON artifact should be recreated
    assert!(
        json_file.exists(),
        "build/simple.json should be recreated even with cache hit"
    );

    // Verify JSON content is the same
    let new_content = fs::read_to_string(&json_file).unwrap();
    let new_json: serde_json::Value = serde_json::from_str(&new_content).unwrap();

    assert_eq!(json, new_json, "JSON content should be identical");
    assert_eq!(
        content, new_content,
        "File content should be byte-for-byte identical"
    );
}

#[test]
fn test_build_with_info_flag() {
    let project = ProjectBuilder::new("build-info")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_info()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/test_build_with_info_flag.stdout.txt");
}

#[test]
fn test_build_with_info_flag_from_cache() {
    let project = ProjectBuilder::new("build-info")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_info()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_build_with_info_flag_from_cache_before.stdout.txt",
        );

    // Build form cache
    project
        .acton()
        .build()
        .with_info()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_build_with_info_flag_from_cache_after.stdout.txt",
        );
}

#[test]
fn test_build_with_info_flag_for_several_contracts() {
    let project = ProjectBuilder::new("build-info")
        .contract("simple", SIMPLE_CONTRACT)
        .contract("simple2", SIMPLE_CONTRACT)
        .contract("simple3", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .build()
        .with_info()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_build_with_info_flag_for_several_contracts.stdout.txt",
        );
}

#[test]
fn test_build_with_dependency_with_compilation_error() {
    let project = ProjectBuilder::new("build-with-dep")
        .contract(
            "child",
            r"
                fun onInternalMessage(in: InMessage) {
                    let a = 10;
                }
            ",
        )
        .contract_with_deps(
            "parent",
            r#"
                import "../gen/child_code.tolk"

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
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_build_with_dependency_with_compilation_error.stderr.txt",
        );
}
