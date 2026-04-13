use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};
use std::fs;
use std::io::Write;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
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

#[test]
fn test_filter_via_config() {
    ProjectBuilder::new("filter-config")
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
        .with_test_config(TestConfig {
            filter: Some("test unit .*".to_string()),
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_contains("unit 1")
        .assert_contains("unit 2")
        .assert_not_contains("other");
}

#[test]
fn test_coverage_via_config() {
    ProjectBuilder::new("coverage-config")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/math",
            r"
            fun add(a: int, b: int): int {
                return a + b;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/math"

            get fun `test addition`() {
                val result = add(2, 3);
                expect(result).toEqual(5);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: Some(true),
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("math.tolk");
}

#[test]
fn test_backtrace_via_config() {
    ProjectBuilder::new("backtrace-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test with error`() {
                throw 42;
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: Some("full".to_string()),
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("exit_code=42")
        .assert_snapshot_matches("integration/snapshots/test_backtrace_via_config.stdout.txt");
}

#[test]
fn test_filter_and_coverage_via_config() {
    ProjectBuilder::new("filter-coverage-config")
        .contract("simple", SIMPLE_CONTRACT)
        .file(
            "code/utils",
            r"
            @noinline
            fun div(x: int): int {
                return 10 / x;
            }

            fun triple(x: int): int {
                return x * 3;
            }
        ",
        )
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../code/utils"

            get fun `test unit div`() {
                val result = div(0);
                expect(result).toEqual(0);
            }

            get fun `test integration triple`() {
                val result = triple(5);
                expect(result).toEqual(15);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: Some("test unit .*".to_string()),
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: Some(true),
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains(" COVERAGE ")
        .assert_contains("utils.tolk")
        .assert_contains("unit div")
        .assert_not_contains("integration triple")
        .assert_snapshot_matches(
            "integration/snapshots/test_filter_and_coverage_via_config.stdout.txt",
        );
}

#[test]
fn test_cli_overrides_config_filter() {
    let project = ProjectBuilder::new("cli-override")
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
        .with_test_config(TestConfig {
            filter: Some("test alpha".to_string()), // Config says alpha
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build();

    // CLI filter should override config
    project
        .acton()
        .test()
        .filter("test beta") // CLI says beta
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("beta")
        .assert_not_contains("alpha")
        .assert_not_contains("gamma");
}

#[test]
fn test_config_with_specific_path() {
    let project = ProjectBuilder::new("config-path")
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
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build();

    // Path specified in CLI, config should still apply
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
fn test_empty_config() {
    ProjectBuilder::new("empty-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test simple`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1);
}

#[test]
fn test_exclude_patterns_via_config() {
    ProjectBuilder::new("exclude-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "unit_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test unit`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "integration_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test integration`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: Some(vec!["**/integration*".to_string()]),
            include_patterns: None,
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("unit")
        .assert_not_contains("integration");
}

#[test]
fn test_include_patterns_via_config() {
    ProjectBuilder::new("include-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "unit_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test unit`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "integration_test",
            r#"
            import "../../lib/testing/expect"

            get fun `test integration`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: Some(vec!["**/unit*".to_string()]),
            reporters: None,
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("unit")
        .assert_not_contains("integration");
}

#[test]
fn test_include_patterns_via_config_with_explicit_directory_path() {
    ProjectBuilder::new("include-config-explicit-path")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file(
            "tests/selected/path_case.test.tolk",
            r"
            get fun `test folder path`() {}
        ",
        )
        .with_test_config(TestConfig {
            include_patterns: Some(vec!["tests/selected/**".to_string()]),
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .path("tests/selected")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test_include_patterns_via_config_with_explicit_directory_path.stdout.txt",
        );
}

#[test]
fn test_reporters_via_config() {
    ProjectBuilder::new("reporters-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test simple`() {
                expect(1).toEqual(1);
            }
            get fun `test simple1`() {
                expect(1).toEqual(2);
            }
            get fun `test simple2`() {
                expect(1).toEqual(1);
            }
            get fun `test simple3`() {
                expect(1).toEqual(1);
            }
            get fun `test simple4`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: Some(vec!["dot".to_owned()]),
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: None,
            junit_merge: None,
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_contains("·x···");
}

#[test]
fn test_junit_config_via_config() {
    ProjectBuilder::new("junit-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test simple`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .with_test_config(TestConfig {
            filter: None,
            exclude_patterns: None,
            include_patterns: None,
            reporters: Some(vec!["junit".to_owned()]),
            debug: None,
            debug_port: None,
            backtrace: None,
            coverage: None,
            coverage_format: None,
            coverage_file: None,
            junit_path: Some("custom-reports".to_owned()),
            junit_merge: Some(true),
            ..Default::default()
        })
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_file_exists("custom-reports/junit-results.xml");
}

#[test]
fn test_fail_fast_via_config() {
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
        .with_test_config(TestConfig {
            fail_fast: Some(true),
            ..Default::default()
        })
        .build();

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
        .assert_snapshot_matches("integration/snapshots/test_with_fail_fast_via_config.stdout.txt");
}

#[test]
fn test_fail_on_diff_via_config_exits_non_zero_for_profile_drift() {
    let project = ProjectBuilder::new("fail-on-diff-config")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .build();

    let baseline_filename = "profile-baseline.json";

    project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--snapshot")
        .arg(baseline_filename)
        .run()
        .success();

    let mut acton_toml = fs::OpenOptions::new()
        .append(true)
        .open(project.path().join("Acton.toml"))
        .expect("Failed to open Acton.toml");
    writeln!(acton_toml, "\n[test]\nfail-on-diff = true").expect("Failed to append test config");

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
        .run()
        .failure();

    failed
        .assert_contains("CHAIN GAS & FEES SUMMARY COMPARISON")
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test_fail_on_diff_via_config_exits_non_zero_for_profile_drift.stderr.txt",
        );
}

#[test]
fn test_fail_on_diff_via_config_without_baseline_snapshot_mode_succeeds() {
    let project = ProjectBuilder::new("fail-on-diff-config-snapshot-only")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("profile", PROFILED_TEST)
        .with_test_config(TestConfig {
            fail_on_diff: Some(true),
            ..Default::default()
        })
        .build();

    let output = project
        .acton()
        .env("ACTON_LOG_DIR", ".acton/logs")
        .test()
        .arg("--snapshot")
        .arg("profile-baseline.json")
        .run()
        .success();

    output.assert_contains("Gas snapshot saved to profile-baseline.json");
    let stderr = output.get_normalized_stderr();
    assert!(
        !stderr.contains("`--fail-on-diff` requires `--baseline-snapshot`"),
        "snapshot mode with fail-on-diff from config must not require baseline, stderr:\n{stderr}"
    );
}
