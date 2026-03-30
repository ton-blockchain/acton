use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DO_CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
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
get fun `test-do-stdlib-config-global-version-zero-roundtrip`() {
    var config = net.getConfig();

    val zeroVersion = GlobalVersion {
        version: 0,
        capabilities: 0,
    };

    config.setGlobalVersion(zeroVersion);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getGlobalVersion();
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
get fun `test-do-stdlib-config-global-version-non-zero-roundtrip`() {
    var config = net.getConfig();

    val nonZero = GlobalVersion {
        version: 2026,
        capabilities: 1099511627783,
    };

    config.setGlobalVersion(nonZero);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getGlobalVersion();
    expect(updated.version).toEqual(nonZero.version);
    expect(updated.capabilities).toEqual(nonZero.capabilities);
    expect(updated.version).toNotEqual(0);
    expect(updated.capabilities).toNotEqual(0);
}
",
        "integration/snapshots/test-runner/config_global_version_roundtrip_supports_zero_values/config_global_version_roundtrip_supports_non_zero_values.stdout.txt",
    );
}
