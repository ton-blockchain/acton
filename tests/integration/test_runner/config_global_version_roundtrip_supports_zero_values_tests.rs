use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::{append_custom_network, spawn_toncenter_v2_mock_with_capture};

const DO_CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

fn run_config_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DO_CONFIG_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("do_config_global_version", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn config_global_version_roundtrip_supports_zero_values() {
    run_config_success_case(
        "do-stdlib-config-global-version-zero-roundtrip",
        r"
get fun `test do stdlib config global version zero roundtrip`() {
    var config = testing.getConfig();

    val zeroVersion = GlobalVersion {
        version: 0,
        capabilities: 0,
    };

    config.setGlobalVersion(zeroVersion);
    expect(testing.setConfig(config)).toBeTrue();

    val updated = testing.getConfig().getGlobalVersion();
    expect(updated.version).toEqual(0);
    expect(updated.capabilities).toEqual(0);
}
",
        "integration/snapshots/test-runner/config_global_version_roundtrip_supports_zero_values/config_global_version_roundtrip_supports_zero_values.stdout.txt",
    );
}

#[test]
fn config_global_version_roundtrip_supports_non_zero_values() {
    run_config_success_case(
        "do-stdlib-config-global-version-non-zero-roundtrip",
        r"
get fun `test do stdlib config global version non zero roundtrip`() {
    var config = testing.getConfig();

    val nonZero = GlobalVersion {
        version: 2026,
        capabilities: 1099511627783,
    };

    config.setGlobalVersion(nonZero);
    expect(testing.setConfig(config)).toBeTrue();

    val updated = testing.getConfig().getGlobalVersion();
    expect(updated.version).toEqual(nonZero.version);
    expect(updated.capabilities).toEqual(nonZero.capabilities);
    expect(updated.version).toNotEqual(0);
    expect(updated.capabilities).toNotEqual(0);
}
",
        "integration/snapshots/test-runner/config_global_version_roundtrip_supports_zero_values/config_global_version_roundtrip_supports_non_zero_values.stdout.txt",
    );
}

#[allow(clippy::significant_drop_tightening)]
#[test]
fn get_config_stays_local_when_broadcast_flag_enabled_in_test_runner() {
    let (mock_url, mock_handle, captured) = spawn_toncenter_v2_mock_with_capture(vec![]);
    let source = format!(
        "{DO_CONFIG_IMPORTS}\n{}\n",
        r"
get fun `test do stdlib get config stays local with broadcast flag`() {
    var config = testing.getConfig();
    val localVersion = GlobalVersion {
        version: 909090,
        capabilities: 808080,
    };

    config.setGlobalVersion(localVersion);
    expect(testing.setConfig(config)).toBeTrue();

    net.enableBroadcast();
    expect(net.isBroadcasting()).toBeTrue();

    val updated = testing.getConfig().getGlobalVersion();
    expect(updated.version).toEqual(localVersion.version);
    expect(updated.capabilities).toEqual(localVersion.capabilities);
}
"
    );

    let project = ProjectBuilder::new("do-stdlib-get-config-broadcast-flag-local")
        .test_file("do_config_global_version", &source)
        .build();
    append_custom_network(project.path(), "mock-config-unused", &mock_url);

    project
        .acton()
        .test()
        .arg("--fork-net")
        .arg("custom:mock-config-unused")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/config_global_version_roundtrip_supports_zero_values/get_config_stays_local_when_broadcast_flag_enabled_in_test_runner.stdout.txt",
        );

    mock_handle.join().expect("mock toncenter must finish");
    let captured = captured
        .lock()
        .expect("captured toncenter requests mutex poisoned");
    assert_eq!(captured.len(), 0, "acton test must not fetch remote config");
}
