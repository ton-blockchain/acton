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
fn config_set_gas_prices_sequential_chain_updates_remain_isolated() {
    run_config_success_case(
        "ds-stdlib-config-gas-prices-sequential-isolation",
        r"
get fun `test ds stdlib config gas prices sequential isolation`() {
    var firstConfig = net.getConfig();
    val initialBase = firstConfig.getGasPrices(BASECHAIN);
    val initialMaster = firstConfig.getGasPrices(MASTERCHAIN);

    val baseAfterFirstUpdate = GasPrices {
        flatGasLimit: initialBase.flatGasLimit + 11,
        flatGasPrice: initialBase.flatGasPrice + 12,
        other: GasPricesExtended {
            gasPrice: initialBase.other.gasPrice + 13,
            gasLimit: initialBase.other.gasLimit + 14,
            specialGasLimit: initialBase.other.specialGasLimit + 15,
            gasCredit: initialBase.other.gasCredit + 16,
            blockGasLimit: initialBase.other.blockGasLimit + 17,
            freezeDueLimit: initialBase.other.freezeDueLimit + 18,
            deleteDueLimit: initialBase.other.deleteDueLimit + 19,
        },
    };

    firstConfig.setGasPrices(baseAfterFirstUpdate, BASECHAIN);
    expect(net.setConfig(firstConfig)).toBeTrue();

    val afterFirstWrite = net.getConfig();
    val baseAfterFirstWrite = afterFirstWrite.getGasPrices(BASECHAIN);
    val masterAfterFirstWrite = afterFirstWrite.getGasPrices(MASTERCHAIN);
    expect(baseAfterFirstWrite.flatGasPrice).toEqual(baseAfterFirstUpdate.flatGasPrice);
    expect(masterAfterFirstWrite.flatGasPrice).toEqual(initialMaster.flatGasPrice);

    var secondConfig = net.getConfig();
    val baseBeforeSecondWrite = secondConfig.getGasPrices(BASECHAIN);
    val masterBeforeSecondWrite = secondConfig.getGasPrices(MASTERCHAIN);

    val masterAfterSecondUpdate = GasPrices {
        flatGasLimit: masterBeforeSecondWrite.flatGasLimit + 31,
        flatGasPrice: masterBeforeSecondWrite.flatGasPrice + 32,
        other: GasPricesExtended {
            gasPrice: masterBeforeSecondWrite.other.gasPrice + 33,
            gasLimit: masterBeforeSecondWrite.other.gasLimit + 34,
            specialGasLimit: masterBeforeSecondWrite.other.specialGasLimit + 35,
            gasCredit: masterBeforeSecondWrite.other.gasCredit + 36,
            blockGasLimit: masterBeforeSecondWrite.other.blockGasLimit + 37,
            freezeDueLimit: masterBeforeSecondWrite.other.freezeDueLimit + 38,
            deleteDueLimit: masterBeforeSecondWrite.other.deleteDueLimit + 39,
        },
    };

    secondConfig.setGasPrices(masterAfterSecondUpdate, MASTERCHAIN);
    expect(net.setConfig(secondConfig)).toBeTrue();

    val afterSecondWrite = net.getConfig();
    val baseAfterSecondWrite = afterSecondWrite.getGasPrices(BASECHAIN);
    val masterAfterSecondWrite = afterSecondWrite.getGasPrices(MASTERCHAIN);

    expect(baseAfterSecondWrite.flatGasPrice).toEqual(baseBeforeSecondWrite.flatGasPrice);
    expect(baseAfterSecondWrite.other.specialGasLimit).toEqual(baseBeforeSecondWrite.other.specialGasLimit);
    expect(masterAfterSecondWrite.flatGasPrice).toEqual(masterAfterSecondUpdate.flatGasPrice);
    expect(masterAfterSecondWrite.other.specialGasLimit).toEqual(masterAfterSecondUpdate.other.specialGasLimit);

    var thirdConfig = net.getConfig();
    val baseBeforeThirdWrite = thirdConfig.getGasPrices(BASECHAIN);
    val masterBeforeThirdWrite = thirdConfig.getGasPrices(MASTERCHAIN);

    val baseAfterThirdUpdate = GasPrices {
        flatGasLimit: baseBeforeThirdWrite.flatGasLimit + 41,
        flatGasPrice: baseBeforeThirdWrite.flatGasPrice + 42,
        other: GasPricesExtended {
            gasPrice: baseBeforeThirdWrite.other.gasPrice + 43,
            gasLimit: baseBeforeThirdWrite.other.gasLimit + 44,
            specialGasLimit: baseBeforeThirdWrite.other.specialGasLimit + 45,
            gasCredit: baseBeforeThirdWrite.other.gasCredit + 46,
            blockGasLimit: baseBeforeThirdWrite.other.blockGasLimit + 47,
            freezeDueLimit: baseBeforeThirdWrite.other.freezeDueLimit + 48,
            deleteDueLimit: baseBeforeThirdWrite.other.deleteDueLimit + 49,
        },
    };

    thirdConfig.setGasPrices(baseAfterThirdUpdate, BASECHAIN);
    expect(net.setConfig(thirdConfig)).toBeTrue();

    val finalConfig = net.getConfig();
    val finalBase = finalConfig.getGasPrices(BASECHAIN);
    val finalMaster = finalConfig.getGasPrices(MASTERCHAIN);

    expect(finalBase.flatGasLimit).toEqual(baseAfterThirdUpdate.flatGasLimit);
    expect(finalBase.flatGasPrice).toEqual(baseAfterThirdUpdate.flatGasPrice);
    expect(finalBase.other.gasPrice).toEqual(baseAfterThirdUpdate.other.gasPrice);
    expect(finalBase.other.gasLimit).toEqual(baseAfterThirdUpdate.other.gasLimit);
    expect(finalBase.other.specialGasLimit).toEqual(baseAfterThirdUpdate.other.specialGasLimit);
    expect(finalBase.other.gasCredit).toEqual(baseAfterThirdUpdate.other.gasCredit);
    expect(finalBase.other.blockGasLimit).toEqual(baseAfterThirdUpdate.other.blockGasLimit);
    expect(finalBase.other.freezeDueLimit).toEqual(baseAfterThirdUpdate.other.freezeDueLimit);
    expect(finalBase.other.deleteDueLimit).toEqual(baseAfterThirdUpdate.other.deleteDueLimit);

    expect(finalMaster.flatGasLimit).toEqual(masterBeforeThirdWrite.flatGasLimit);
    expect(finalMaster.flatGasPrice).toEqual(masterBeforeThirdWrite.flatGasPrice);
    expect(finalMaster.other.gasPrice).toEqual(masterBeforeThirdWrite.other.gasPrice);
    expect(finalMaster.other.gasLimit).toEqual(masterBeforeThirdWrite.other.gasLimit);
    expect(finalMaster.other.specialGasLimit).toEqual(masterBeforeThirdWrite.other.specialGasLimit);
    expect(finalMaster.other.gasCredit).toEqual(masterBeforeThirdWrite.other.gasCredit);
    expect(finalMaster.other.blockGasLimit).toEqual(masterBeforeThirdWrite.other.blockGasLimit);
    expect(finalMaster.other.freezeDueLimit).toEqual(masterBeforeThirdWrite.other.freezeDueLimit);
    expect(finalMaster.other.deleteDueLimit).toEqual(masterBeforeThirdWrite.other.deleteDueLimit);
}
",
        "integration/snapshots/test-runner/config_set_gas_prices_sequential_chain_updates_remain_isolated/config_set_gas_prices_sequential_chain_updates_remain_isolated.stdout.txt",
    );
}
