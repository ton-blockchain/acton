use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_config::config::TestSettings;
use acton_config::test::TestConfig as RunnerTestConfig;
use std::net::TcpListener;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const SIMPLE_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test-debug-smoke`() {
    expect(1).toEqual(1);
}
"#;

fn reserve_debug_port() -> (Option<TcpListener>, String) {
    if let Ok(listener) = TcpListener::bind("127.0.0.1:0") {
        let port = listener
            .local_addr()
            .expect("Reserved TCP port has no address")
            .port()
            .to_string();
        return (Some(listener), port);
    }

    (None, "1".to_string())
}

fn merge_test_config(
    settings: TestSettings,
    debug_override: Option<bool>,
    debug_port_override: Option<u16>,
) -> RunnerTestConfig {
    settings.to_test_config(
        None,
        vec![],
        false,
        debug_override,
        debug_port_override,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        vec![],
        None,
        None,
        false,
        None,
    )
}

#[test]
fn debug_port_without_debug_does_not_start_server() {
    ProjectBuilder::new("g-debug-port-without-debug")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", SIMPLE_TEST)
        .build()
        .acton()
        .test()
        .arg("--debug-port")
        .arg("18182")
        .run()
        .success()
        .assert_passed(1)
        .assert_not_contains("Debugger server listening on")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_debug_port/debug_port_without_debug_does_not_start_server.stdout.txt",
        );
}

#[test]
fn debug_port_rejects_values_outside_u16_range() {
    ProjectBuilder::new("g-debug-port-without-debug")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", SIMPLE_TEST)
        .build()
        .acton()
        .test()
        .arg("--debug")
        .arg("--debug-port")
        .arg("70000")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_debug_port/debug_port_rejects_values_outside_u16_range.stderr.txt",
        );
}

#[test]
fn debug_port_rejects_non_numeric_value() {
    ProjectBuilder::new("g-debug-port-invalid-string")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", SIMPLE_TEST)
        .build()
        .acton()
        .test()
        .arg("--debug-port")
        .arg("not-a-number")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_debug_port/debug_port_rejects_non_numeric_value.stderr.txt",
        );
}

#[test]
fn debug_port_conflict_is_reported_immediately() {
    let project = ProjectBuilder::new("g-debug-port-conflict")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", SIMPLE_TEST)
        .build();

    let (listener, port) = reserve_debug_port();

    let output = project
        .acton()
        .test()
        .arg("--debug")
        .arg("--debug-port")
        .arg(&port)
        .run()
        .failure();

    output.assert_not_contains("Debugger server listening on");

    if listener.is_some() {
        output.assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_debug_port/debug_port_conflict_is_reported_immediately.stderr.txt",
        );
    }
}

#[test]
fn debug_flag_preserves_missing_path_error() {
    ProjectBuilder::new("g-debug-missing-path")
        .contract("simple", SIMPLE_CONTRACT)
        .build()
        .acton()
        .test()
        .arg("--debug")
        .path("missing.test.tolk")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_debug_port/debug_flag_preserves_missing_path_error.stderr.txt",
        );
}

#[test]
fn cli_debug_override_wins_over_config_debug_false() {
    let merged = merge_test_config(
        TestSettings {
            debug: Some(false),
            ..Default::default()
        },
        Some(true),
        None,
    );

    assert!(
        merged.debug,
        "CLI --debug must override Acton.toml debug=false"
    );
}

#[test]
fn cli_default_debug_port_must_not_override_config_debug_port() {
    let merged = merge_test_config(
        TestSettings {
            debug_port: Some(18186),
            ..Default::default()
        },
        None,
        None,
    );

    assert_eq!(merged.debug_port, 18186);
}

#[test]
fn cli_explicit_debug_port_still_overrides_config_debug_port() {
    let merged = merge_test_config(
        TestSettings {
            debug_port: Some(18186),
            ..Default::default()
        },
        None,
        Some(12345),
    );

    assert_eq!(merged.debug_port, 12345);
}
