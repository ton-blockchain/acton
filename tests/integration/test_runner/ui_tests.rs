use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use std::net::TcpListener;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const UI_DEPLOY_TEST_TEMPLATE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/build/build"
import "../../lib/emulation/network"

get fun `__TEST_NAME__`() {
    val init = ContractState {
        code: build("simple"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployer = net.treasury("deployer");
    val msg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: init,
        },
    });

    val txs = net.send(deployer.address, msg);
    expect(txs).toHaveSuccessfulDeploy({ to: address });
}
"#;

fn ui_deploy_test_source(test_name: &str) -> String {
    UI_DEPLOY_TEST_TEMPLATE.replace("__TEST_NAME__", test_name)
}

fn reserve_ui_port() -> (Option<TcpListener>, String) {
    // Preferred mode: occupy an ephemeral localhost port so `acton test --ui`
    // always fails fast with deterministic "address already in use".
    if let Ok(listener) = TcpListener::bind("127.0.0.1:0") {
        let port = listener
            .local_addr()
            .expect("Reserved TCP port has no address")
            .port()
            .to_string();
        return (Some(listener), port);
    }

    // Sandbox fallback: local socket bind may be prohibited.
    // Use a privileged port to keep `--ui` failure deterministic.
    (None, "1".to_string())
}

#[test]
fn ui_creates_default_trace_dir_and_runs_tests_before_bind_failure() {
    let project = ProjectBuilder::new("f-ui-default-trace")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-default-trace"))
        .build();

    let (_listener, port) = reserve_ui_port();

    project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .run()
        .failure()
        .assert_passed(1)
        .assert_file_exists(".acton/traces/test-ui-default-trace_trace.json")
        .assert_file_exists(".acton/traces/contracts/simple.json")
        .assert_file_contains(
            ".acton/traces/test-ui-default-trace_trace.json",
            "\"name\":\"test-ui-default-trace\"",
        )
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_creates_default_trace_dir_and_runs_tests_before_bind_failure.stdout.txt",
        );
}

#[test]
fn ui_save_test_trace_writes_to_custom_directory() {
    let project = ProjectBuilder::new("f-ui-custom-trace")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-custom-trace"))
        .build();

    let (_listener, port) = reserve_ui_port();

    project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .arg("--save-test-trace")
        .arg("custom-traces")
        .run()
        .failure()
        .assert_passed(1)
        .assert_file_exists("custom-traces/test-ui-custom-trace_trace.json")
        .assert_file_exists("custom-traces/contracts/simple.json")
        .assert_file_contains(
            "custom-traces/test-ui-custom-trace_trace.json",
            "\"name\":\"test-ui-custom-trace\"",
        )
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_save_test_trace_writes_to_custom_directory.stdout.txt",
        );

    let default_trace = project
        .path()
        .join(".acton/traces/test-ui-custom-trace_trace.json");
    assert!(
        !default_trace.exists(),
        "Default UI trace path must not be used when --save-test-trace is set: {}",
        default_trace.display()
    );
}

#[test]
fn ui_trace_files_are_created_only_for_executed_tests() {
    let project = ProjectBuilder::new("f-ui-executed-only-traces")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "ui",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            @test("skip")
            get fun `test-ui-skipped`() {
                expect(1).toEqual(2); // should never execute
            }

            @test("todo")
            get fun `test-ui-todo`() {
                expect(1).toEqual(2); // should never execute
            }

            get fun `test-ui-executed`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();
                val deployer = net.treasury("deployer");
                val txs = net.send(
                    deployer.address,
                    createMessage({
                        bounce: false,
                        value: ton("1"),
                        dest: {
                            stateInit: init,
                        },
                    }),
                );
                expect(txs).toHaveSuccessfulDeploy({ to: address });
            }
        "#,
        )
        .build();

    let (_listener, port) = reserve_ui_port();

    project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .run()
        .failure()
        .assert_passed(1)
        .assert_skipped(1)
        .assert_todo(1)
        .assert_file_exists(".acton/traces/test-ui-executed_trace.json")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_trace_files_are_created_only_for_executed_tests.stdout.txt",
        );

    let skipped_trace = project
        .path()
        .join(".acton/traces/test-ui-skipped_trace.json");
    assert!(
        !skipped_trace.exists(),
        "Skipped test trace must not be created: {}",
        skipped_trace.display()
    );

    let todo_trace = project.path().join(".acton/traces/test-ui-todo_trace.json");
    assert!(
        !todo_trace.exists(),
        "Todo test trace must not be created: {}",
        todo_trace.display()
    );
}

#[test]
fn ui_works_with_dot_reporter_without_console_summary() {
    let project = ProjectBuilder::new("f-ui-dot-reporter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-dot-reporter"))
        .build();

    let (_listener, port) = reserve_ui_port();

    project
        .acton()
        .test()
        .with_reporter("dot")
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .run()
        .failure()
        .assert_contains("·")
        .assert_not_contains("✓ 1 passed")
        .assert_file_exists(".acton/traces/test-ui-dot-reporter_trace.json")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_works_with_dot_reporter_without_console_summary.stdout.txt",
        );
}

#[test]
fn ui_filter_limits_trace_generation_to_selected_tests() {
    let project = ProjectBuilder::new("f-ui-filter-traces")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "ui",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/testing/transaction_expect"
            import "../../lib/build/build"
            import "../../lib/emulation/network"

            get fun `test-ui-filter-alpha`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();
                val deployer = net.treasury("deployer");
                val txs = net.send(
                    deployer.address,
                    createMessage({
                        bounce: false,
                        value: ton("1"),
                        dest: {
                            stateInit: init,
                        },
                    }),
                );
                expect(txs).toHaveSuccessfulDeploy({ to: address });
            }

            get fun `test-ui-filter-beta`() {
                val init = ContractState {
                    code: build("simple"),
                    data: createEmptyCell(),
                };
                val address = AutoDeployAddress { stateInit: init }.calculateAddress();
                val deployer = net.treasury("deployer");
                val txs = net.send(
                    deployer.address,
                    createMessage({
                        bounce: false,
                        value: ton("1"),
                        dest: {
                            stateInit: init,
                        },
                    }),
                );
                expect(txs).toHaveSuccessfulDeploy({ to: address });
            }
        "#,
        )
        .build();

    let (_listener, port) = reserve_ui_port();

    project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .filter("test-ui-filter-alpha")
        .run()
        .failure()
        .assert_passed(1)
        .assert_not_contains("filter-beta")
        .assert_file_exists(".acton/traces/test-ui-filter-alpha_trace.json")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_filter_limits_trace_generation_to_selected_tests.stdout.txt",
        );

    let beta_trace = project
        .path()
        .join(".acton/traces/test-ui-filter-beta_trace.json");
    assert!(
        !beta_trace.exists(),
        "Filtered out test trace must not be created: {}",
        beta_trace.display()
    );
}
