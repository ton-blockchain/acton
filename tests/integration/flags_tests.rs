use crate::support::TestOutputExt;
use crate::support::compilation::extract_compiled_contracts;
use crate::support::project::ProjectBuilder;
use acton_config::color::ColorMode;
use std::fs;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test manifest path works`() {
    expect(1).toEqual(1);
}
"#;

const UNFORMATTED_FMT_TOLK: &str = r"
fun onInternalMessage(in:InMessage){
val x=1;
}
";

const PROFILED_TEST: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/types/big_array"

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

const PROFILED_TEST_WITH_DRIFT: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/types/big_array"

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

    val secondPing = createMessage({
        bounce: false,
        value: ton("0.2"),
        dest: address,
    });
    val secondPingResult = net.send(deployer.address, secondPing);
    expect(secondPingResult.size()).toEqual(1);
}
"#;

const BUILD_WITH_PROJECT_ROOT_RELATIVE_PATH_TEST: &str = r#"
import "../../lib/build/build"
import "../../lib/testing/expect"

get fun `test build path from project root`() {
    val byName = build("counter");
    val byPath = build("counter", "tests/acton-stdlib/contracts/counter.tolk");
    expect(byPath).toEqual(byName);
}
"#;

const ROOT_TEST_IMPORT: &str = "../../lib/testing/expect";
const NESTED_TEST_IMPORT: &str = "../../../lib/testing/expect";

fn passing_test_file(import_path: &str, test_name: &str, expected: i32) -> String {
    format!(
        r#"
import "{import_path}"

get fun `{test_name}`() {{
    expect({expected}).toEqual({expected});
}}
"#
    )
}

fn body_printing_test_project(project_name: &str) -> ProjectBuilder {
    ProjectBuilder::new(project_name)
        .file(
            "contracts/test_body_messages",
            r"
struct (0xF8000001) TestBodyMsg {
    queryId: uint64
    recipient: address
    amount: coins
}
",
        )
        .contract(
            "test_body_sink",
            r#"
import "test_body_messages"

contract TestBodySink {
    incomingMessages: TestBodyMsg
}

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val _msg = lazy TestBodyMsg.fromSlice(in.body);
}

fun onBouncedMessage(_: InMessageBounced) {}
"#,
        )
        .test_file(
            "print_bodies",
            r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../contracts/test_body_messages"

get fun `test show bodies prints decoded transaction body`() {
    val sender = net.treasury("sender");
    val init = ContractState {
        code: build("test_body_sink"),
        data: createEmptyCell(),
    };
    val sinkAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    }));

    val txs = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("0.1"),
        dest: sinkAddress,
        body: TestBodyMsg {
            queryId: 11,
            recipient: sender.address,
            amount: ton("0.02"),
        },
    }));

    expect(txs).toHaveLength(1);
    println(txs);
}
"#,
        )
}

#[test]
fn test_run_specific_test_file() {
    let project = ProjectBuilder::new("multi-file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test1",
            r#"
            import "../../lib/testing/expect"

            get fun `test in file 1`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "test2",
            r#"
            import "../../lib/testing/expect"

            get fun `test in file 2`() {
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
        .assert_contains("in file 1")
        .assert_not_contains("in file 2");
}

#[test]
fn test_filter_by_name() {
    ProjectBuilder::new("filtered")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test unit 1`() {
                expect(1).toEqual(1);
            }

            get fun `test unit 2`() {
                expect(2).toEqual(2);
            }

            get fun `test other`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test unit .*")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains("unit 1")
        .assert_contains("unit 2")
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

            get fun `test alpha`() {
                expect(1).toEqual(1);
            }

            get fun `test beta`() {
                expect(2).toEqual(2);
            }

            get fun `test gamma`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test beta")
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

            get fun `test unit counter test`() {
                expect(1).toEqual(1);
            }

            get fun `test unit wallet test`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .test_file(
            "integration_tests",
            r#"
            import "../../lib/testing/expect"

            get fun `test integration counter test`() {
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
        .assert_contains("unit counter test")
        .assert_not_contains("unit wallet test")
        .assert_not_contains("integration counter test");
}

#[test]
fn test_filter_with_no_matches() {
    ProjectBuilder::new("no-match")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test alpha`() {
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
fn test_include_flag_filters_test_files() {
    let project = ProjectBuilder::new("include-test-files")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/smoke/alpha.test.tolk",
            &passing_test_file(NESTED_TEST_IMPORT, "test alpha file", 1),
        )
        .raw_file(
            "tests/slow/beta.test.tolk",
            &passing_test_file(NESTED_TEST_IMPORT, "test beta file", 2),
        )
        .build();

    project
        .acton()
        .test()
        .path("tests")
        .include_pattern("tests/smoke/**")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("alpha file")
        .assert_not_contains("beta file")
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_include_flag_filters_test_files.stdout.txt",
        );
}

#[test]
fn test_exclude_flag_filters_test_files() {
    let project = ProjectBuilder::new("exclude-test-files")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/smoke/alpha.test.tolk",
            &passing_test_file(NESTED_TEST_IMPORT, "test alpha file", 1),
        )
        .raw_file(
            "tests/slow/beta.test.tolk",
            &passing_test_file(NESTED_TEST_IMPORT, "test beta file", 2),
        )
        .build();

    project
        .acton()
        .test()
        .path("tests")
        .exclude_pattern("tests/slow/**")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("alpha file")
        .assert_not_contains("beta file")
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_exclude_flag_filters_test_files.stdout.txt",
        );
}

#[test]
fn test_fail_fast() {
    let project = ProjectBuilder::new("fail-fast")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test1",
            r#"
            import "../../lib/testing/expect"

            get fun `test first pass`() {
                expect(1).toEqual(1);
            }

            get fun `test second fail`() {
                expect(1).toEqual(2);
            }

            get fun `test third pass`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "test2",
            r#"
            import "../../lib/testing/expect"

            get fun `test fourth pass`() {
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
        .assert_contains("first pass")
        .assert_contains("second fail")
        .assert_contains("third pass")
        .assert_contains("fourth pass")
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
        .assert_contains("first pass")
        .assert_contains("second fail")
        .assert_not_contains("third pass")
        .assert_not_contains("fourth pass")
        .assert_snapshot_matches("integration/snapshots/flags/test_with_fail_fast.stdout.txt");
}

#[test]
fn test_show_bodies_flag_decodes_transaction_bodies_in_test_output() {
    body_printing_test_project("test-show-bodies-flag")
        .build()
        .acton()
        .test()
        .show_bodies()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_show_bodies_flag_decodes_transaction_bodies_in_test_output.stdout.txt",
        );
}

#[test]
fn test_junit_path_flag_writes_report_to_custom_directory() {
    let project = ProjectBuilder::new("test-junit-path-flag")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            &passing_test_file(ROOT_TEST_IMPORT, "test junit custom path", 1),
        )
        .build();

    let output = project
        .acton()
        .test()
        .with_reporter("junit")
        .arg("--junit-path")
        .arg("custom-reports/nested")
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_junit_path_flag_writes_report_to_custom_directory.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "custom-reports/nested/TEST-test.test.tolk.xml",
            "integration/snapshots/flags/test_junit_path_flag_writes_report_to_custom_directory.xml.gen",
        );

    let default_report = project.path().join("test-results/TEST-test.test.tolk.xml");
    assert!(
        !default_report.exists(),
        "default junit report should not be written when --junit-path is set: {}",
        default_report.display()
    );
}

#[test]
fn test_clear_cache_flag_recompiles_contracts_before_running_tests() {
    let project = ProjectBuilder::new("test-clear-cache-flag")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            &passing_test_file(ROOT_TEST_IMPORT, "test-clear-cache", 1),
        )
        .build();

    let first_run = project.acton().test().run().success();
    let first_compiled = extract_compiled_contracts(&first_run.get_normalized_stdout());
    assert_eq!(
        first_compiled,
        vec!["simple".to_owned()],
        "first test run should compile the contract"
    );

    let cached_run = project.acton().test().run().success();
    let cached_compiled = extract_compiled_contracts(&cached_run.get_normalized_stdout());
    assert!(
        cached_compiled.is_empty(),
        "cached test run should not recompile contracts, got: {cached_compiled:?}"
    );

    let cleared_run = project.acton().test().clear_cache().run().success();
    let cleared_compiled = extract_compiled_contracts(&cleared_run.get_normalized_stdout());
    assert_eq!(
        cleared_compiled,
        vec!["simple".to_owned()],
        "clear-cache test run should recompile the contract"
    );

    cleared_run
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_clear_cache_flag_recompiles_contracts_before_running_tests.stdout.txt",
        );
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
    let project_root = project.path().to_string_lossy().to_string();

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
        .arg("--project-root")
        .arg(&project_root)
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
    let project_root = project.path().to_string_lossy().to_string();

    project
        .acton()
        .arg("--project-root")
        .arg(&project_root)
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
    let relative_project_root = project_dir_name;

    project
        .acton()
        .arg("--project-root")
        .arg(&relative_project_root)
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
fn test_manifest_path_uses_explicit_path_and_keeps_project_root_auto_detection() {
    let project = ProjectBuilder::new("manifest-path-search-ancestors").build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested/deeper");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_wallets = project.path().join("wallets.toml");
    let nested_wallets = nested_dir.join("wallets.toml");
    assert!(
        !root_wallets.exists(),
        "wallets.toml must not exist before wallet command"
    );
    assert!(
        !nested_wallets.exists(),
        "wallets.toml must not exist before wallet command in nested directory"
    );

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .wallet_new()
        .arg("--name")
        .arg("manifest-path-search-up")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_wallets.exists(),
        "wallets.toml must be written in auto-detected project root: {}",
        root_wallets.display()
    );
    assert!(
        !nested_wallets.exists(),
        "wallets.toml must not be written in nested cwd: {}",
        nested_wallets.display()
    );
}

#[test]
fn test_project_root_build_from_nested_directory_snapshot_and_cache() {
    let project = ProjectBuilder::new("project-root-build-from-nested-snapshot-cache")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let output = project
        .acton()
        .arg("--project-root")
        .arg("..")
        .build()
        .current_dir(&nested_dir)
        .run()
        .success();

    output
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_project_root_build_from_nested_directory_snapshot_and_cache.stdout.txt",
        )
        .assert_file_snapshot_matches(
            "build/simple.json",
            "integration/snapshots/flags/test_project_root_build_from_nested_directory_snapshot_and_cache.build_simple_json.txt",
        );

    assert!(
        project.path().join("build/cache").exists(),
        "build cache should be created under project root"
    );
    assert!(
        !nested_dir.join("build/cache").exists(),
        "build cache must not be created under nested working directory"
    );
}

#[test]
fn test_project_root_build_works_from_nested_directory() {
    let project = ProjectBuilder::new("project-root-build-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .build()
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        project.path().join("build/simple.json").exists(),
        "build output should be created under project root"
    );
}

#[test]
fn test_project_root_test_build_extension_resolves_relative_contract_path_from_project_root() {
    let project = ProjectBuilder::new("project-root-build-extension-test")
        .contract("counter", SIMPLE_CONTRACT)
        .raw_file("tests/acton-stdlib/contracts/counter.tolk", SIMPLE_CONTRACT)
        .test_file(
            "build_from_project_root",
            BUILD_WITH_PROJECT_ROOT_RELATIVE_PATH_TEST,
        )
        .build();

    let runner_dir = project.path().join("runner");
    fs::create_dir_all(&runner_dir).expect("Failed to create sibling runner directory");

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .test()
        .current_dir(&runner_dir)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_project_root_test_build_extension_resolves_relative_contract_path_from_project_root.stdout.txt",
        );
}

#[test]
fn test_project_root_full_flow_from_sibling_directory_on_new_project() {
    let workspace = ProjectBuilder::new("project-root-full-flow")
        .without_acton_toml()
        .build();

    let project_dir = workspace.path().join("generated-project");
    let runner_dir = workspace.path().join("runner");
    fs::create_dir_all(&runner_dir).expect("Failed to create sibling runner directory");

    let new_output = workspace
        .acton()
        .arg("new")
        .arg("generated-project")
        .arg("--name")
        .arg("generated-project")
        .arg("--description")
        .arg("Project for --project-root integration flow")
        .arg("--template")
        .arg("empty")
        .arg("--license")
        .arg("MIT")
        .current_dir(workspace.path())
        .run()
        .success();
    new_output.assert_snapshot_matches(
        "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.new.stdout.txt",
    );

    assert!(project_dir.join("Acton.toml").exists());

    let build_output = workspace
        .acton()
        .arg("--project-root")
        .arg("../generated-project")
        .build()
        .current_dir(&runner_dir)
        .run()
        .success();
    build_output.assert_snapshot_matches(
        "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.build.stdout.txt",
    );

    assert!(
        project_dir.join("build/Empty.json").exists(),
        "build output should be created under project root when using --project-root"
    );

    let test_output = workspace
        .acton()
        .arg("--project-root")
        .arg("../generated-project")
        .test()
        .current_dir(&runner_dir)
        .run()
        .success();
    test_output
        .assert_passed(4)
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.test.stdout.txt",
        );

    let script_output = workspace
        .acton()
        .arg("--project-root")
        .arg("../generated-project")
        .script("../generated-project/scripts/deploy.tolk")
        .current_dir(&runner_dir)
        .run()
        .success();
    script_output.assert_snapshot_matches(
        "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.script.stdout.txt",
    );

    let run_output = workspace
        .acton()
        .arg("--project-root")
        .arg("../generated-project")
        .run_script_cmd("deploy-emulation")
        .current_dir(&runner_dir)
        .run()
        .success();
    run_output.assert_snapshot_matches(
        "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.run.stdout.txt",
    );

    let check_output = workspace
        .acton()
        .arg("--project-root")
        .arg("../generated-project")
        .check()
        .current_dir(&runner_dir)
        .run()
        .success();
    check_output.assert_snapshot_matches(
        "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.check.stdout.txt",
    );

    let fmt_output = workspace
        .acton()
        .arg("--project-root")
        .arg("../generated-project")
        .fmt()
        .current_dir(&runner_dir)
        .run()
        .success();
    fmt_output.assert_snapshot_matches(
        "integration/snapshots/flags/test_project_root_full_flow_from_sibling_directory_on_new_project.fmt.stdout.txt",
    );

    assert!(
        project_dir.join("build/cache").exists(),
        "cache should be created under project root"
    );
    assert!(
        !runner_dir.join("build").exists(),
        "sibling runner directory must not receive build artifacts"
    );
}

#[test]
fn test_project_root_check_works_from_nested_directory() {
    let project = ProjectBuilder::new("project-root-check-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .check()
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_project_root_check_works_from_nested_directory.stdout.txt",
        );
}

#[test]
fn test_project_root_test_works_from_nested_directory() {
    let project = ProjectBuilder::new("project-root-test-from-nested")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("manifest_path", PASSING_TEST)
        .build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .test()
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/flags/test_project_root_test_works_from_nested_directory.stdout.txt",
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
        .join("build/traces/test-profiled-transaction_trace.json");
    let nested_trace = nested_dir.join("build/traces/test-profiled-transaction_trace.json");

    project
        .acton()
        .arg("--project-root")
        .arg("..")
        .test()
        .arg("--save-test-trace")
        .current_dir(&nested_dir)
        .run()
        .success()
        .assert_file_exists("build/traces/test-profiled-transaction_trace.json");

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
        .arg("--project-root")
        .arg("..")
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
        .arg("--project-root")
        .arg("..")
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
        .arg("--project-root")
        .arg("..")
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
        "baseline snapshot must be loaded from project root, stderr:\n{stderr}"
    );
}

#[test]
fn test_snapshot_nested_output_creates_parent_directories() {
    let project = ProjectBuilder::new("profiling-snapshot-nested-output")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    let snapshot_path = "build/profiles/profile-baseline.json";

    project
        .acton()
        .test()
        .arg("--snapshot")
        .arg(snapshot_path)
        .run()
        .success()
        .assert_contains("Gas snapshot saved to build/profiles/profile-baseline.json");

    assert!(
        project.path().join(snapshot_path).exists(),
        "snapshot file should be created with missing parent dirs"
    );
}

#[test]
fn test_fail_on_diff_exits_non_zero_for_profile_drift() {
    let project = ProjectBuilder::new("profiling-fail-on-diff")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    let baseline_filename = "profile-baseline.json";
    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--snapshot")
        .arg(baseline_filename)
        .run()
        .success();

    fs::write(
        project.path().join("tests/profile.test.tolk"),
        PROFILED_TEST_WITH_DRIFT,
    )
    .expect("Failed to write drifted test file");

    let failed = project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--baseline-snapshot")
        .arg(baseline_filename)
        .arg("--fail-on-diff")
        .run()
        .failure();

    failed
        .assert_contains("CHAIN GAS & FEES SUMMARY COMPARISON")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_fail_on_diff_exits_non_zero_for_profile_drift.stderr.txt",
        );
}

#[test]
fn test_fail_on_diff_succeeds_when_profile_matches_baseline() {
    let project = ProjectBuilder::new("profiling-fail-on-diff-no-drift")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    let baseline_filename = "profile-baseline.json";
    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--snapshot")
        .arg(baseline_filename)
        .run()
        .success();

    let output = project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--baseline-snapshot")
        .arg(baseline_filename)
        .arg("--fail-on-diff")
        .run()
        .success();

    output.assert_contains("CHAIN GAS & FEES SUMMARY COMPARISON");
    let stderr = output.get_normalized_stderr();
    assert!(
        !stderr.contains("Profiling drift detected"),
        "unexpected drift error in stderr:\n{stderr}"
    );
}

#[test]
fn test_baseline_missing_without_fail_on_diff_warns_and_succeeds() {
    let project = ProjectBuilder::new("profiling-baseline-missing-non-strict")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--baseline-snapshot")
        .arg("missing-baseline.json")
        .run()
        .success()
        .assert_contains("CHAIN GAS & FEES SUMMARY")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_baseline_missing_without_fail_on_diff_warns_and_succeeds.stderr.txt",
        );
}

#[test]
fn test_baseline_missing_with_fail_on_diff_fails() {
    let project = ProjectBuilder::new("profiling-baseline-missing-strict")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--baseline-snapshot")
        .arg("missing-baseline.json")
        .arg("--fail-on-diff")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_baseline_missing_with_fail_on_diff_fails.stderr.txt",
        );
}

#[test]
fn test_baseline_invalid_without_fail_on_diff_warns_and_succeeds() {
    let project = ProjectBuilder::new("profiling-baseline-invalid-non-strict")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    fs::write(project.path().join("invalid-baseline.json"), "{invalid")
        .expect("Failed to write invalid baseline snapshot");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--baseline-snapshot")
        .arg("invalid-baseline.json")
        .run()
        .success()
        .assert_contains("CHAIN GAS & FEES SUMMARY")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_baseline_invalid_without_fail_on_diff_warns_and_succeeds.stderr.txt",
        );
}

#[test]
fn test_baseline_invalid_with_fail_on_diff_fails() {
    let project = ProjectBuilder::new("profiling-baseline-invalid-strict")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();
    project.acton().init().run().success();

    fs::write(project.path().join("invalid-baseline.json"), "{invalid")
        .expect("Failed to write invalid baseline snapshot");

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--baseline-snapshot")
        .arg("invalid-baseline.json")
        .arg("--fail-on-diff")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_baseline_invalid_with_fail_on_diff_fails.stderr.txt",
        );
}

#[test]
fn test_fail_on_diff_without_baseline_is_rejected_by_cli() {
    let project = ProjectBuilder::new("profiling-fail-on-diff-without-baseline")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();

    project
        .acton()
        .test()
        .arg("--fail-on-diff")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/flags/test_fail_on_diff_without_baseline_is_rejected_by_cli.stderr.txt",
        );
}

#[test]
fn test_up_rejects_conflicting_flag_combinations() {
    let project = ProjectBuilder::new("up-conflicting-flags").build();
    let cases: &[(&[&str], &[&str])] = &[
        (&["0.1.0", "--trunk"], &["[VERSION]", "--trunk"]),
        (&["0.1.0", "--stable"], &["[VERSION]", "--stable"]),
        (&["0.1.0", "--list"], &["[VERSION]", "--list"]),
        (&["0.1.0", "--check"], &["[VERSION]", "--check"]),
        (&["--trunk", "--stable"], &["--trunk", "--stable"]),
        (&["--trunk", "--list"], &["--trunk", "--list"]),
        (&["--trunk", "--check"], &["--trunk", "--check"]),
        (&["--stable", "--list"], &["--stable", "--list"]),
        (&["--stable", "--check"], &["--stable", "--check"]),
        (&["--list", "--check"], &["--list", "--check"]),
    ];

    for (args, expected_needles) in cases {
        let mut cmd = project.acton().arg("up");
        for arg in *args {
            cmd = cmd.arg(arg);
        }

        let output = cmd.run().failure();
        output.assert_stderr_contains("cannot be used with");
        for needle in *expected_needles {
            output.assert_stderr_contains(needle);
        }
    }
}

#[test]
fn test_up_rejects_unicode_dash_flag_in_version_argument() {
    let project = ProjectBuilder::new("up-unicode-dash-version").build();

    let output = project
        .acton()
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .arg("up")
        .arg("\u{2014}trunk")
        .run()
        .failure();

    output
        .assert_stderr_contains("looks like an option typed with a Unicode dash")
        .assert_stderr_contains("Use --trunk instead");

    let stderr = output.get_stderr();
    assert!(
        stderr.contains("\u{1b}[33m—trunk") && stderr.contains("\u{1b}[33m--trunk"),
        "Expected yellow highlights for typo and suggested flag, got:\n{stderr}"
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
        .arg("--project-root")
        .arg("..")
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
        .arg("--project-root")
        .arg("..")
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
fn test_manifest_path_wallet_new_local_writes_to_project_root_with_manifest_path() {
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
        .arg("--project-root")
        .arg("..")
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
fn test_manifest_path_wallet_new_global_creates_symlink_in_project_root_with_manifest_path() {
    let project = ProjectBuilder::new("manifest-path-wallet-global-symlink-root").build();
    let home = tempfile::TempDir::new().expect("failed to create temp HOME");
    let home_str = home
        .path()
        .to_str()
        .expect("temp HOME path must be valid UTF-8");
    project.acton().env("HOME", home_str).init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_symlink = project.path().join("global.wallets.toml");
    let nested_symlink = nested_dir.join("global.wallets.toml");
    assert!(
        !root_symlink.exists(),
        "global.wallets.toml symlink must not exist before wallet command"
    );

    project
        .acton()
        .env("HOME", home_str)
        .arg("--project-root")
        .arg("..")
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
fn test_manifest_path_wallet_new_local_writes_to_project_root() {
    let project = ProjectBuilder::new("manifest-path-wallet-local-cwd").build();
    project.acton().init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_wallets = project.path().join("wallets.toml");
    let nested_wallets = nested_dir.join("wallets.toml");

    project
        .acton()
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .wallet_new()
        .arg("--name")
        .arg("manifest-path-local-wallet-cwd")
        .arg("--version")
        .arg("v5r1")
        .arg("--local")
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_wallets.exists(),
        "wallets.toml must be written in auto-detected project root: {}",
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
    let project = ProjectBuilder::new("manifest-path-wallet-global-cwd").build();
    let home = tempfile::TempDir::new().expect("failed to create temp HOME");
    let home_str = home
        .path()
        .to_str()
        .expect("temp HOME path must be valid UTF-8");
    project.acton().env("HOME", home_str).init().run().success();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_symlink = project.path().join("global.wallets.toml");
    let nested_symlink = nested_dir.join("global.wallets.toml");
    assert!(
        !root_symlink.exists(),
        "global.wallets.toml symlink must not exist before wallet command"
    );

    project
        .acton()
        .env("HOME", home_str)
        .arg("--manifest-path")
        .arg("../Acton.toml")
        .wallet_new()
        .arg("--name")
        .arg("manifest-path-global-wallet-cwd")
        .arg("--version")
        .arg("v5r1")
        .arg("--global")
        .current_dir(&nested_dir)
        .run()
        .success();

    assert!(
        root_symlink.exists(),
        "global.wallets.toml symlink must be created in auto-detected project root: {}",
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
