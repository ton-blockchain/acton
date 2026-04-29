use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};
use std::fs;
use std::net::TcpListener;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const UI_DEPLOY_TEST_TEMPLATE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"

get fun `__TEST_NAME__`() {
    val init = ContractState {
        code: build("simple"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployer = testing.treasury("deployer");
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

fn append_acton_toml(project: &Project, content: &str) {
    let acton_toml_path = project.path().join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("should read generated Acton.toml");
    acton_toml.push_str(content);
    fs::write(&acton_toml_path, acton_toml).expect("should update generated Acton.toml");
}

#[test]
fn ui_bind_failure_is_reported_before_tests_run_and_skips_default_traces() {
    let project = ProjectBuilder::new("f-ui-default-trace")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-default-trace"))
        .build();

    let (listener, port) = reserve_ui_port();

    let output = project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .run()
        .failure();

    output
        .assert_not_contains("Starting UI server at")
        .assert_stderr_contains("Failed to start UI server on 127.0.0.1:")
        .assert_stderr_contains("Choose another port with --ui-port")
        .assert_stderr_contains("Or stop the process currently listening on that port");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_bind_failure.stderr.txt",
        );
    }

    let trace_dir = project.path().join("build/traces");
    assert!(
        !trace_dir.exists(),
        "UI bind failure must happen before default trace creation: {}",
        trace_dir.display()
    );
}

#[test]
fn ui_port_config_is_used_when_cli_port_is_absent() {
    let project = ProjectBuilder::new("f-ui-config-port")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-config-port"))
        .build();

    let (listener, port) = reserve_ui_port();
    append_acton_toml(
        &project,
        &format!(
            r#"
[test]
ui = true
ui-port = {port}
"#
        ),
    );

    let output = project.acton().test().run().failure();

    output
        .assert_not_contains("Starting UI server at")
        .assert_stderr_contains("Failed to start UI server on 127.0.0.1:")
        .assert_stderr_contains("Choose another port with --ui-port")
        .assert_stderr_contains("Or stop the process currently listening on that port");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_bind_failure.stderr.txt",
        );
    }
}

#[test]
fn ui_bind_failure_skips_custom_trace_output() {
    let project = ProjectBuilder::new("f-ui-custom-trace")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("ui", &ui_deploy_test_source("test-ui-custom-trace"))
        .build();

    let (listener, port) = reserve_ui_port();

    let output = project
        .acton()
        .test()
        .arg("--ui")
        .arg("--ui-port")
        .arg(&port)
        .arg("--save-test-trace")
        .arg("custom-traces")
        .run()
        .failure();

    output
        .assert_stderr_contains("Failed to start UI server on 127.0.0.1:")
        .assert_stderr_contains("Choose another port with --ui-port")
        .assert_stderr_contains("Or stop the process currently listening on that port");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_ui/ui_bind_failure.stderr.txt",
        );
    }

    let default_trace = project.path().join("build/traces");
    assert!(
        !default_trace.exists(),
        "Default UI trace directory must not be created when bind fails: {}",
        default_trace.display()
    );

    let custom_trace = project.path().join("custom-traces");
    assert!(
        !custom_trace.exists(),
        "Custom UI trace directory must not be created when bind fails: {}",
        custom_trace.display()
    );
}
