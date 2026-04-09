use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;
use std::path::{Path, PathBuf};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn test_compile_simple_contract() {
    let project = ProjectBuilder::new("compile-simple")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success()
        .assert_contains("Compilation successful")
        .assert_contains("Code in base64")
        .assert_contains("Code in hex")
        .assert_contains("Code hash hex");
}

#[test]
fn test_compile_file_not_found() {
    let project = ProjectBuilder::new("compile-not-found").build();

    project
        .acton()
        .compile("nonexistent.tolk")
        .run()
        .failure()
        .assert_contains("Cannot find file or directory");
}

#[test]
fn test_compile_not_a_file() {
    let project = ProjectBuilder::new("compile-dir")
        .contract("test", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts")
        .run()
        .failure()
        .assert_stderr_contains("is not a file");
}

#[test]
fn test_compile_wrong_extension() {
    let project = ProjectBuilder::new("compile-wrong-ext").build();

    fs::create_dir_all(project.path().join("src")).unwrap();
    fs::write(project.path().join("src/test.txt"), "some content").unwrap();

    project
        .acton()
        .compile("src/test.txt")
        .run()
        .failure()
        .assert_stderr_contains("must end with .tolk");
}

#[test]
fn test_compile_syntax_error() {
    let project = ProjectBuilder::new("compile-syntax")
        .contract("broken", "fun invalid {{{")
        .build();

    project
        .acton()
        .compile("contracts/broken.tolk")
        .run()
        .failure()
        .assert_contains("Error:");
}

#[test]
fn test_compile_undefined_symbol() {
    let project = ProjectBuilder::new("compile-undefined")
        .contract(
            "undefined",
            r"
            fun onInternalMessage(in: InMessage) {
                val x = nonexistent();
            }
            fun onBouncedMessage(_: InMessageBounced) {}
        ",
        )
        .build();

    project
        .acton()
        .compile("contracts/undefined.tolk")
        .run()
        .failure()
        .assert_snapshot_matches("integration/snapshots/test_compile_undefined_symbol.stdout.txt");
}

// ========================================
// Cache Tests
// ========================================

#[test]
fn test_compile_cache_hit() {
    let project = ProjectBuilder::new("compile-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First compilation from source
    let first = project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success();

    first.assert_contains("Compilation successful");

    // Second compilation from cache
    let second = project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success();

    second.assert_contains("Compilation successful (from cache)");
}

#[test]
fn test_compile_cache_invalidation_on_change() {
    let project = ProjectBuilder::new("compile-cache-invalidate")
        .contract("test", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/test.tolk")
        .run()
        .success();

    fs::write(
        project.path().join("contracts/test.tolk"),
        r"
        fun onInternalMessage(in: InMessage) {
            // Modified
        }
        fun onBouncedMessage(_: InMessageBounced) {}
    ",
    )
    .unwrap();

    let output = project
        .acton()
        .compile("contracts/test.tolk")
        .run()
        .success();

    output.assert_not_contains("from cache");
}

#[test]
fn test_compile_with_clear_cache() {
    let project = ProjectBuilder::new("compile-clear")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First compilation
    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success();

    // Second compilation should use cache
    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success()
        .assert_contains("from cache");

    // Third compilation with clear-cache
    project
        .acton()
        .compile("contracts/simple.tolk")
        .clear_cache()
        .run()
        .success()
        .assert_contains("Cache cleared")
        .assert_not_contains("from cache");
}

#[test]
fn test_compile_simple_contract_with_recursive_dependency() {
    let project = ProjectBuilder::new("compile-simple")
        .contract_with_deps("simple1", SIMPLE_CONTRACT, vec!["simple2"])
        .contract_with_deps("simple2", SIMPLE_CONTRACT, vec!["simple1"])
        .build();

    project
        .acton()
        .compile("contracts/simple1.tolk")
        .run()
        .success()
        .assert_contains("Compilation successful")
        .assert_contains("Code in base64")
        .assert_contains("Code in hex")
        .assert_contains("Code hash hex");

    project
        .acton()
        .compile("contracts/simple2.tolk")
        .run()
        .success()
        .assert_contains("Compilation successful")
        .assert_contains("Code in base64")
        .assert_contains("Code in hex")
        .assert_contains("Code hash hex");
}

// ========================================
// Output Format Tests
// ========================================

#[test]
fn test_compile_json_output() {
    let project = ProjectBuilder::new("compile-json")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .compile("contracts/simple.tolk")
        .with_json()
        .run()
        .success();

    let stdout = output.get_stdout();
    assert!(stdout.contains("\"success\": true"));
    assert!(stdout.contains("\"code_boc64\""));
    assert!(stdout.contains("\"code_hex\""));
    assert!(stdout.contains("\"code_hash_hex\""));
}

#[test]
fn test_compile_json_error() {
    let project = ProjectBuilder::new("compile-json-err")
        .contract("broken", "fun invalid {{{")
        .build();

    let output = project
        .acton()
        .compile("contracts/broken.tolk")
        .with_json()
        .run()
        .failure();

    let stdout = output.get_stdout();
    assert!(stdout.contains("\"success\": false"));
    assert!(stdout.contains("\"error\""));
}

#[test]
fn test_compile_base64_only() {
    let project = ProjectBuilder::new("compile-base64")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let output = project
        .acton()
        .compile("contracts/simple.tolk")
        .base64_only()
        .run()
        .success();

    let stdout = output.get_stdout().trim().to_string();

    assert!(!stdout.contains("Compilation successful"));
    assert!(!stdout.contains("Code in base64"));

    #[allow(deprecated)]
    let decoded = base64::decode(&stdout).expect("Decoding failed");
    assert!(!decoded.is_empty());
}

// ========================================
// File Output Tests
// ========================================

#[test]
fn test_compile_with_source_map_output() {
    let project = ProjectBuilder::new("compile-boc-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_source_map("source_map.json")
        .run()
        .success();

    let source_map_file = project.path().join("source_map.json");
    assert!(
        source_map_file.exists(),
        "Source map file should be created"
    );

    let content = fs::read(&source_map_file).unwrap();
    assert!(!content.is_empty(), "Source map file should not be empty");
}

#[test]
fn test_compile_with_source_map_output_to_nonexistent_directory() {
    let project = ProjectBuilder::new("compile-boc-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_source_map("some/dir/here/source_map.json")
        .run()
        .success();

    let source_map_file = project.path().join("some/dir/here/source_map.json");
    assert!(
        source_map_file.exists(),
        "Source map file should be created"
    );

    let content = fs::read(&source_map_file).unwrap();
    assert!(!content.is_empty(), "Source map file should not be empty");
}

#[test]
fn test_compile_with_boc_output() {
    let project = ProjectBuilder::new("compile-boc-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_boc_output("output.boc")
        .run()
        .success();

    let boc_file = project.path().join("output.boc");
    assert!(boc_file.exists(), "BoC file should be created");

    let content = fs::read(&boc_file).unwrap();
    assert!(!content.is_empty(), "BoC file should not be empty");
}

#[test]
fn test_compile_with_boc_output_to_nonexistent_directory() {
    let project = ProjectBuilder::new("compile-boc-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_boc_output("some/dir/here/output.boc")
        .run()
        .success();

    let boc_file = project.path().join("some/dir/here/output.boc");
    assert!(boc_file.exists(), "BoC file should be created");

    let content = fs::read(&boc_file).unwrap();
    assert!(!content.is_empty(), "BoC file should not be empty");
}

#[test]
fn test_compile_with_fift_output() {
    let project = ProjectBuilder::new("compile-fift-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_fift_output("output.fif")
        .run()
        .success();

    let fift_file = project.path().join("output.fif");
    assert!(fift_file.exists(), "Fift file should be created");

    let content = fs::read_to_string(&fift_file).unwrap();
    assert!(!content.is_empty(), "Fift file should not be empty");
}

#[test]
fn test_compile_with_fift_output_to_nonexistent_directory() {
    let project = ProjectBuilder::new("compile-fift-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_fift_output("some/dir/here/output.fif")
        .run()
        .success();

    let fift_file = project.path().join("some/dir/here/output.fif");
    assert!(fift_file.exists(), "Fift file should be created");

    let content = fs::read_to_string(&fift_file).unwrap();
    assert!(!content.is_empty(), "Fift file should not be empty");
}

#[test]
fn test_compile_with_both_outputs() {
    let project = ProjectBuilder::new("compile-both-out")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_boc_output("output.boc")
        .with_fift_output("output.fif")
        .run()
        .success();

    assert!(project.path().join("output.boc").exists());
    assert!(project.path().join("output.fif").exists());
}

#[test]
fn test_compile_empty_path() {
    let project = ProjectBuilder::new("compile-empty-path").build();

    project
        .acton()
        .compile("")
        .run()
        .failure()
        .assert_stderr_snapshot_matches("integration/snapshots/test_compile_empty_path.stderr.txt");
}

#[test]
fn test_compile_file_without_read_permission() {
    let project = ProjectBuilder::new("compile-no-read")
        .contract("secret", SIMPLE_CONTRACT)
        .build();

    // Make the file unreadable (on Unix systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_path = project.path().join("contracts/secret.tolk");
        let mut perms = fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o000); // no permissions
        fs::set_permissions(&file_path, perms).unwrap();
    }

    project
        .acton()
        .compile("contracts/secret.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_file_without_read_permission.stderr.txt",
        );
}

#[cfg(unix)]
#[test]
fn test_compile_import_from_symlink_file() {
    let project = ProjectBuilder::new("compile-import-symlink")
        .contract(
            "main",
            r#"
            import "./linked-lib.tolk";

            fun onInternalMessage(in: InMessage) {
                helper();
            }

            fun onBouncedMessage(_: InMessageBounced) {}
        "#,
        )
        .build();

    let contracts_dir = project.path().join("contracts");
    let real_lib_path = contracts_dir.join("real-lib.tolk");
    let symlink_lib_path = contracts_dir.join("linked-lib.tolk");

    fs::write(&real_lib_path, "fun helper() {}").unwrap();
    std::os::unix::fs::symlink(&real_lib_path, &symlink_lib_path).unwrap();

    project
        .acton()
        .compile("contracts/main.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_import_from_symlink_file.stderr.txt",
        );
}

#[test]
fn test_compile_corrupted_cache_file() {
    let project = ProjectBuilder::new("compile-corrupted-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // First compile to create cache
    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success();

    // Manually corrupt the cache file
    let cache_dir = project.path().join("build/cache");
    if cache_dir.exists() {
        let cache_file = first_cache_json_file(&cache_dir);
        fs::write(&cache_file, "corrupted cache data!!!").unwrap();
    }

    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success()
        .assert_contains("Compilation successful")
        .assert_not_contains("from cache");
}

#[test]
fn test_compile_ignores_unrelated_corrupted_cache_file_and_keeps_it() {
    let project = ProjectBuilder::new("compile-unrelated-corrupted-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    let cache_dir = project.path().join("build/cache");
    fs::create_dir_all(&cache_dir).unwrap();
    let broken_path = cache_dir.join("broken.json");
    fs::write(&broken_path, "not-json").unwrap();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success()
        .assert_contains("Compilation successful");

    assert!(
        broken_path.exists(),
        "Unrelated corrupted cache entry should not be eagerly removed"
    );

    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success()
        .assert_contains("Compilation successful (from cache)");
}

#[test]
fn test_compile_clear_cache_removes_nested_cache_subdirectories() {
    let project = ProjectBuilder::new("compile-clear-cache-removes-subdirs")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .run()
        .success();

    let cache_dir = project.path().join("build/cache");
    let debug_dir = cache_dir.join("debug");
    let nested_dir = cache_dir.join("nested");
    fs::create_dir_all(&debug_dir).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();
    fs::write(debug_dir.join("junk.json"), "junk").unwrap();
    fs::write(nested_dir.join("junk.txt"), "junk").unwrap();

    project
        .acton()
        .compile("contracts/simple.tolk")
        .clear_cache()
        .run()
        .success()
        .assert_contains("Cache cleared");

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
fn test_compile_boc_output_write_error() {
    let project = ProjectBuilder::new("compile-boc-write-err")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // Create a readonly directory
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
        .compile("contracts/simple.tolk")
        .with_boc_output("readonly/output.boc")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_boc_output_write_error.stderr.txt",
        );
}

#[test]
fn test_compile_fift_output_write_error() {
    let project = ProjectBuilder::new("compile-fift-write-err")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // Create a readonly directory
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
        .compile("contracts/simple.tolk")
        .with_fift_output("readonly/output.fif")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_fift_output_write_error.stderr.txt",
        );
}

#[test]
fn test_compile_source_map_write_error() {
    let project = ProjectBuilder::new("compile-sourcemap-write-err")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // Create a readonly directory
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
        .compile("contracts/simple.tolk")
        .with_source_map("readonly/sourcemap.json")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_source_map_write_error.stderr.txt",
        );
}

#[test]
fn test_compile_invalid_boc_output_path() {
    let project = ProjectBuilder::new("compile-invalid-boc-path")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // Create a directory where we want to create boc file
    fs::create_dir(project.path().join("output.boc")).expect("Create dir blocking boc output");

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_boc_output("output.boc")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_invalid_boc_output_path.stderr.txt",
        );
}

#[test]
fn test_compile_invalid_fift_output_path() {
    let project = ProjectBuilder::new("compile-invalid-fift-path")
        .contract("simple", SIMPLE_CONTRACT)
        .build();

    // Create a directory where we want to create fift file
    fs::create_dir(project.path().join("output.fif")).expect("Create dir blocking fift output");

    project
        .acton()
        .compile("contracts/simple.tolk")
        .with_fift_output("output.fif")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_compile_invalid_fift_output_path.stderr.txt",
        );
}
