use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DT_CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_config_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DT_CONFIG_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("dt_config_precompiled", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn config_get_precompiled_contracts_config_default_decodes_with_builtin_entry() {
    run_config_success_case(
        "dt-stdlib-config-precompiled-default-builtin-entry",
        r#"
get fun `test-dt-stdlib-config-precompiled-default-builtin-entry`() {
    val config = net.getConfig();
    val precompiled = config.getPrecompiledContractsConfig();

    expect(precompiled.list).toHaveLength(1);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_get_precompiled_contracts_config_default_decodes_with_builtin_entry_tests/config_get_precompiled_contracts_config_default_decodes_with_builtin_entry.stdout.txt",
    );
}

#[test]
fn config_get_precompiled_contracts_config_roundtrips_explicit_empty_value() {
    run_config_success_case(
        "dt-stdlib-config-precompiled-explicit-empty",
        r#"
get fun `test-dt-stdlib-config-precompiled-explicit-empty`() {
    var config = net.getConfig();

    val emptyPrecompiled = PrecompiledContractsConfig {
        list: createEmptyMap<uint256, PrecompiledSmartContract>(),
    };
    config.setPrecompiledContractsConfig(emptyPrecompiled);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getPrecompiledContractsConfig();
    expect(updated.list).toHaveLength(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_get_precompiled_contracts_config_default_decodes_with_builtin_entry_tests/config_get_precompiled_contracts_config_roundtrips_explicit_empty_value.stdout.txt",
    );
}
