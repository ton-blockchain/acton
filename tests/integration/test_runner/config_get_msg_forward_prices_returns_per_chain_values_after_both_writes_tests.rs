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
fn config_get_msg_forward_prices_returns_per_chain_values_after_both_writes() {
    run_config_success_case(
        "dn-stdlib-config-msg-forward-prices-dual-write",
        r"
get fun `test dn stdlib config msg forward prices dual write`() {
    var config = net.getConfig();
    var basechain = config.getMsgForwardPrices(BASECHAIN);
    var masterchain = config.getMsgForwardPrices(MASTERCHAIN);

    val baseBeforeLump = basechain.lumpPrice;
    val masterBeforeLump = masterchain.lumpPrice;

    basechain.lumpPrice += 111;
    basechain.bitPrice += 7;
    basechain.cellPrice += 13;
    basechain.ihrPriceFactor += 1;
    basechain.firstFrac += 2;
    basechain.nextFrac += 3;

    masterchain.lumpPrice += 222;
    masterchain.bitPrice += 17;
    masterchain.cellPrice += 19;
    masterchain.ihrPriceFactor += 4;
    masterchain.firstFrac += 5;
    masterchain.nextFrac += 6;

    val expectedBaseLump = basechain.lumpPrice;
    val expectedBaseBit = basechain.bitPrice;
    val expectedBaseCell = basechain.cellPrice;
    val expectedBaseIhr = basechain.ihrPriceFactor;
    val expectedBaseFirstFrac = basechain.firstFrac;
    val expectedBaseNextFrac = basechain.nextFrac;

    val expectedMasterLump = masterchain.lumpPrice;
    val expectedMasterBit = masterchain.bitPrice;
    val expectedMasterCell = masterchain.cellPrice;
    val expectedMasterIhr = masterchain.ihrPriceFactor;
    val expectedMasterFirstFrac = masterchain.firstFrac;
    val expectedMasterNextFrac = masterchain.nextFrac;

    config.setMsgForwardPrices(basechain, BASECHAIN);
    config.setMsgForwardPrices(masterchain, MASTERCHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val persisted = net.getConfig();
    val baseAfter = persisted.getMsgForwardPrices(BASECHAIN);
    val masterAfter = persisted.getMsgForwardPrices(MASTERCHAIN);

    expect(baseAfter.lumpPrice).toEqual(expectedBaseLump);
    expect(baseAfter.bitPrice).toEqual(expectedBaseBit);
    expect(baseAfter.cellPrice).toEqual(expectedBaseCell);
    expect(baseAfter.ihrPriceFactor).toEqual(expectedBaseIhr);
    expect(baseAfter.firstFrac).toEqual(expectedBaseFirstFrac);
    expect(baseAfter.nextFrac).toEqual(expectedBaseNextFrac);

    expect(masterAfter.lumpPrice).toEqual(expectedMasterLump);
    expect(masterAfter.bitPrice).toEqual(expectedMasterBit);
    expect(masterAfter.cellPrice).toEqual(expectedMasterCell);
    expect(masterAfter.ihrPriceFactor).toEqual(expectedMasterIhr);
    expect(masterAfter.firstFrac).toEqual(expectedMasterFirstFrac);
    expect(masterAfter.nextFrac).toEqual(expectedMasterNextFrac);

    expect(baseAfter.lumpPrice).toNotEqual(baseBeforeLump);
    expect(masterAfter.lumpPrice).toNotEqual(masterBeforeLump);
    expect(baseAfter.lumpPrice).toNotEqual(expectedMasterLump);
    expect(masterAfter.lumpPrice).toNotEqual(expectedBaseLump);
}
",
        "integration/snapshots/test-runner/config_get_msg_forward_prices_returns_per_chain_values_after_both_writes/config_get_msg_forward_prices_returns_per_chain_values_after_both_writes.stdout.txt",
    );
}
