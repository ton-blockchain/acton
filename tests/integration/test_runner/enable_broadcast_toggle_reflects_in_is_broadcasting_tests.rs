use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_network_success(project_name: &str, test_body: &str, snapshot_path: &str) {
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
fn enable_broadcast_toggle_reflects_in_is_broadcasting() {
    run_network_success(
        "bn-stdlib-enable-broadcast-toggle",
        r"
get fun `test-bn-enable-broadcast-toggle`() {
    expect(net.isBroadcasting()).toEqual(false);

    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);
}
",
        "integration/snapshots/test-runner/enable_broadcast_toggle_reflects_in_is_broadcasting/enable_broadcast_toggle_reflects_in_is_broadcasting.stdout.txt",
    );
}

#[test]
fn disable_broadcast_toggle_reflects_in_is_broadcasting() {
    run_network_success(
        "bn-stdlib-disable-broadcast-toggle",
        r"
get fun `test-bn-disable-broadcast-toggle`() {
    expect(net.isBroadcasting()).toEqual(false);

    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);
}
",
        "integration/snapshots/test-runner/enable_broadcast_toggle_reflects_in_is_broadcasting/disable_broadcast_toggle_reflects_in_is_broadcasting.stdout.txt",
    );
}
