use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NET_TEST_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn wrap_test_source(test_body: &str) -> String {
    format!("{NET_TEST_IMPORTS}\n{test_body}\n")
}

fn run_wallet_mode_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = wrap_test_source(test_body);
    ProjectBuilder::new(project_name)
        .test_file("wallet_mode", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn wallet_uses_local_treasury_when_broadcast_disabled() {
    run_wallet_mode_success(
        "s-lib-api-wallet-local-when-not-broadcasting",
        r#"
get fun `test-s-lib-api-wallet-local-when-not-broadcasting`() {
    expect(net.isBroadcasting()).toEqual(false);

    val deployer = net.wallet("deployer");
    val localTreasury = net.treasury("deployer");
    expect(deployer.address).toEqual(localTreasury.address);
}
"#,
        "integration/snapshots/test-runner/api_wallet_mode/lib_api_wallet_uses_local_treasury_when_broadcast_disabled.stdout.txt",
    );
}

#[test]
fn enable_broadcast_wallet_lookup_requires_configured_wallet() {
    let source = wrap_test_source(
        r#"
get fun `test-s-lib-api-enable-broadcast-requires-configured-wallet`() {
    expect(net.isBroadcasting()).toEqual(false);
    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.wallet("deployer");
}
"#,
    );

    ProjectBuilder::new("s-lib-api-enable-broadcast-wallet-lookup")
        .test_file("wallet_mode", &source)
        .build()
        .acton()
        .test()
        .run()
        .code(1)
        .assert_failed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_wallet_mode/lib_api_enable_broadcast_wallet_lookup_requires_configured_wallet.stdout.txt",
        );
}

#[test]
fn disable_broadcast_restores_local_wallet_resolution() {
    run_wallet_mode_success(
        "s-lib-api-disable-broadcast-restores-local-wallet",
        r#"
get fun `test-s-lib-api-disable-broadcast-restores-local-wallet`() {
    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);

    val restored = net.wallet("restored");
    expect(restored.address).toEqual(net.treasury("restored").address);
}
"#,
        "integration/snapshots/test-runner/api_wallet_mode/lib_api_disable_broadcast_restores_local_wallet_resolution.stdout.txt",
    );
}

#[test]
fn broadcast_toggle_roundtrip_updates_mode() {
    run_wallet_mode_success(
        "s-lib-api-broadcast-toggle-roundtrip",
        r#"
get fun `test-s-lib-api-broadcast-toggle-roundtrip`() {
    expect(net.isBroadcasting()).toEqual(false);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);

    net.enableBroadcast();
    net.enableBroadcast();
    expect(net.isBroadcasting()).toEqual(true);

    net.disableBroadcast();
    expect(net.isBroadcasting()).toEqual(false);
}
"#,
        "integration/snapshots/test-runner/api_wallet_mode/lib_api_broadcast_toggle_roundtrip_updates_mode.stdout.txt",
    );
}

#[test]
fn local_wallet_names_map_to_distinct_treasuries() {
    run_wallet_mode_success(
        "s-lib-api-local-wallet-names-distinct",
        r#"
get fun `test-s-lib-api-local-wallet-names-distinct`() {
    val alpha = net.wallet("alpha");
    val beta = net.wallet("beta");

    expect(alpha.address).toNotEqual(beta.address);
    expect(alpha.address).toEqual(net.treasury("alpha").address);
    expect(beta.address).toEqual(net.treasury("beta").address);
}
"#,
        "integration/snapshots/test-runner/api_wallet_mode/lib_api_local_wallet_names_map_to_distinct_treasuries.stdout.txt",
    );
}
