use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test-manifest-path-works`() {
    expect(1).toEqual(1);
}
"#;

#[test]
fn test_run_specific_test_file() {
    let project = ProjectBuilder::new("multi-file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test1",
            r#"
            import "../../lib/testing/expect"

            get fun `test-in-file-1`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "test2",
            r#"
            import "../../lib/testing/expect"

            get fun `test-in-file-2`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .build();

    // Run only test1.tolk
    project
        .acton()
        .test()
        .path("tests/test1.test.tolk")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("in-file-1")
        .assert_not_contains("in-file-2");
}

#[test]
fn test_filter_by_name() {
    ProjectBuilder::new("filtered")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-unit-1`() {
                expect(1).toEqual(1);
            }

            get fun `test-unit-2`() {
                expect(2).toEqual(2);
            }

            get fun `test-other`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test-unit-.*")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains("unit-1")
        .assert_contains("unit-2")
        .assert_not_contains("other");
}

#[test]
fn test_filter_single_test() {
    ProjectBuilder::new("single-filter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-alpha`() {
                expect(1).toEqual(1);
            }

            get fun `test-beta`() {
                expect(2).toEqual(2);
            }

            get fun `test-gamma`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test-beta")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("beta")
        .assert_not_contains("alpha")
        .assert_not_contains("gamma");
}

#[test]
fn test_combined_path_and_filter() {
    let project = ProjectBuilder::new("path-and-filter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "unit_tests",
            r#"
            import "../../lib/testing/expect"

            get fun `test-unit-counter-test`() {
                expect(1).toEqual(1);
            }

            get fun `test-unit-wallet-test`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .test_file(
            "integration_tests",
            r#"
            import "../../lib/testing/expect"

            get fun `test-integration-counter-test`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build();

    // Run only unit_tests.tolk with counter filter
    project
        .acton()
        .test()
        .path("tests/unit_tests.test.tolk")
        .filter(".*counter.*")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("unit-counter-test")
        .assert_not_contains("unit-wallet-test")
        .assert_not_contains("integration-counter-test");
}

#[test]
fn test_filter_with_no_matches() {
    ProjectBuilder::new("no-match")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-alpha`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("non-existent-test")
        .run()
        .failure()
        .assert_passed(0);
}

#[test]
fn test_fail_fast() {
    let project = ProjectBuilder::new("fail-fast")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test1",
            r#"
            import "../../lib/testing/expect"

            get fun `test-first-pass`() {
                expect(1).toEqual(1);
            }

            get fun `test-second-fail`() {
                expect(1).toEqual(2);
            }

            get fun `test-third-pass`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "test2",
            r#"
            import "../../lib/testing/expect"

            get fun `test-fourth-pass`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build();

    // Without fail-fast: should fail but run all tests
    project
        .acton()
        .test()
        .run()
        .failure() // exit code 1 because of failure
        .assert_passed(3) // first, third, fourth
        .assert_failed(1) // second
        .assert_contains("first-pass")
        .assert_contains("second-fail")
        .assert_contains("third-pass")
        .assert_contains("fourth-pass")
        .assert_snapshot_matches("integration/snapshots/flags/test_without_fail_fast.stdout.txt");

    // With fail-fast: should stop after second test
    project
        .acton()
        .test()
        .fail_fast()
        .run()
        .failure()
        .assert_passed(1) // only first
        .assert_failed(1) // second
        .assert_contains("first-pass")
        .assert_contains("second-fail")
        .assert_not_contains("third-pass")
        .assert_not_contains("fourth-pass")
        .assert_snapshot_matches("integration/snapshots/flags/test_with_fail_fast.stdout.txt");
}

#[test]
fn test_manifest_path_allows_running_outside_project_root() {
    let project = ProjectBuilder::new("manifest-path-outside")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let project_parent = project
        .path()
        .parent()
        .expect("Project should have a parent directory");
    let manifest_path = project.path().join("Acton.toml");
    let manifest_path = manifest_path.to_string_lossy().to_string();

    project
        .acton()
        .check()
        .current_dir(project_parent)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_allows_running_outside_project_root_without_manifest.stderr.txt",
        );

    project
        .acton()
        .arg("--manifest-path")
        .arg(&manifest_path)
        .check()
        .current_dir(project_parent)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_allows_running_outside_project_root_with_manifest.stdout.txt",
        );
}

#[test]
fn test_manifest_path_accepts_project_directory() {
    let project = ProjectBuilder::new("manifest-path-directory")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let project_parent = project
        .path()
        .parent()
        .expect("Project should have a parent directory");
    let manifest_dir = project.path().to_string_lossy().to_string();

    project
        .acton()
        .arg("--manifest-path")
        .arg(&manifest_dir)
        .check()
        .current_dir(project_parent)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_accepts_project_directory.stdout.txt",
        );
}

#[test]
fn test_manifest_path_accepts_relative_path_from_parent() {
    let project = ProjectBuilder::new("manifest-path-relative")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let project_parent = project
        .path()
        .parent()
        .expect("Project should have a parent directory");
    let project_dir_name = project
        .path()
        .file_name()
        .expect("Project directory should have a name")
        .to_string_lossy()
        .to_string();
    let relative_manifest_path = format!("{project_dir_name}/Acton.toml");

    project
        .acton()
        .arg("--manifest-path")
        .arg(&relative_manifest_path)
        .check()
        .current_dir(project_parent)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_accepts_relative_path_from_parent.stdout.txt",
        );
}

#[test]
fn test_manifest_path_missing_file_returns_clear_error() {
    let project = ProjectBuilder::new("manifest-path-missing")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let project_parent = project
        .path()
        .parent()
        .expect("Project should have a parent directory");

    project
        .acton()
        .arg("--manifest-path")
        .arg("missing/Acton.toml")
        .check()
        .current_dir(project_parent)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_missing_file_returns_clear_error.stderr.txt",
        );
}

#[test]
fn test_manifest_path_build_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-path-build-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let output = project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .build()
        .current_dir(&nested_dir)
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_build_works_from_nested_directory.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "build/simple.json",
            "integration/snapshots/flags/test_manifest_path_build_works_from_nested_directory.build_simple_json.txt",
        );
}

#[test]
fn test_manifest_path_check_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-path-check-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .check()
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_check_works_from_nested_directory.stdout.txt",
        );
}

#[test]
fn test_manifest_path_test_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-path-test-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("manifest_path", PASSING_TEST)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .test()
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_path_test_works_from_nested_directory.stdout.txt",
        );
}
