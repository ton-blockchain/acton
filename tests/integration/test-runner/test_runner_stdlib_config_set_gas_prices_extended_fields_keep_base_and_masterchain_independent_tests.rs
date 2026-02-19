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
fn config_set_gas_prices_extended_fields_keep_base_and_masterchain_independent() {
    run_config_success_case(
        "bd-stdlib-config-gas-prices-extended-dual-chain-independence",
        r#"
get fun `test-bd-stdlib-config-gas-prices-extended-dual-chain-independence`() {
    var config = net.getConfig();

    val baseBefore = config.getGasPrices(BASECHAIN);
    val masterBefore = config.getGasPrices(MASTERCHAIN);

    val baseAfterUpdate = GasPrices {
        flatGasLimit: baseBefore.flatGasLimit + 101,
        flatGasPrice: baseBefore.flatGasPrice + 102,
        other: GasPricesExtended {
            gasPrice: baseBefore.other.gasPrice + 103,
            gasLimit: baseBefore.other.gasLimit + 104,
            specialGasLimit: baseBefore.other.specialGasLimit + 105,
            gasCredit: baseBefore.other.gasCredit + 106,
            blockGasLimit: baseBefore.other.blockGasLimit + 107,
            freezeDueLimit: baseBefore.other.freezeDueLimit + 108,
            deleteDueLimit: baseBefore.other.deleteDueLimit + 109,
        },
    };
    val masterAfterUpdate = GasPrices {
        flatGasLimit: masterBefore.flatGasLimit + 201,
        flatGasPrice: masterBefore.flatGasPrice + 202,
        other: GasPricesExtended {
            gasPrice: masterBefore.other.gasPrice + 203,
            gasLimit: masterBefore.other.gasLimit + 204,
            specialGasLimit: masterBefore.other.specialGasLimit + 205,
            gasCredit: masterBefore.other.gasCredit + 206,
            blockGasLimit: masterBefore.other.blockGasLimit + 207,
            freezeDueLimit: masterBefore.other.freezeDueLimit + 208,
            deleteDueLimit: masterBefore.other.deleteDueLimit + 209,
        },
    };

    config.setGasPrices(baseAfterUpdate, BASECHAIN);
    config.setGasPrices(masterAfterUpdate, MASTERCHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig();
    val actualBase = updated.getGasPrices(BASECHAIN);
    val actualMaster = updated.getGasPrices(MASTERCHAIN);

    expect(actualBase.flatGasLimit).toEqual(baseAfterUpdate.flatGasLimit);
    expect(actualBase.flatGasPrice).toEqual(baseAfterUpdate.flatGasPrice);
    expect(actualBase.other.gasPrice).toEqual(baseAfterUpdate.other.gasPrice);
    expect(actualBase.other.gasLimit).toEqual(baseAfterUpdate.other.gasLimit);
    expect(actualBase.other.specialGasLimit).toEqual(baseAfterUpdate.other.specialGasLimit);
    expect(actualBase.other.gasCredit).toEqual(baseAfterUpdate.other.gasCredit);
    expect(actualBase.other.blockGasLimit).toEqual(baseAfterUpdate.other.blockGasLimit);
    expect(actualBase.other.freezeDueLimit).toEqual(baseAfterUpdate.other.freezeDueLimit);
    expect(actualBase.other.deleteDueLimit).toEqual(baseAfterUpdate.other.deleteDueLimit);

    expect(actualMaster.flatGasLimit).toEqual(masterAfterUpdate.flatGasLimit);
    expect(actualMaster.flatGasPrice).toEqual(masterAfterUpdate.flatGasPrice);
    expect(actualMaster.other.gasPrice).toEqual(masterAfterUpdate.other.gasPrice);
    expect(actualMaster.other.gasLimit).toEqual(masterAfterUpdate.other.gasLimit);
    expect(actualMaster.other.specialGasLimit).toEqual(masterAfterUpdate.other.specialGasLimit);
    expect(actualMaster.other.gasCredit).toEqual(masterAfterUpdate.other.gasCredit);
    expect(actualMaster.other.blockGasLimit).toEqual(masterAfterUpdate.other.blockGasLimit);
    expect(actualMaster.other.freezeDueLimit).toEqual(masterAfterUpdate.other.freezeDueLimit);
    expect(actualMaster.other.deleteDueLimit).toEqual(masterAfterUpdate.other.deleteDueLimit);

    expect(actualBase.other.specialGasLimit).toNotEqual(masterAfterUpdate.other.specialGasLimit);
    expect(actualMaster.other.specialGasLimit).toNotEqual(baseAfterUpdate.other.specialGasLimit);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_set_gas_prices_extended_fields_keep_base_and_masterchain_independent_tests/config_set_gas_prices_extended_fields_keep_base_and_masterchain_independent.stdout.txt",
    );
}

#[test]
fn config_set_gas_prices_for_basechain_does_not_change_masterchain_extended_fields() {
    run_config_success_case(
        "bd-stdlib-config-gas-prices-single-chain-independence",
        r#"
get fun `test-bd-stdlib-config-gas-prices-single-chain-independence`() {
    var config = net.getConfig();

    val baseBefore = config.getGasPrices(BASECHAIN);
    val masterBefore = config.getGasPrices(MASTERCHAIN);

    val baseAfterUpdate = GasPrices {
        flatGasLimit: baseBefore.flatGasLimit + 501,
        flatGasPrice: baseBefore.flatGasPrice + 502,
        other: GasPricesExtended {
            gasPrice: baseBefore.other.gasPrice + 503,
            gasLimit: baseBefore.other.gasLimit + 504,
            specialGasLimit: baseBefore.other.specialGasLimit + 505,
            gasCredit: baseBefore.other.gasCredit + 506,
            blockGasLimit: baseBefore.other.blockGasLimit + 507,
            freezeDueLimit: baseBefore.other.freezeDueLimit + 508,
            deleteDueLimit: baseBefore.other.deleteDueLimit + 509,
        },
    };

    config.setGasPrices(baseAfterUpdate, BASECHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig();
    val actualBase = updated.getGasPrices(BASECHAIN);
    val actualMaster = updated.getGasPrices(MASTERCHAIN);

    expect(actualBase.flatGasLimit).toEqual(baseAfterUpdate.flatGasLimit);
    expect(actualBase.flatGasPrice).toEqual(baseAfterUpdate.flatGasPrice);
    expect(actualBase.other.gasPrice).toEqual(baseAfterUpdate.other.gasPrice);
    expect(actualBase.other.gasLimit).toEqual(baseAfterUpdate.other.gasLimit);
    expect(actualBase.other.specialGasLimit).toEqual(baseAfterUpdate.other.specialGasLimit);
    expect(actualBase.other.gasCredit).toEqual(baseAfterUpdate.other.gasCredit);
    expect(actualBase.other.blockGasLimit).toEqual(baseAfterUpdate.other.blockGasLimit);
    expect(actualBase.other.freezeDueLimit).toEqual(baseAfterUpdate.other.freezeDueLimit);
    expect(actualBase.other.deleteDueLimit).toEqual(baseAfterUpdate.other.deleteDueLimit);

    expect(actualMaster.flatGasLimit).toEqual(masterBefore.flatGasLimit);
    expect(actualMaster.flatGasPrice).toEqual(masterBefore.flatGasPrice);
    expect(actualMaster.other.gasPrice).toEqual(masterBefore.other.gasPrice);
    expect(actualMaster.other.gasLimit).toEqual(masterBefore.other.gasLimit);
    expect(actualMaster.other.specialGasLimit).toEqual(masterBefore.other.specialGasLimit);
    expect(actualMaster.other.gasCredit).toEqual(masterBefore.other.gasCredit);
    expect(actualMaster.other.blockGasLimit).toEqual(masterBefore.other.blockGasLimit);
    expect(actualMaster.other.freezeDueLimit).toEqual(masterBefore.other.freezeDueLimit);
    expect(actualMaster.other.deleteDueLimit).toEqual(masterBefore.other.deleteDueLimit);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_set_gas_prices_extended_fields_keep_base_and_masterchain_independent_tests/config_set_gas_prices_for_basechain_does_not_change_masterchain_extended_fields.stdout.txt",
    );
}
