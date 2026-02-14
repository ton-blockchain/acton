use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

#[test]
fn test_new_empty_project_non_interactive() {
    let project = ProjectBuilder::new("new-empty")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Project name: test-project")
        .assert_contains("Template: empty")
        .assert_contains("License: MIT");

    let acton_toml = project.path().join("foobar/Acton.toml");
    assert!(acton_toml.exists());

    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "test-project""#));
    assert!(content.contains(r#"description = "test description""#));
    assert!(content.contains(r#"license = "MIT""#));

    assert!(project.path().join("foobar/contracts").exists());
    assert!(project.path().join("foobar/tests").exists());
    assert!(project.path().join("foobar/LICENSE").exists());
    assert!(project.path().join("foobar/.gitignore").exists());
}

#[test]
fn test_new_counter_project_non_interactive() {
    let project = ProjectBuilder::new("new-counter")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("counter-project")
        .arg("--description")
        .arg("counter description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("Apache-2.0")
        .run()
        .success();

    output.assert_contains("Template: counter");

    let acton_toml = project.path().join("foobar/Acton.toml");
    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "counter-project""#));

    assert!(
        project
            .path()
            .join("foobar/contracts/counter.tolk")
            .exists()
    );
    assert!(content.contains(r"[contracts.counter]"));
}

#[test]
fn test_new_empty_project_in_existed_directory() {
    let project = ProjectBuilder::new("foobar")
        .contract("foo", "")
        .without_acton_toml()
        .build();

    let dir = project.path().parent().expect("Should be parent directory");

    let output = project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure();

    output.assert_stderr_snapshot_matches(
        "integration/snapshots/test_new_empty_project_in_existed_directory.stderr.txt",
    );
}

#[test]
fn test_new_empty_project_in_existed_directory_with_acton_toml() {
    let project = ProjectBuilder::new("foobar").contract("foo", "").build();

    let dir = project.path().parent().expect("Should be parent directory");

    let output = project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .failure();

    output
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_empty_project_in_existed_directory_with_acton_toml.stderr.txt",
        );
}

#[test]
fn test_new_project_with_git_initialization() {
    let project = ProjectBuilder::new("new-git").without_acton_toml().build();

    project
        .acton()
        .arg("new")
        .arg(
            &project
                .path()
                .join("test-git-project")
                .display()
                .to_string(),
        )
        .arg("--name")
        .arg("git-test-project")
        .arg("--description")
        .arg("git test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_contains("Created new Acton project");

    let project_dir = project.path().join("test-git-project");
    assert!(project_dir.join(".git").exists());
}

#[test]
fn test_new_project_symlinks_global_wallets() {
    let project = ProjectBuilder::new("new-symlink")
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
        .arg("new")
        .arg(&project.path().join("my-project").display().to_string())
        .arg("--name")
        .arg("symlink-project")
        .arg("--description")
        .arg("test")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    let symlink = project
        .path()
        .join("my-project")
        .join("global.wallets.toml");
    assert!(symlink.exists());
}

#[test]
fn test_new_empty_project_full_flow() {
    let project = ProjectBuilder::new("new-empty-full")
        .without_acton_toml()
        .build();

    let dir = project.path();
    let project_dir = project.path().join("foobar");

    // 1. Create project
    project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_empty_project_full_flow_check.stderr.txt",
        );
}

#[test]
fn test_new_counter_project_full_flow() {
    let project = ProjectBuilder::new("new-counter-full")
        .without_acton_toml()
        .build();

    let dir = project.path();
    let project_dir = project.path().join("foobar");

    // 1. Create project
    project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("counter")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_counter_project_full_flow_check.stderr.txt",
        );
}

#[test]
fn test_new_jetton_project_full_flow() {
    let project = ProjectBuilder::new("new-jetton-full")
        .without_acton_toml()
        .build();

    let dir = project.path();
    let project_dir = project.path().join("foobar");

    // 1. Create project
    project
        .acton()
        .arg("new")
        .arg(&dir.join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("jetton")
        .arg("--license")
        .arg("MIT")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_new.stdout.txt",
        );

    // 2. Build project
    project
        .acton()
        .current_dir(&project_dir)
        .arg("build")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_build.stdout.txt",
        );

    // 3. Run tests
    project
        .acton()
        .current_dir(&project_dir)
        .arg("test")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_test.stdout.txt",
        );

    // 4. Run deploy script in emulation mode
    project
        .acton()
        .current_dir(&project_dir)
        .arg("script")
        .arg("scripts/deploy.tolk")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_script.stdout.txt",
        );

    // 5. Run linter check
    project
        .acton()
        .current_dir(&project_dir)
        .arg("check")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_new_jetton_project_full_flow_check.stderr.txt",
        );
}

#[test]
fn test_new_empty_project_with_dot_env() {
    let project = ProjectBuilder::new("new-dot-env")
        .without_acton_toml()
        .build();

    let output = project
        .acton()
        .arg("new")
        .arg(&project.path().join("foobar").display().to_string())
        .arg("--name")
        .arg("test-project")
        .arg("--description")
        .arg("test description")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .run()
        .success();

    output
        .assert_contains("Created new Acton project")
        .assert_contains("Project name: test-project")
        .assert_contains("Template: empty")
        .assert_contains("License: MIT");

    let acton_toml = project.path().join("foobar/Acton.toml");
    assert!(acton_toml.exists());

    let content = fs::read_to_string(&acton_toml).unwrap();
    assert!(content.contains(r#"name = "test-project""#));
    assert!(content.contains(r#"description = "test description""#));
    assert!(content.contains(r#"license = "MIT""#));

    assert!(project.path().join("foobar/contracts").exists());
    assert!(project.path().join("foobar/tests").exists());
    assert!(project.path().join("foobar/LICENSE").exists());
    assert!(project.path().join("foobar/.gitignore").exists());
    assert!(project.path().join("foobar/.env").exists());
}
