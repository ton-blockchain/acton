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
        .test_file("config_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn config_set_msg_forward_prices_keeps_basechain_and_masterchain_isolated() {
    run_config_success_case(
        "be-stdlib-config-msg-forward-prices-branch-isolation",
        r#"
get fun `test-be-stdlib-config-msg-forward-prices-branch-isolation`() {
    var config = net.getConfig();

    var basechain = config.getMsgForwardPrices(BASECHAIN);
    var masterchain = config.getMsgForwardPrices(MASTERCHAIN);

    val baseBeforeLump = basechain.lumpPrice;
    val baseBeforeBit = basechain.bitPrice;
    val baseBeforeCell = basechain.cellPrice;
    val masterBeforeLump = masterchain.lumpPrice;
    val masterBeforeBit = masterchain.bitPrice;
    val masterBeforeCell = masterchain.cellPrice;

    basechain.lumpPrice += 1001;
    basechain.bitPrice += 11;
    basechain.cellPrice += 22;

    val expectedBaseLumpAfterBaseUpdate = basechain.lumpPrice;
    val expectedBaseBitAfterBaseUpdate = basechain.bitPrice;
    val expectedBaseCellAfterBaseUpdate = basechain.cellPrice;

    config.setMsgForwardPrices(basechain, BASECHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val afterBaseUpdate = net.getConfig();
    val baseAfterBaseUpdate = afterBaseUpdate.getMsgForwardPrices(BASECHAIN);
    val masterAfterBaseUpdate = afterBaseUpdate.getMsgForwardPrices(MASTERCHAIN);

    expect(baseAfterBaseUpdate.lumpPrice).toEqual(expectedBaseLumpAfterBaseUpdate);
    expect(baseAfterBaseUpdate.bitPrice).toEqual(expectedBaseBitAfterBaseUpdate);
    expect(baseAfterBaseUpdate.cellPrice).toEqual(expectedBaseCellAfterBaseUpdate);
    expect(masterAfterBaseUpdate.lumpPrice).toEqual(masterBeforeLump);
    expect(masterAfterBaseUpdate.bitPrice).toEqual(masterBeforeBit);
    expect(masterAfterBaseUpdate.cellPrice).toEqual(masterBeforeCell);

    var configMasterUpdate = net.getConfig();
    var masterchainUpdated = configMasterUpdate.getMsgForwardPrices(MASTERCHAIN);

    masterchainUpdated.lumpPrice += 2002;
    masterchainUpdated.bitPrice += 33;
    masterchainUpdated.cellPrice += 44;

    val expectedMasterLumpAfterMasterUpdate = masterchainUpdated.lumpPrice;
    val expectedMasterBitAfterMasterUpdate = masterchainUpdated.bitPrice;
    val expectedMasterCellAfterMasterUpdate = masterchainUpdated.cellPrice;

    configMasterUpdate.setMsgForwardPrices(masterchainUpdated, MASTERCHAIN);
    expect(net.setConfig(configMasterUpdate)).toBeTrue();

    val finalConfig = net.getConfig();
    val baseFinal = finalConfig.getMsgForwardPrices(BASECHAIN);
    val masterFinal = finalConfig.getMsgForwardPrices(MASTERCHAIN);

    expect(baseFinal.lumpPrice).toEqual(expectedBaseLumpAfterBaseUpdate);
    expect(baseFinal.bitPrice).toEqual(expectedBaseBitAfterBaseUpdate);
    expect(baseFinal.cellPrice).toEqual(expectedBaseCellAfterBaseUpdate);
    expect(masterFinal.lumpPrice).toEqual(expectedMasterLumpAfterMasterUpdate);
    expect(masterFinal.bitPrice).toEqual(expectedMasterBitAfterMasterUpdate);
    expect(masterFinal.cellPrice).toEqual(expectedMasterCellAfterMasterUpdate);

    expect(baseFinal.lumpPrice).toNotEqual(baseBeforeLump);
    expect(baseFinal.bitPrice).toNotEqual(baseBeforeBit);
    expect(baseFinal.cellPrice).toNotEqual(baseBeforeCell);
    expect(masterFinal.lumpPrice).toNotEqual(masterBeforeLump);
    expect(masterFinal.bitPrice).toNotEqual(masterBeforeBit);
    expect(masterFinal.cellPrice).toNotEqual(masterBeforeCell);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_set_msg_forward_prices_keeps_basechain_and_masterchain_isolated_tests/config_set_msg_forward_prices_keeps_basechain_and_masterchain_isolated.stdout.txt",
    );
}
