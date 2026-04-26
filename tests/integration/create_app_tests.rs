use crate::common::acton_exe;
use std::fs;
use std::process::Command;

#[test]
fn test_create_app_scaffolds_empty_ui_into_app_directory() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let output = Command::new(acton_exe())
        .args(["--color", "never", "create-app"])
        .current_dir(temp_dir.path())
        .output()
        .expect("failed to run acton create-app");

    assert!(
        output.status.success(),
        "acton create-app failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let app_dir = temp_dir.path().join("app");
    assert!(app_dir.join("package.json").is_file());
    assert!(app_dir.join("package-lock.json").is_file());
    assert!(app_dir.join("vite.config.ts").is_file());
    assert!(app_dir.join("app/src/App.tsx").is_file());
    assert!(app_dir.join("app/src/providers/AppProviders.tsx").is_file());
    assert!(!app_dir.join("node_modules").exists());
    assert!(!app_dir.join("dist").exists());
    assert!(!app_dir.join(".idea").exists());

    let package_json = fs::read_to_string(app_dir.join("package.json"))
        .expect("failed to read generated package.json");
    assert!(package_json.contains("\"name\": \"ton-dapp-template\""));
}

#[test]
fn test_create_app_scaffolds_empty_ui_into_custom_directory() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let output = Command::new(acton_exe())
        .args(["--color", "never", "create-app", "frontend"])
        .current_dir(temp_dir.path())
        .output()
        .expect("failed to run acton create-app frontend");

    assert!(
        output.status.success(),
        "acton create-app frontend failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let app_dir = temp_dir.path().join("frontend");
    assert!(app_dir.join("package.json").is_file());
    assert!(app_dir.join("vite.config.ts").is_file());
    assert!(app_dir.join("app/src/App.tsx").is_file());
    assert!(!temp_dir.path().join("app").exists());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cd frontend"));
}

#[test]
fn test_create_app_rejects_existing_app_directory() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir(temp_dir.path().join("app")).expect("failed to create existing app dir");

    let output = Command::new(acton_exe())
        .args(["--color", "never", "create-app"])
        .current_dir(temp_dir.path())
        .output()
        .expect("failed to run acton create-app");

    assert!(
        !output.status.success(),
        "acton create-app unexpectedly succeeded:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Directory app already exists"));
}
