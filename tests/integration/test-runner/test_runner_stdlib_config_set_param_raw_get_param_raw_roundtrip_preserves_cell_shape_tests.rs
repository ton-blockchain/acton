use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DM_CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_config_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DM_CONFIG_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("dm_config_raw_params", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn config_set_param_raw_get_param_raw_roundtrip_preserves_cell_shape() {
    run_config_success_case(
        "dm-stdlib-config-raw-roundtrip",
        r#"
get fun `test-dm-stdlib-config-raw-roundtrip`() {
    var config = net.getConfig();

    val payload = beginCell()
        .storeUint(0xD00DCAFE, 32)
        .storeRef(beginCell().storeInt(-77, 8).endCell())
        .endCell();

    config.setParamRaw(70, payload);

    val roundtrip = config.getParamRaw(70);
    var parsed = roundtrip.beginParse();

    expect(roundtrip).toEqual(payload);
    expect(parsed.loadUint(32)).toEqual(0xD00DCAFE);

    var nested = parsed.loadRef().beginParse();
    expect(nested.loadInt(8)).toEqual(-77);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_set_param_raw_get_param_raw_roundtrip_preserves_cell_shape_tests/config_set_param_raw_get_param_raw_roundtrip_preserves_cell_shape.stdout.txt",
    );
}

#[test]
fn config_set_param_raw_overwrite_keeps_neighbor_slot_unchanged() {
    run_config_success_case(
        "dm-stdlib-config-neighbor-isolation",
        r#"
get fun `test-dm-stdlib-config-neighbor-isolation`() {
    var config = net.getConfig();

    val leftOriginal = beginCell().storeUint(0x11, 8).storeUint(0xAA, 8).endCell();
    val rightOriginal = beginCell().storeUint(0x22, 8).storeUint(0xBB, 8).endCell();
    config.setParamRaw(80, leftOriginal);
    config.setParamRaw(81, rightOriginal);

    val leftOverwrite = beginCell()
        .storeUint(0x33, 8)
        .storeRef(beginCell().storeUint(0xCC, 8).endCell())
        .endCell();
    config.setParamRaw(80, leftOverwrite);

    val leftAfter = config.getParamRaw(80);
    val rightAfter = config.getParamRaw(81);

    expect(leftAfter).toEqual(leftOverwrite);
    expect(leftAfter).toNotEqual(leftOriginal);
    expect(rightAfter).toEqual(rightOriginal);

    var rightSlice = rightAfter.beginParse();
    expect(rightSlice.loadUint(8)).toEqual(0x22);
    expect(rightSlice.loadUint(8)).toEqual(0xBB);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_set_param_raw_get_param_raw_roundtrip_preserves_cell_shape_tests/config_set_param_raw_overwrite_keeps_neighbor_slot_unchanged.stdout.txt",
    );
}
