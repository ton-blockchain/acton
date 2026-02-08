use crate::support::TestOutputExt;
use crate::support::compilation::CompilationOrder;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

// ========================================
// Basic Cache Tests
// ========================================

#[test]
fn test_incremental_no_changes() {
    let project = ProjectBuilder::new("incr-no-changes")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First build with compile from source
    let first = project.acton().build().run().success();
    let first_order = CompilationOrder::from_stdout(&first.get_normalized_stdout());
    assert_eq!(first_order.count(), 1, "First build should compile");
    assert!(first_order.contains("simple"));

    // Second build with cache
    let second = project.acton().build().run().success();
    let second_order = CompilationOrder::from_stdout(&second.get_normalized_stdout());
    assert_eq!(second_order.count(), 0, "Second build should use cache");
}

#[test]
fn test_incremental_single_contract_change() {
    let project = ProjectBuilder::new("incr-single-change")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First build
    project.acton().build().run().success();

    // Modify contract
    fs::write(
        project.path().join("contracts/simple.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Added comment
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write modified contract");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());
    assert_eq!(order.count(), 1, "Should recompile modified contract");
    assert!(order.contains("simple"));
}

#[test]
fn test_incremental_whitespace_only_change() {
    let project = ProjectBuilder::new("incr-whitespace")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    fs::write(
        project.path().join("contracts/simple.tolk"),
        r"

        fun onInternalMessage(in: InMessage) {}

        fun onBouncedMessage(_: InMessageBounced) {}

    ",
    )
    .expect("Write contract");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());
    assert_eq!(order.count(), 1, "Should recompile on whitespace change");
}

// ========================================
// Dependency Chain Cache Tests
// ========================================

#[test]
fn test_incremental_base_contract_change() {
    let project = ProjectBuilder::new("incr-base-change")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("dependent", SIMPLE_CONTRACT, vec!["base"])
        .build();

    // First build
    project.acton().build().run().success();

    // Cache hit, no changes
    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());
    assert_eq!(order.count(), 0, "Should use cache for both");

    // Modify base contract
    fs::write(
        project.path().join("contracts/base.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified base
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write base");

    let third = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&third.get_normalized_stdout());

    order.assert_chain(&["base", "dependent"]);
}

#[test]
fn test_incremental_dependent_contract_change() {
    let project = ProjectBuilder::new("incr-dep-change")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("dependent", SIMPLE_CONTRACT, vec!["base"])
        .build();

    project.acton().build().run().success();

    // Modify only dependent
    fs::write(
        project.path().join("contracts/dependent.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified dependent
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write dependent");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert_eq!(order.count(), 1, "Should recompile only dependent");
    assert!(order.contains("dependent"));
    assert!(!order.contains("base"), "Should NOT recompile base");
}

#[test]
fn test_incremental_deep_chain_base_change() {
    let project = ProjectBuilder::new("incr-deep-chain")
        .contract("level0", SIMPLE_CONTRACT)
        .contract_with_deps("level1", SIMPLE_CONTRACT, vec!["level0"])
        .contract_with_deps("level2", SIMPLE_CONTRACT, vec!["level1"])
        .contract_with_deps("level3", SIMPLE_CONTRACT, vec!["level2"])
        .build();

    project.acton().build().run().success();

    // Modify level0 (base)
    fs::write(
        project.path().join("contracts/level0.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified level0
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write level0");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    // Should recompile all contracts in chain
    order.assert_chain(&["level0", "level1", "level2", "level3"]);
}

#[test]
fn test_incremental_deep_chain_mid_change() {
    let project = ProjectBuilder::new("incr-mid-chain")
        .contract("level0", SIMPLE_CONTRACT)
        .contract_with_deps("level1", SIMPLE_CONTRACT, vec!["level0"])
        .contract_with_deps("level2", SIMPLE_CONTRACT, vec!["level1"])
        .contract_with_deps("level3", SIMPLE_CONTRACT, vec!["level2"])
        .build();

    project.acton().build().run().success();

    // Modify level1 (middle)
    fs::write(
        project.path().join("contracts/level1.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified level1
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write level1");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    // Should recompile level1, level0 should use cache
    assert!(!order.contains("level0"), "Should NOT recompile level0");
    order.assert_chain(&["level1", "level2", "level3"]);
}

// ========================================
// Diamond Dependency Cache Tests
// ========================================

#[test]
fn test_incremental_diamond_base_change() {
    let project = ProjectBuilder::new("incr-diamond-base")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("left", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("right", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("top", SIMPLE_CONTRACT, vec!["left", "right"])
        .build();

    project.acton().build().run().success();

    // Modify base
    fs::write(
        project.path().join("contracts/base.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified base
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write base");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert!(order.contains("base"), "Should recompile base");
    assert!(order.contains("left"), "Should recompile left");
    assert!(order.contains("right"), "Should recompile right");
    assert!(order.contains("top"), "Should recompile top");
}

#[test]
fn test_incremental_diamond_branch_change() {
    let project = ProjectBuilder::new("incr-diamond-branch")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("left", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("right", SIMPLE_CONTRACT, vec!["base"])
        .contract_with_deps("top", SIMPLE_CONTRACT, vec!["left", "right"])
        .build();

    project.acton().build().run().success();

    // Modify only left branch
    fs::write(
        project.path().join("contracts/left.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified left
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write left");

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert!(order.contains("left"), "Should recompile left");
    assert!(order.contains("top"), "Should recompile top");
    assert!(!order.contains("base"), "Should NOT recompile base");
    assert!(!order.contains("right"), "Should NOT recompile right");
}

// ========================================
// Import Change Detection Tests
// ========================================

#[test]
fn test_incremental_library_file_change() {
    let project = ProjectBuilder::new("incr-lib-change")
        .file(
            "common/utils",
            r"
            fun helper(): int {
                return 42;
            }
        ",
        )
        .contract(
            "main",
            r#"
            import "../common/utils"

            fun onInternalMessage(in: InMessage) {
                val x = helper();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
        )
        .build();

    project.acton().build().run().success();

    // Modify library file
    fs::write(
        project.path().join("common/utils.tolk"),
        r"
            fun helper(): int {
                return 43; // Changed
            }
        ",
    )
    .expect("Write lib");

    // Should recompile main tha depends on lib
    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert_eq!(order.count(), 1, "Should recompile main");
    assert!(order.contains("main"));
}

#[test]
fn test_incremental_nested_import_change() {
    let project = ProjectBuilder::new("incr-nested-import")
        .file(
            "common/base",
            r"
            fun baseFunc(): int { return 1; }
        ",
        )
        .file(
            "common/wrapper",
            r#"
            import "./base"
            fun wrapperFunc(): int { return baseFunc(); }
        "#,
        )
        .contract(
            "main",
            r#"
            import "../common/wrapper"

            fun onInternalMessage(in: InMessage) {
                val x = wrapperFunc();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
        )
        .build();

    project.acton().build().run().success();

    // Modify base common nested import
    fs::write(
        project.path().join("common/base.tolk"),
        r"
            fun baseFunc(): int { return 2; } // Changed
        ",
    )
    .expect("Write base lib");

    // Should recompile main
    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert!(
        order.contains("main"),
        "Should recompile main due to nested import change"
    );
}

#[test]
fn test_incremental_nested_import_change_with_mappings() {
    let project = ProjectBuilder::new("incr-nested-import")
        .mapping("@common", "common")
        .file(
            "common/base",
            r"
            fun baseFunc(): int { return 1; }
        ",
        )
        .file(
            "common/wrapper",
            r#"
            import "@common/base"
            fun wrapperFunc(): int { return baseFunc(); }
        "#,
        )
        .contract(
            "main",
            r#"
            import "@common/wrapper"

            fun onInternalMessage(in: InMessage) {
                val x = wrapperFunc();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
        )
        .build();

    project.acton().build().run().success();

    // Modify base common nested import
    fs::write(
        project.path().join("common/base.tolk"),
        r"
            fun baseFunc(): int { return 2; } // Changed
        ",
    )
    .expect("Write base lib");

    // Should recompile main
    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert!(
        order.contains("main"),
        "Should recompile main due to nested import change"
    );
}

// ========================================
// Clear Cache Tests
// ========================================

#[test]
fn test_clear_cache_flag() {
    let project = ProjectBuilder::new("clear-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First build
    project.acton().build().run().success();

    // Second build with cache
    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());
    assert_eq!(order.count(), 0, "Should use cache");

    // Third build with --clear-cache
    let third = project.acton().build().clear_cache().run().success();
    let order = CompilationOrder::from_stdout(&third.get_normalized_stdout());

    assert_eq!(order.count(), 1, "Should recompile after cache clear");
    assert!(order.contains("simple"));
}

#[test]
fn test_clear_cache_with_dependencies() {
    let project = ProjectBuilder::new("clear-cache-deps")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("dependent", SIMPLE_CONTRACT, vec!["base"])
        .build();

    project.acton().build().run().success();

    // Clear cache and rebuild
    let second = project.acton().build().clear_cache().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert_eq!(order.count(), 2, "Should recompile both after cache clear");
    order.assert_chain(&["base", "dependent"]);
}

// ========================================
// Filtered Build Cache Tests
// ========================================

#[test]
fn test_incremental_filtered_build() {
    let project = ProjectBuilder::new("incr-filtered")
        .contract("contract1", SIMPLE_CONTRACT)
        .contract("contract2", SIMPLE_CONTRACT)
        .build();

    // Build all
    project.acton().build().run().success();

    // Build only contract1 (should use cache)
    let second = project
        .acton()
        .build()
        .contract("contract1")
        .run()
        .success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert_eq!(order.count(), 0, "Should use cache for filtered build");

    // Modify contract1
    fs::write(
        project.path().join("contracts/contract1.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write contract1");

    // Build contract1 again
    let third = project
        .acton()
        .build()
        .contract("contract1")
        .run()
        .success();
    let order = CompilationOrder::from_stdout(&third.get_normalized_stdout());

    assert_eq!(order.count(), 1, "Should recompile modified contract");
    assert!(order.contains("contract1"));
}

#[test]
fn test_incremental_filtered_with_deps() {
    let project = ProjectBuilder::new("incr-filtered-deps")
        .contract("base", SIMPLE_CONTRACT)
        .contract_with_deps("dependent", SIMPLE_CONTRACT, vec!["base"])
        .contract("independent", SIMPLE_CONTRACT)
        .build();

    // Build all
    project.acton().build().run().success();

    // Modify base
    fs::write(
        project.path().join("contracts/base.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified base
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .expect("Write base");

    // Build only dependent (should also rebuild base)
    let second = project
        .acton()
        .build()
        .contract("dependent")
        .run()
        .success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    order.assert_chain(&["base", "dependent"]);
    assert!(
        !order.contains("independent"),
        "Should NOT build independent"
    );
}

// ========================================
// Multiple Changes Tests
// ========================================

#[test]
fn test_incremental_multiple_changes() {
    let project = ProjectBuilder::new("incr-multi-changes")
        .contract("contract1", SIMPLE_CONTRACT)
        .contract("contract2", SIMPLE_CONTRACT)
        .contract("contract3", SIMPLE_CONTRACT)
        .build();

    project.acton().build().run().success();

    // Modify multiple contracts
    for i in 1..=2 {
        fs::write(
            project.path().join(format!("contracts/contract{i}.tolk")),
            format!(
                r"
                    fun onInternalMessage(in: InMessage) {{
                        // Modified {i}
                    }}
                    fun onBouncedMessage(_: InMessageBounced) {{}}
                "
            ),
        )
        .expect("Write contract");
    }

    let second = project.acton().build().run().success();
    let order = CompilationOrder::from_stdout(&second.get_normalized_stdout());

    assert_eq!(order.count(), 2, "Should recompile both modified contracts");
    order.assert_chain(&["contract1", "contract2"]);
    assert!(
        !order.contains("contract3"),
        "Should NOT recompile contract3"
    );
}
