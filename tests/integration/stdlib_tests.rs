use acton::stdlib::ensure_latest;
use std::fs;
use tempfile::TempDir;

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

#[test]
fn test_stdlib_ensure_latest_creates_dir() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // create Acton.toml to simulate project root
    fs::write(
        project_root.join("Acton.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();

    ensure_latest(project_root).expect("Failed to ensure latest stdlib");

    let acton_dir = project_root.join(".acton");
    assert!(acton_dir.exists());
    assert!(acton_dir.join("tolk-stdlib").exists());
    assert!(acton_dir.join(".version").exists());

    let version = fs::read_to_string(acton_dir.join(".version")).unwrap();
    assert_eq!(version.trim(), expected_stdlib_version());

    // check if some standard files exist
    assert!(acton_dir.join("testing/assert.tolk").exists());
}

#[test]
fn test_stdlib_ensure_latest_updates_on_version_mismatch() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    let acton_dir = project_root.join(".acton");

    fs::write(
        project_root.join("Acton.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();
    fs::create_dir_all(&acton_dir).unwrap();

    // write an old version
    fs::write(acton_dir.join(".version"), "0.0.1").unwrap();

    // create a file that should be overwritten or accompanied by new files
    let test_file = acton_dir.join("testing/assert.tolk");
    fs::create_dir_all(test_file.parent().unwrap()).unwrap();
    fs::write(&test_file, "old content").unwrap();

    ensure_latest(project_root).expect("Failed to ensure latest stdlib");

    let version = fs::read_to_string(acton_dir.join(".version")).unwrap();
    assert_eq!(version.trim(), expected_stdlib_version());

    let content = fs::read_to_string(&test_file).unwrap();
    assert_ne!(content, "old content"); // should be overwritten by extract
}

#[test]
fn test_stdlib_ensure_latest_does_nothing_if_no_acton_toml() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    ensure_latest(project_root).expect("Failed to ensure latest stdlib");

    assert!(!project_root.join(".acton").exists());
}

#[test]
fn test_stdlib_ensure_latest_no_update_if_version_matches() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    let acton_dir = project_root.join(".acton");

    fs::write(
        project_root.join("Acton.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();
    fs::create_dir_all(&acton_dir).unwrap();

    let current_version = expected_stdlib_version();
    fs::write(acton_dir.join(".version"), &current_version).unwrap();

    // create a "canary" file that is NOT in the real stdlib
    let canary_path = acton_dir.join("canary.txt");
    fs::write(&canary_path, "i am alive").unwrap();

    // create a file that would be overwritten if update happened
    let test_file = acton_dir.join("testing/assert.tolk");
    fs::create_dir_all(test_file.parent().unwrap()).unwrap();
    fs::write(&test_file, "original content").unwrap();

    ensure_latest(project_root).expect("Failed to ensure latest stdlib");

    // version should remain the same
    let version = fs::read_to_string(acton_dir.join(".version")).unwrap();
    assert_eq!(version.trim(), current_version);

    // content should NOT be overwritten because update was skipped
    let content = fs::read_to_string(&test_file).unwrap();
    assert_eq!(content, "original content");

    // canary should still be there
    assert!(canary_path.exists());
}

#[test]
fn test_stdlib_ensure_latest_updates_legacy_trunk_marker_without_git_hash() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();
    let acton_dir = project_root.join(".acton");

    fs::write(
        project_root.join("Acton.toml"),
        "[package]\nname = \"test\"",
    )
    .unwrap();
    fs::create_dir_all(&acton_dir).unwrap();

    let current_version = expected_stdlib_version();
    let legacy_trunk_version = format!("{}-trunk", env!("CARGO_PKG_VERSION"));
    fs::write(acton_dir.join(".version"), legacy_trunk_version).unwrap();

    let test_file = acton_dir.join("testing/assert.tolk");
    fs::create_dir_all(test_file.parent().unwrap()).unwrap();
    fs::write(&test_file, "old content").unwrap();

    ensure_latest(project_root).expect("Failed to ensure latest stdlib");

    let version = fs::read_to_string(acton_dir.join(".version")).unwrap();
    assert_eq!(version.trim(), current_version);

    let content = fs::read_to_string(&test_file).unwrap();
    assert_ne!(content, "old content");
}
