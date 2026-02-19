use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_config_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CONFIG_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("bb_config_global_version", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn global_version_roundtrip_persists_after_net_set_config() {
    run_config_success_case(
        "bb-stdlib-config-global-version-roundtrip",
        r#"
get fun `test-bb-stdlib-config-global-version-roundtrip`() {
    var config = net.getConfig();
    val before = config.getGlobalVersion();

    val target = GlobalVersion {
        version: 424242,
        capabilities: 4294967313,
    };

    config.setGlobalVersion(target);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getGlobalVersion();
    expect(updated.version).toEqual(target.version);
    expect(updated.capabilities).toEqual(target.capabilities);
    expect(updated.version).toNotEqual(before.version);
    expect(updated.capabilities).toNotEqual(before.capabilities);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_global_version_roundtrip_persists_after_net_set_config_tests/global_version_roundtrip_persists_after_net_set_config.stdout.txt",
    );
}

#[test]
fn global_version_typed_and_raw_reads_match_after_roundtrip() {
    run_config_success_case(
        "bb-stdlib-config-global-version-raw-typed-consistency",
        r#"
get fun `test-bb-stdlib-config-global-version-raw-typed-consistency`() {
    var config = net.getConfig();
    val target = GlobalVersion {
        version: 424243,
        capabilities: 1099511640121,
    };

    config.setGlobalVersion(target);
    expect(net.setConfig(config)).toBeTrue();

    val refreshed = net.getConfig();
    val typed = refreshed.getGlobalVersion();
    val fromRaw = GlobalVersion.fromCell(refreshed.getParamRaw(GLOBAL_VERSION_INDEX));
    val secondRead = net.getConfig().getGlobalVersion();

    expect(typed.version).toEqual(target.version);
    expect(typed.capabilities).toEqual(target.capabilities);
    expect(fromRaw.version).toEqual(target.version);
    expect(fromRaw.capabilities).toEqual(target.capabilities);
    expect(secondRead.version).toEqual(target.version);
    expect(secondRead.capabilities).toEqual(target.capabilities);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_global_version_roundtrip_persists_after_net_set_config_tests/global_version_typed_and_raw_reads_match_after_roundtrip.stdout.txt",
    );
}
