use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn test_init_create_app_scaffolds_empty_ui_into_app_directory() {
    let project = ProjectBuilder::new("init-create-app")
        .without_acton_toml()
        .build();

    let output = project.acton().init().arg("--create-dapp").run().success();
    output.assert_snapshot_matches(
        "integration/snapshots/create_app/test_init_create_app_scaffolds_empty_ui_into_app_directory.stdout.txt",
    );

    let app_dir = project.path().join("app");
    assert!(!project.path().join("Acton.toml").exists());
    assert!(app_dir.join("package.json").is_file());
    assert!(app_dir.join("package-lock.json").is_file());
    assert!(app_dir.join("vite.config.ts").is_file());
    assert!(app_dir.join("app/src/App.tsx").is_file());
    assert!(app_dir.join("app/src/providers/AppProviders.tsx").is_file());
    assert!(app_dir.join("app/src/styles.css").is_file());
    assert!(app_dir.join(".prettierignore").is_file());
    assert!(!app_dir.join("node_modules").exists());
    assert!(!app_dir.join("dist").exists());
    assert!(!app_dir.join(".idea").exists());

    output.assert_file_snapshot_matches(
        "app/package.json",
        "integration/snapshots/create_app/test_init_create_app_scaffolds_empty_ui_into_app_directory.package.json",
    );
    output.assert_file_snapshot_matches(
        "app/README.md",
        "integration/snapshots/create_app/test_init_create_app_scaffolds_empty_ui_into_app_directory.readme.md",
    );
    output.assert_file_snapshot_matches(
        "app/.github/workflows/ci.yml",
        "integration/snapshots/create_app/test_init_create_app_scaffolds_empty_ui_into_app_directory.ci.yml",
    );
}

#[test]
fn test_init_create_app_scaffolds_empty_ui_into_custom_directory() {
    let project = ProjectBuilder::new("init-create-app-custom")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .init()
        .arg("--create-dapp=frontend")
        .run()
        .success();
    output.assert_snapshot_matches(
        "integration/snapshots/create_app/test_init_create_app_scaffolds_empty_ui_into_custom_directory.stdout.txt",
    );

    let app_dir = project.path().join("frontend");
    assert!(!project.path().join("Acton.toml").exists());
    assert!(app_dir.join("package.json").is_file());
    assert!(app_dir.join("vite.config.ts").is_file());
    assert!(app_dir.join("app/src/App.tsx").is_file());
    assert!(!project.path().join("app").exists());
}

#[test]
fn test_init_create_app_rejects_existing_app_directory() {
    let project = ProjectBuilder::new("init-create-app-existing")
        .without_acton_toml()
        .build();
    fs::create_dir(project.path().join("app")).expect("failed to create existing app dir");

    let output = project.acton().init().arg("--create-dapp").run().failure();
    output.assert_stderr_snapshot_matches(
        "integration/snapshots/create_app/test_init_create_app_rejects_existing_app_directory.stderr.txt",
    );

    assert!(!project.path().join("Acton.toml").exists());
}
