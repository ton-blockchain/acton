//! Reserved integration test module for subagent BN.
//!
//! Ownership boundary for agent BN:
//! - tests/integration/test_std_agent_bn_tests.rs
//! - tests/integration/snapshots/test_std_agent_bn/**
//! - tests/integration/testdata/test_std_agent_bn/**
//! - tests/support/test_std_agent_bn/** (optional)
//!
//! Required test name prefix:
//! - bn_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_bn_network_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("broadcast_toggle", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn bn_stdlib_enable_broadcast_toggle_reflects_in_is_broadcasting() {
    run_bn_network_success(
        "bn-stdlib-enable-broadcast-toggle",
        r#"
get fun `test-bn-enable-broadcast-toggle`() {
    expect(net.isBroadcasting()).toEqual(false);

    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);
}
"#,
        "integration/snapshots/test_std_agent_bn/bn_stdlib_enable_broadcast_toggle_reflects_in_is_broadcasting.stdout.txt",
    );
}

#[test]
fn bn_stdlib_disable_broadcast_toggle_reflects_in_is_broadcasting() {
    run_bn_network_success(
        "bn-stdlib-disable-broadcast-toggle",
        r#"
get fun `test-bn-disable-broadcast-toggle`() {
    expect(net.isBroadcasting()).toEqual(false);

    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);
}
"#,
        "integration/snapshots/test_std_agent_bn/bn_stdlib_disable_broadcast_toggle_reflects_in_is_broadcasting.stdout.txt",
    );
}
