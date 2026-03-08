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

const UNFORMATTED_FMT_TOLK: &str = r#"
fun onInternalMessage(in:InMessage){
val x=1;
}
"#;

const PROFILED_TEST: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build/build"
import "../../lib/emulation/network"

get fun `test-profiled-transaction`() {
    val init = ContractState {
        code: build("simple"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployer = net.treasury("deployer");
    val deployMessage = createMessage({
        bounce: false,
        value: ton("1.0"),
        dest: {
            stateInit: init,
        },
    });
    val deployResult = net.send(deployer.address, deployMessage);
    expect(deployResult.size()).toEqual(1);

    val ping = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: address,
    });
    val pingResult = net.send(deployer.address, ping);
    expect(pingResult.size()).toEqual(1);
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

#[test]
fn test_manifest_path_test_save_test_trace_default_writes_to_project_root() {
    let project = ProjectBuilder::new("manifest-path-trace-root")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("manifest_path", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_trace = project
        .path()
        .join(".acton/traces/test-profiled-transaction_trace.json");
    let nested_trace = nested_dir.join(".acton/traces/test-profiled-transaction_trace.json");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .test()
        .arg("--save-test-trace")
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_file_exists(".acton/traces/test-profiled-transaction_trace.json");

    assert!(
        root_trace.exists(),
        "trace file must be written in project root: {}",
        root_trace.display()
    );
    assert!(
        !nested_trace.exists(),
        "trace file must not be written in nested cwd: {}",
        nested_trace.display()
    );
}

#[test]
fn test_manifest_path_test_junit_default_writes_to_project_root() {
    let project = ProjectBuilder::new("manifest-path-junit-root")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("manifest_path", PASSING_TEST)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_report = project
        .path()
        .join("test-results/TEST-manifest_path.test.tolk.xml");
    let nested_report = nested_dir.join("test-results/TEST-manifest_path.test.tolk.xml");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .test()
        .with_reporter("junit")
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_file_exists("test-results/TEST-manifest_path.test.tolk.xml");

    assert!(
        root_report.exists(),
        "junit report must be written in project root: {}",
        root_report.display()
    );
    assert!(
        !nested_report.exists(),
        "junit report must not be written in nested cwd: {}",
        nested_report.display()
    );
}

#[test]
fn test_manifest_path_test_profiling_snapshots_use_project_root() {
    let project = ProjectBuilder::new("manifest-path-profiling-root")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("manifest_path", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let baseline_filename = "profile-baseline.json";
    let root_baseline = project.path().join(baseline_filename);
    let nested_baseline = nested_dir.join(baseline_filename);

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .test()
        .arg("--snapshot")
        .arg(baseline_filename)
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_baseline.exists(),
        "snapshot must be written in project root: {}",
        root_baseline.display()
    );
    assert!(
        !nested_baseline.exists(),
        "snapshot must not be written in nested cwd: {}",
        nested_baseline.display()
    );

    let output = project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .test()
        .arg("--baseline-snapshot")
        .arg(baseline_filename)
        .current_dir(&nested_dir)
        .run()
        .success();
    output.assert_contains("Baseline: profile-baseline.json");

    let stderr = output.get_normalized_stderr();
    assert!(
        !stderr.contains("Warning: Failed to load baseline gas snapshot"),
        "baseline snapshot must be loaded from project root, stderr:\n{}",
        stderr
    );
}

#[test]
fn test_manifest_path_fmt_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-path-fmt-from-nested")
        .contract("simple", UNFORMATTED_FMT_TOLK)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let contract_path = project.path().join("contracts/simple.tolk");
    let before = fs::read_to_string(&contract_path).expect("failed to read contract before fmt");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .fmt()
        .current_dir(&nested_dir)
        .run()
        .success();

    let after = fs::read_to_string(&contract_path).expect("failed to read contract after fmt");
    assert_ne!(before, after, "fmt should update file in project root");
    assert!(
        after.contains("in: InMessage"),
        "formatted file should contain normalized spacing in function args"
    );
    assert!(
        after.contains("val x = 1;"),
        "formatted file should contain normalized spacing in assignment"
    );
}

#[test]
fn test_manifest_auto_detect_fmt_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-auto-fmt-from-nested")
        .contract("simple", UNFORMATTED_FMT_TOLK)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let contract_path = project.path().join("contracts/simple.tolk");
    let before = fs::read_to_string(&contract_path).expect("failed to read contract before fmt");

    project
        .acton()
        .fmt()
        .current_dir(&nested_dir)
        .run()
        .success();

    let after = fs::read_to_string(&contract_path).expect("failed to read contract after fmt");
    assert_ne!(before, after, "fmt should update file in project root");
    assert!(
        after.contains("in: InMessage"),
        "formatted file should contain normalized spacing in function args"
    );
    assert!(
        after.contains("val x = 1;"),
        "formatted file should contain normalized spacing in assignment"
    );
}

#[test]
#[cfg_attr(not(unix), ignore)]
fn test_manifest_path_run_works_from_nested_directory_and_uses_project_root() {
    let project = ProjectBuilder::new("manifest-path-run-from-nested")
        .script_config("emit-file", "echo nested > run-root.txt")
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_output = project.path().join("run-root.txt");
    let nested_output = nested_dir.join("run-root.txt");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .run_script_cmd("emit-file")
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_output.exists(),
        "run output must be written in project root: {}",
        root_output.display()
    );
    assert!(
        !nested_output.exists(),
        "run output must not be written in nested cwd: {}",
        nested_output.display()
    );
}

#[test]
fn test_manifest_path_wallet_new_local_writes_to_project_root() {
    let project = ProjectBuilder::new("manifest-path-wallet-local-root").build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_wallets = project.path().join("wallets.toml");
    let nested_wallets = nested_dir.join("wallets.toml");
    assert!(
        !root_wallets.exists(),
        "wallets.toml must not exist before wallet command"
    );

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .wallet_new()
        .arg("--name")
        .arg("manifest-path-local-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_wallets.exists(),
        "wallets.toml must be written in project root: {}",
        root_wallets.display()
    );
    assert!(
        !nested_wallets.exists(),
        "wallets.toml must not be written in nested cwd: {}",
        nested_wallets.display()
    );
}

#[test]
fn test_manifest_path_wallet_new_global_creates_symlink_in_project_root() {
    let project = ProjectBuilder::new("manifest-path-wallet-global-symlink-root").build();
    project.acton().init().run().success();

    let home = tempfile::TempDir::new().expect("failed to create temp HOME");
    let home_str = home
        .path()
        .to_str()
        .expect("temp HOME path must be valid UTF-8");

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_symlink = project.path().join("global.wallets.toml");
    let nested_symlink = nested_dir.join("global.wallets.toml");

    project
        .acton()
        .env("HOME", home_str)
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .wallet_new()
        .arg("--name")
        .arg("manifest-path-global-wallet")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_symlink.exists(),
        "global.wallets.toml symlink must be created in project root: {}",
        root_symlink.display()
    );
    assert!(
        !nested_symlink.exists(),
        "global.wallets.toml symlink must not be created in nested cwd: {}",
        nested_symlink.display()
    );
}

#[test]
fn test_manifest_auto_detect_build_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-auto-build-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let output = project
        .acton()
        .build()
        .current_dir(&nested_dir)
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_auto_detect_build_works_from_nested_directory.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "build/simple.json",
            "integration/snapshots/flags/test_manifest_auto_detect_build_works_from_nested_directory.build_simple_json.txt",
        );
}

#[test]
fn test_manifest_auto_detect_check_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-auto-check-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .check()
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_auto_detect_check_works_from_nested_directory.stdout.txt",
        );
}

#[test]
fn test_manifest_auto_detect_test_works_from_nested_directory() {
    let project = ProjectBuilder::new("manifest-auto-test-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("manifest_path", PASSING_TEST)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .test()
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_manifest_auto_detect_test_works_from_nested_directory.stdout.txt",
        );
}

#[test]
fn test_manifest_auto_detect_stops_at_git_boundary() {
    let project = ProjectBuilder::new("manifest-auto-git-boundary")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let subrepo_dir = project.path().join("subrepo");
    let nested_dir = subrepo_dir.join("nested");
    fs::create_dir_all(subrepo_dir.join(".git")).expect("Failed to create .git boundary");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .check()
        .current_dir(&nested_dir)
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_manifest_auto_detect_stops_at_git_boundary.stderr.txt",
        );
}
