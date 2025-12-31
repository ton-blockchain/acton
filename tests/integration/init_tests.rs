use crate::common::assertion;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output;

use std::fs;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const NON_CONTRACT_FILE: &str = r#"
fun helper(): int {
    return 42;
}
"#;

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
        .assert_contains(".acton/");

    let acton_file = project.path().join("Acton.toml");
    assert!(acton_file.exists());

    let content = fs::read_to_string(&acton_file).expect("Should read Acton.toml file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/test_init_empty_directory.toml.gen"),
    );

    assert!(project.path().join(".acton").exists());
    assert!(project.path().join(".acton/tolk-stdlib").exists());
}

#[test]
fn test_init_already_initialized() {
    let project = ProjectBuilder::new("init-exists")
        .without_acton_toml()
        .build();

    project.acton().init().run().success();

    // Second init should warn
    let output = project.acton().init().run().success();

    output.assert_contains("Acton.toml already exists");
}

#[test]
fn test_init_with_no_contracts() {
    let project = ProjectBuilder::new("init-no-contracts")
        .without_acton_toml()
        .file("lib/helper", NON_CONTRACT_FILE)
        .build();

    let output = project.acton().init().run().success();

    output.assert_contains("No contracts found in the current directory");

    let acton_file = project.path().join("Acton.toml");
    assert!(acton_file.exists());

    let content = fs::read_to_string(&acton_file).expect("Should read Acton.toml file");

    assertion().eq(
        normalize_output(content.as_str(), project.path()),
        snapbox::file!("snapshots/test_init_with_no_contracts.toml.gen"),
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
        snapbox::file!("snapshots/test_init_discovers_single_contract.toml.gen"),
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

    let acton_toml = fs::read_to_string(project.path().join("Acton.toml")).unwrap();

    assert!(acton_toml.contains("[contracts.my_contract]"));
    assert!(acton_toml.contains("name = \"My Contract\""));

    assert!(acton_toml.contains("[contracts.my_contract_v2]"));
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
        .assert_snapshot_matches("integration/snapshots/test_init_output_format.stdout.txt");
}

#[test]
fn test_init_project_symlinks_global_wallets() {
    let project = ProjectBuilder::new("init-symlink")
        .without_acton_toml()
        .build();
    let home_temp = tempfile::TempDir::new().unwrap();
    let home_path = home_temp.path();

    let global_wallets_dir = home_path.join(".acton").join("wallets");
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
}
