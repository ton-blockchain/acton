use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const NON_CONTRACT_FILE: &str = r"
fun helper(): int {
    return 42;
}
";

const LOCAL_GLOBAL_WALLETS: &str = r#"[wallets.local]
kind = "v5r1"
keys = { mnemonic = "local-wallet-only" }
"#;

const LOCAL_GLOBAL_LIBRARIES: &str = r#"[libraries.local]
hash = "beef"
code = "te6ccgEBAQEAAgAAAA=="
"#;

fn expected_stdlib_version() -> String {
    if acton::build_info::RELEASE_CHANNEL == "trunk" {
        format!(
            "{}-trunk+{}",
            acton::build_info::PACKAGE_VERSION,
            acton::build_info::GIT_HASH
        )
    } else {
        acton::build_info::PACKAGE_VERSION.to_owned()
    }
}

// ========================================
// Basic Init Tests
// ========================================

#[test]
fn test_init_empty_directory() {
    let project = ProjectBuilder::new("init-empty")
        .without_acton_toml()
        .build();

    let output = project.acton().init().run().success();

    output
        .assert_contains("Initialized new Acton project")
        .assert_contains("Acton.toml")
        .assert_contains("Installing standard library");

    let acton_file = project.path().join("Acton.toml");
    assert!(acton_file.exists());

    let content = fs::read_to_string(&acton_file).expect("Should read Acton.toml file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/init/test_init_empty_directory.toml.gen"),
    );

    assert!(project.path().join(".acton").exists());
    assert!(project.path().join(".acton/tolk-stdlib").exists());
}

#[test]
fn test_init_warns_when_current_directory_is_empty() {
    let project = ProjectBuilder::new("init-truly-empty")
        .without_acton_toml()
        .build();
    fs::remove_dir_all(project.path().join("contracts")).unwrap();
    fs::remove_dir_all(project.path().join("tests")).unwrap();
    let log_dir = project.path().parent().unwrap().join("init-empty-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", log_dir.to_str().unwrap())
        .init()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/init/test_init_warns_when_current_directory_is_empty.stdout.txt",
        );

    assert!(project.path().join("Acton.toml").exists());
    assert!(project.path().join(".acton/tolk-stdlib").exists());
}

#[test]
fn test_init_does_not_warn_for_existing_acton_toml_in_otherwise_empty_directory() {
    let project = ProjectBuilder::new("init-existing-manifest-empty")
        .without_acton_toml()
        .build();
    fs::remove_dir_all(project.path().join("contracts")).unwrap();
    fs::remove_dir_all(project.path().join("tests")).unwrap();
    let original_manifest = "[package]\n";
    fs::write(project.path().join("Acton.toml"), original_manifest).unwrap();
    let log_dir = project
        .path()
        .parent()
        .unwrap()
        .join("init-existing-empty-logs");

    project
        .acton()
        .env("ACTON_LOG_DIR", log_dir.to_str().unwrap())
        .init()
        .run()
        .success()
        .assert_not_contains("For new projects, prefer creating from a template");

    assert_eq!(
        fs::read_to_string(project.path().join("Acton.toml")).unwrap(),
        original_manifest
    );
}

#[test]
fn test_init_already_initialized() {
    let project = ProjectBuilder::new("init-exists")
        .without_acton_toml()
        .build();

    project.acton().init().run().success();

    // Second init should update project
    let output = project.acton().init().run().success();

    output.assert_contains("Updated Acton project");
}

#[test]
fn test_init_leaves_existing_config_with_missing_default_mappings_unchanged() {
    let project = ProjectBuilder::new("init-patch-mappings")
        .without_acton_toml()
        .build();

    let original_manifest = r#"[package]
name = "my-acton-project"
description = "A TON blockchain project"
version = "0.1.0"
license = "MIT"

[fmt]
width = 100
ignore = []

[import-mappings]
tests = "custom-tests"
"#;
    fs::write(project.path().join("Acton.toml"), original_manifest).unwrap();

    let output = project.acton().init().run().success();

    output
        .assert_contains("Updated Acton project")
        .assert_contains("Skipping Acton.toml project configuration")
        .assert_not_contains("Patched Acton.toml with default mappings");

    assert_eq!(
        fs::read_to_string(project.path().join("Acton.toml")).unwrap(),
        original_manifest
    );
}

#[test]
fn test_init_existing_complete_mappings_are_left_unchanged() {
    let project = ProjectBuilder::new("init-complete-mappings")
        .without_acton_toml()
        .build();

    fs::write(
        project.path().join("Acton.toml"),
        r#"[package]
name = "my-acton-project"
description = "A TON blockchain project"
version = "0.1.0"
license = "MIT"

[fmt]
width = 100
ignore = []

[import-mappings]
acton = ".acton"
contracts = "contracts"
gen = "gen"
tests = "tests"
wrappers = "wrappers"
"#,
    )
    .unwrap();

    let output = project.acton().init().run().success();

    output
        .assert_contains("Skipping Acton.toml project configuration")
        .assert_contains("Updated Acton project")
        .assert_not_contains("Patched Acton.toml with default mappings")
        .assert_file_snapshot_matches(
            "Acton.toml",
            "integration/snapshots/init/test_init_existing_complete_mappings_are_left_unchanged.toml.gen",
        );
}

#[test]
fn test_init_updates_stdlib_if_already_initialized() {
    let project = ProjectBuilder::new("init-update-stdlib")
        .without_acton_toml()
        .build();

    // Initialize first time
    project.acton().init().run().success();

    // Delete stdlib
    fs::remove_dir_all(project.path().join(".acton/")).unwrap();
    assert!(!project.path().join(".acton/").exists());

    // Second init should restore stdlib
    let output = project.acton().init().run().success();
    output.assert_contains("Updated Acton project");

    assert!(project.path().join(".acton/tolk-stdlib").exists());
}

#[test]
fn test_init_stdlib_only_updates_without_touching_acton_toml() {
    let original_manifest = "\
# keep this exact file
this is intentionally not valid toml
";
    let project = ProjectBuilder::new("init-stdlib-only-preserve-manifest")
        .without_acton_toml()
        .raw_file("Acton.toml", original_manifest)
        .build();

    let acton_dir = project.path().join(".acton");
    let stale_stdlib_file = acton_dir.join("testing/assert.tolk");
    fs::create_dir_all(stale_stdlib_file.parent().unwrap()).unwrap();
    fs::write(acton_dir.join(".version"), expected_stdlib_version()).unwrap();
    fs::write(&stale_stdlib_file, "stale stdlib content").unwrap();

    let output = project.acton().init().arg("--stdlib-only").run().success();

    output.assert_snapshot_matches(
        "integration/snapshots/init/test_init_stdlib_only_updates_without_touching_acton_toml.stdout.txt",
    );
    assert_eq!(
        fs::read_to_string(project.path().join("Acton.toml")).unwrap(),
        original_manifest
    );
    assert_ne!(
        fs::read_to_string(&stale_stdlib_file).unwrap(),
        "stale stdlib content"
    );
    assert!(!project.path().join(".gitignore").exists());
}

#[test]
fn test_init_stdlib_only_installs_without_acton_toml() {
    let project = ProjectBuilder::new("init-stdlib-only-no-manifest")
        .without_acton_toml()
        .build();

    project
        .acton()
        .init()
        .arg("--stdlib-only")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/init/test_init_stdlib_only_installs_without_acton_toml.stdout.txt",
        );

    assert!(!project.path().join("Acton.toml").exists());
    assert!(project.path().join(".acton/tolk-stdlib").exists());
    assert!(!project.path().join(".gitignore").exists());
}

#[test]
fn test_init_patches_gitignore_if_already_initialized() {
    let project = ProjectBuilder::new("init-update-gitignore")
        .without_acton_toml()
        .build();

    // Initialize first time
    project.acton().init().run().success();

    // Wipe gitignore
    fs::write(project.path().join(".gitignore"), "some-other-file\n").unwrap();

    // Second init should patch gitignore
    let output = project.acton().init().run().success();
    output
        .assert_contains("Updated Acton project")
        .assert_contains("Patched .gitignore");

    let gitignore_content = fs::read_to_string(project.path().join(".gitignore")).unwrap();
    assert!(gitignore_content.contains(".acton/"));
    assert!(gitignore_content.contains("wallets.toml"));
}

#[test]
fn test_init_symlinks_global_wallets_if_already_initialized() {
    let project = ProjectBuilder::new("init-update-symlink")
        .without_acton_toml()
        .build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();
    let global_wallets_dir = home_path.join(".config").join("acton").join("wallets");
    fs::create_dir_all(&global_wallets_dir).unwrap();
    let global_config = global_wallets_dir.join("global.wallets.toml");
    fs::write(
        &global_config,
        "[wallets.global]\nkind=\"v5r1\"\nkeys={mnemonic=\"word1\"}",
    )
    .unwrap();

    // Initialize first time
    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    // Remove symlink
    fs::remove_file(project.path().join("global.wallets.toml")).unwrap();

    // Second init should restore symlink
    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    output.assert_contains("Updated Acton project");
    assert!(project.path().join("global.wallets.toml").exists());
}

#[test]
fn test_init_symlinks_global_libraries_if_already_initialized() {
    let project = ProjectBuilder::new("init-update-libraries-symlink")
        .without_acton_toml()
        .build();

    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();
    let global_libraries_dir = home_path.join(".config").join("acton").join("libraries");
    fs::create_dir_all(&global_libraries_dir).unwrap();
    let global_config = global_libraries_dir.join("global.libraries.toml");
    fs::write(
        &global_config,
        "[libraries.demo]\nhash = \"abcd\"\ncode = \"te6ccgEBAQEAAgAAAA==\"\n",
    )
    .unwrap();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    fs::remove_file(project.path().join("global.libraries.toml")).unwrap();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    output.assert_contains("Updated Acton project");
    let symlink = project.path().join("global.libraries.toml");
    assert!(symlink.exists());
    assert!(
        fs::symlink_metadata(&symlink)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn test_init_with_no_contracts() {
    let project = ProjectBuilder::new("init-no-contracts")
        .without_acton_toml()
        .file("lib/helper", NON_CONTRACT_FILE)
        .build();

    let output = project.acton().init().run().success();

    output.assert_contains("Found no contracts in the current directory");

    let acton_file = project.path().join("Acton.toml");
    assert!(acton_file.exists());

    let content = fs::read_to_string(&acton_file).expect("Should read Acton.toml file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/init/test_init_with_no_contracts.toml.gen"),
    );
}

// ========================================
// Contract Discovery Tests
// ========================================

#[test]
fn test_init_discovers_single_contract() {
    let project = ProjectBuilder::new("init-single")
        .without_acton_toml()
        .contract("my_contract", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().init().run().success();

    output
        .assert_contains("Discovered 1 contract")
        .assert_contains("my_contract");

    let acton_file = project.path().join("Acton.toml");
    assert!(acton_file.exists());

    let content = fs::read_to_string(&acton_file).expect("Should read Acton.toml file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/init/test_init_discovers_single_contract.toml.gen"),
    );
}

#[test]
fn test_init_discovers_multiple_contracts() {
    let project = ProjectBuilder::new("init-multiple")
        .without_acton_toml()
        .contract("contract1", SIMPLE_CONTRACT)
        .contract("contract2", SIMPLE_CONTRACT)
        .contract("contract3", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().init().run().success();

    output
        .assert_contains("Discovered 3 contracts")
        .assert_contains("contract1")
        .assert_contains("contract2")
        .assert_contains("contract3");

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[contracts.contract1]"));
    assert!(acton_toml.contains("[contracts.contract2]"));
    assert!(acton_toml.contains("[contracts.contract3]"));
}

#[test]
fn test_init_ignores_non_contract_files() {
    let project = ProjectBuilder::new("init-mixed")
        .without_acton_toml()
        .contract("my_contract", SIMPLE_CONTRACT)
        .file("lib/helper", NON_CONTRACT_FILE)
        .file("utils/math", NON_CONTRACT_FILE)
        .build();

    let output = project.acton().init().run().success();

    // Should only find the actual contract
    output.assert_contains("Discovered 1 contract");

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[contracts.my_contract]"));
    assert!(!acton_toml.contains("helper"));
    assert!(!acton_toml.contains("math"));
}

#[test]
fn test_init_contract_in_subdirectory() {
    let project = ProjectBuilder::new("init-subdir")
        .without_acton_toml()
        .contract_at("nested", "src/contracts", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().init().run().success();

    output.assert_contains("Discovered 1 contract");

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[contracts.nested]"));
    assert!(acton_toml.contains("src = \"src/contracts/nested.tolk\""));
}

#[test]
fn test_init_ignores_hidden_directories() {
    let project = ProjectBuilder::new("init-hidden")
        .without_acton_toml()
        .contract_at("contract_git", ".git/hooks", SIMPLE_CONTRACT)
        .contract_at("contract_npm", "node_modules", SIMPLE_CONTRACT)
        .contract_at("contract_target", "target", SIMPLE_CONTRACT)
        .contract_at("valid", "contracts", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().init().run().success();

    output.assert_contains("Discovered 1 contract");

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[contracts.valid]"));
}

// ========================================
// Contract Naming Tests
// ========================================

#[test]
fn test_init_contract_name_formatting() {
    let project = ProjectBuilder::new("init-naming")
        .without_acton_toml()
        .contract_at("my_contract", "contracts", SIMPLE_CONTRACT)
        .contract_at("my-contract-v2", "contracts", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().init().run().success();

    output.assert_contains("Discovered 2 contracts");
    output.assert_file_snapshot_matches(
        "Acton.toml",
        "integration/snapshots/init/test_init_contract_name_formatting.toml.gen",
    );
}

#[test]
fn test_init_single_word_contract() {
    let project = ProjectBuilder::new("init-single-word")
        .without_acton_toml()
        .contract_at("wallet", "contracts", SIMPLE_CONTRACT)
        .build();

    project.acton().init().run().success();

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[contracts.wallet]"));
    assert!(acton_toml.contains("name = \"Wallet\""));
}

// ========================================
// Edge Cases Tests
// ========================================

#[test]
fn test_init_with_invalid_tolk_files() {
    let project = ProjectBuilder::new("init-invalid")
        .without_acton_toml()
        .contract_at("broken", "contracts", "this is not valid tolk code {{{")
        .contract_at("valid", "contracts", SIMPLE_CONTRACT)
        .build();

    let output = project.acton().init().run().success();

    output.assert_contains("Discovered 1 contract");

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();
    assert!(acton_toml.contains("[contracts.valid]"));
    assert!(!acton_toml.contains("broken"));
}

#[test]
fn test_init_preserves_existing_acton_directory() {
    let project = ProjectBuilder::new("init-preserve")
        .without_acton_toml()
        .build();

    fs::create_dir_all(project.path().join(".acton/custom")).unwrap();
    fs::write(
        project.path().join(".acton/custom/file.txt"),
        "custom content",
    )
    .unwrap();

    project.acton().init().run().success();

    // custom content should still exist
    assert!(project.path().join(".acton/custom/file.txt").exists());
    let content = fs::read_to_string(project.path().join(".acton/custom/file.txt")).unwrap();
    assert_eq!(content, "custom content");

    assert!(project.path().join(".acton").exists());
    assert!(project.path().join(".acton/tolk-stdlib").exists());
}

// ========================================
// Output Format Tests
// ========================================

#[test]
fn test_init_output_format() {
    let project = ProjectBuilder::new("init-output")
        .without_acton_toml()
        .contract("contract1", SIMPLE_CONTRACT)
        .contract("contract2", SIMPLE_CONTRACT)
        .build();

    project
        .acton()
        .init()
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/init/test_init_output_format.stdout.txt");
}

#[test]
fn test_init_project_symlinks_global_wallets() {
    let project = ProjectBuilder::new("init-symlink")
        .without_acton_toml()
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_wallets_dir = home_path.join(".config").join("acton").join("wallets");
    fs::create_dir_all(&global_wallets_dir).unwrap();
    let global_config = global_wallets_dir.join("global.wallets.toml");
    fs::write(
        &global_config,
        "[wallets.global]\nkind=\"v5r1\"\nkeys={mnemonic=\"word1\"}",
    )
    .unwrap();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    let symlink = project.path().join("global.wallets.toml");
    assert!(symlink.exists());
    assert!(
        fs::symlink_metadata(&symlink)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn test_init_project_symlinks_global_libraries() {
    let project = ProjectBuilder::new("init-symlink-libraries")
        .without_acton_toml()
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_libraries_dir = home_path.join(".config").join("acton").join("libraries");
    fs::create_dir_all(&global_libraries_dir).unwrap();
    let global_config = global_libraries_dir.join("global.libraries.toml");
    fs::write(
        &global_config,
        "[libraries.demo]\nhash = \"abcd\"\ncode = \"te6ccgEBAQEAAgAAAA==\"\n",
    )
    .unwrap();

    project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    let symlink = project.path().join("global.libraries.toml");
    assert!(symlink.exists());
    assert!(
        fs::symlink_metadata(&symlink)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn test_init_preserves_existing_local_global_wallets_file() {
    let project = ProjectBuilder::new("init-preserve-local-global-wallets")
        .without_acton_toml()
        .raw_file("global.wallets.toml", LOCAL_GLOBAL_WALLETS)
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_wallets_dir = home_path.join(".config").join("acton").join("wallets");
    fs::create_dir_all(&global_wallets_dir).unwrap();
    let global_config = global_wallets_dir.join("global.wallets.toml");
    fs::write(
        &global_config,
        "[wallets.global]\nkind=\"v5r1\"\nkeys={mnemonic=\"word1\"}",
    )
    .unwrap();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    let local_file = project.path().join("global.wallets.toml");
    assert!(local_file.exists());
    assert!(
        !fs::symlink_metadata(&local_file)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    output.assert_file_snapshot_matches(
        "global.wallets.toml",
        "integration/snapshots/init/test_init_preserves_existing_local_global_wallets.toml.gen",
    );
}

#[test]
fn test_init_preserves_existing_local_global_libraries_file() {
    let project = ProjectBuilder::new("init-preserve-local-global-libraries")
        .without_acton_toml()
        .raw_file("global.libraries.toml", LOCAL_GLOBAL_LIBRARIES)
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_libraries_dir = home_path.join(".config").join("acton").join("libraries");
    fs::create_dir_all(&global_libraries_dir).unwrap();
    let global_config = global_libraries_dir.join("global.libraries.toml");
    fs::write(
        &global_config,
        "[libraries.demo]\nhash = \"abcd\"\ncode = \"te6ccgEBAQEAAgAAAA==\"\n",
    )
    .unwrap();

    let output = project
        .acton()
        .env("HOME", home_path.to_str().unwrap())
        .init()
        .run()
        .success();

    let local_file = project.path().join("global.libraries.toml");
    assert!(local_file.exists());
    assert!(
        !fs::symlink_metadata(&local_file)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    output.assert_file_snapshot_matches(
        "global.libraries.toml",
        "integration/snapshots/init/test_init_preserves_existing_local_global_libraries.toml.gen",
    );
}

#[test]
fn test_init_patches_gitignore_no_duplicates() {
    let project = ProjectBuilder::new("init-gitignore-duplicates")
        .without_acton_toml()
        .raw_file(".gitignore", ".acton/\nwallets.toml\n")
        .build();

    let output = project.acton().init().run().success();

    let gitignore_path = project.path().join(".gitignore");
    let content = fs::read_to_string(&gitignore_path).unwrap();
    let lines = content.lines().map(str::trim).collect::<Vec<_>>();

    let acton_count = lines.iter().filter(|&&l| l == ".acton/").count();
    assert_eq!(
        acton_count, 1,
        "Should only have one .acton/ entry, found {acton_count}\nContent:\n{content}"
    );

    let wallets_count = lines.iter().filter(|&&l| l == "wallets.toml").count();
    assert_eq!(
        wallets_count, 1,
        "Should only have one wallets.toml entry, found {wallets_count}\nContent:\n{content}"
    );

    output.assert_file_snapshot_matches(
        ".gitignore",
        "integration/snapshots/init/test_init_patches_gitignore_no_duplicates.gitignore",
    );
}

#[test]
fn test_init_creates_gitignore_if_not_exists() {
    let project = ProjectBuilder::new("init-gitignore-create")
        .without_acton_toml()
        .build();

    project.acton().init().run().success();

    let gitignore_path = project.path().join(".gitignore");
    assert!(gitignore_path.exists());
    let content = fs::read_to_string(&gitignore_path).unwrap();

    assert!(content.contains(".acton/"));
    assert!(content.contains("wallets.toml"));
    assert!(content.contains("*.mnemonic"));
    assert!(content.contains("global.wallets.toml"));
}

#[test]
fn test_init_patches_gitignore_adds_only_missing_patterns_per_group() {
    let initial_gitignore = "\
# Acton related files
.acton/
gen/
# Mnemonic and wallet files
wallets.toml
global.wallets.toml
";

    let project = ProjectBuilder::new("init-gitignore-missing-per-group")
        .without_acton_toml()
        .raw_file(".gitignore", initial_gitignore)
        .build();

    let output = project.acton().init().run().success();
    output.assert_contains("Patched .gitignore");

    let content = fs::read_to_string(project.path().join(".gitignore")).unwrap();
    let lines = content.lines().map(str::trim).collect::<Vec<_>>();

    for pattern in [
        ".acton/",
        "gen/",
        "build/",
        "lcov.info",
        "libraries.toml",
        "global.libraries.toml",
        ".env",
        "*.mnemonic",
        "wallets.toml",
        "global.wallets.toml",
    ] {
        let count = lines.iter().filter(|&&line| line == pattern).count();
        assert_eq!(
            count, 1,
            "Pattern {pattern} should appear exactly once, found {count}\nContent:\n{content}"
        );
    }

    for heading in ["# Acton related files", "# Mnemonic and wallet files"] {
        let count = lines.iter().filter(|&&line| line == heading).count();
        assert_eq!(
            count, 1,
            "Heading {heading} should appear exactly once, found {count}\nContent:\n{content}"
        );
    }
}

#[test]
fn test_init_does_not_patch_gitignore_when_groups_are_complete() {
    let initial_gitignore = "\
# Acton related files
.acton/
gen/
build/
lcov.info
libraries.toml
global.libraries.toml

# Mnemonic and wallet files
.env
*.mnemonic
wallets.toml
global.wallets.toml
";

    let project = ProjectBuilder::new("init-gitignore-complete-groups")
        .without_acton_toml()
        .raw_file(".gitignore", initial_gitignore)
        .build();

    let output = project.acton().init().run().success();
    output.assert_not_contains("Patched .gitignore");

    let content = fs::read_to_string(project.path().join(".gitignore")).unwrap();
    assert_eq!(content, initial_gitignore);
}
