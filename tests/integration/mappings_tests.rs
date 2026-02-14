use crate::support::assertions::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn test_mappings_success() {
    let project = ProjectBuilder::new("mappings_success")
        .mapping("@core", "./libs/core")
        .file(
            "libs/core/math",
            "fun add(a: int, b: int): int { return a + b; }",
        )
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
                add(1, 2);
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .success();
}

#[test]
fn test_mappings_missing_file() {
    let project = ProjectBuilder::new("mappings_missing_file")
        .mapping("@core", "./libs/core")
        // math.tolk is NOT created
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/mappings/missing_file.txt");
}

#[test]
fn test_mappings_unknown_prefix() {
    let project = ProjectBuilder::new("mappings_unknown_prefix")
        // @core is NOT mapped
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/mappings/unknown_prefix.txt");
}

#[test]
fn test_mappings_with_subdirectories() {
    let project = ProjectBuilder::new("mappings_subdirs")
        .mapping("@libs", "./libs")
        .file(
            "libs/utils/math",
            "fun add(a: int, b: int): int { return a + b; }",
        )
        .contract(
            "main",
            r#"
            import "@libs/utils/math"

            fun onInternalMessage() {
                add(1, 2);
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .success();
}

#[test]
fn test_mappings_normalization() {
    let project = ProjectBuilder::new("mappings_normalization")
        .mapping("core", "./libs/core") // without @ prefix
        .file(
            "libs/core/math",
            "fun add(a: int, b: int): int { return a + b; }",
        )
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
                add(1, 2);
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .success();
}

#[test]
fn test_mappings_multiple() {
    let project = ProjectBuilder::new("mappings_multiple")
        .mapping("@core", "./libs/core")
        .mapping("@utils", "./libs/utils")
        .file(
            "libs/core/math",
            "fun add(a: int, b: int): int { return a + b; }",
        )
        .file(
            "libs/utils/string",
            "fun get_len(s: int): int { return 42; }",
        )
        .contract(
            "main",
            r#"
            import "@core/math"
            import "@utils/string"

            fun onInternalMessage() {
                add(1, get_len(1));
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .success();
}

#[test]
fn test_mappings_empty_value() {
    let project = ProjectBuilder::new("mappings_empty")
        .mapping("@core", "")
        .file("math", "fun add(a: int, b: int): int { return a + b; }")
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
                add(1, 2);
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .success();
}

#[test]
fn test_mappings_recursive() {
    let project = ProjectBuilder::new("mappings_recursive")
        .mapping("@core", "./libs/core")
        .mapping("@utils", "./libs/utils")
        .file("libs/core/math", "import \"@utils/logic.tolk\"\nfun add_with_1(a: int, b: int): int { return add(a, b) + 1; }")
        .file("libs/utils/logic", "fun add(a: int, b: int): int { return a + b; }")
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
                add_with_1(1, 2);
            }
            "#,
        )
        .build();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .success();
}

#[cfg(unix)]
#[test]
fn test_mappings_symlink_target_rejected() {
    let project = ProjectBuilder::new("mappings_symlink_target")
        .mapping("@core", "./libs/core")
        .contract(
            "main",
            r#"
            import "@core/math"

            fun onInternalMessage() {
                helper();
            }
            "#,
        )
        .build();

    let libs_core = project.path().join("libs/core");
    fs::create_dir_all(&libs_core).unwrap();

    let real_file = libs_core.join("real_math.tolk");
    let symlink_file = libs_core.join("math.tolk");
    fs::write(&real_file, "fun helper() {}").unwrap();
    std::os::unix::fs::symlink(&real_file, &symlink_file).unwrap();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/mappings/symlink_target.txt");
}
