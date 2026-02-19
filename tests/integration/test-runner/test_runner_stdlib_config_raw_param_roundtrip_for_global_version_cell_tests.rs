use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CONFIG_IMPORTS: &str = r#"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

const CONFIG_IMPORTS_WITH_BUILD: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
"#;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
fun calculateGasFee(workchain: int8, gasUsed: int): coins
    asm(gasUsed workchain) "GETGASFEE"
get fun getParam() {
    return calculateGasFee(0, 100000);
}
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

fn run_config_success_case_with_contract(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CONFIG_IMPORTS_WITH_BUILD}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
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
fn config_raw_param_roundtrip_for_global_version_cell() {
    run_config_success_case(
        "ai-stdlib-config-raw-param-roundtrip",
        r#"
get fun `test-ai-stdlib-config-raw-param-roundtrip`() {
    var config = net.getConfig();
    val overrideVersion = GlobalVersion {
        version: 12345,
        capabilities: 0xABCD,
    };

    config.setParamRaw(GLOBAL_VERSION_INDEX, overrideVersion.toCell());

    val roundtripRaw = config.getParamRaw(GLOBAL_VERSION_INDEX);
    val decoded = GlobalVersion.fromCell(roundtripRaw);

    expect(decoded.version).toEqual(12345);
    expect(decoded.capabilities).toEqual(0xABCD);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_raw_param_roundtrip_for_global_version_cell.stdout.txt",
    );
}

#[test]
fn config_set_config_rejects_invalid_global_version_cell() {
    run_config_success_case(
        "ai-stdlib-config-invalid-global-version",
        r#"
get fun `test-ai-stdlib-config-invalid-global-version-cell`() {
    var config = net.getConfig();
    config.setParamRaw(GLOBAL_VERSION_INDEX, beginCell().endCell());

    val result = net.setConfig(config);
    expect(result).toBeFalse();
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_set_config_rejects_invalid_global_version_cell.stdout.txt",
    );
}

#[test]
fn config_storage_prices_roundtrip_updates_initial_entry() {
    run_config_success_case(
        "ai-stdlib-config-storage-prices-roundtrip",
        r#"
get fun `test-ai-stdlib-config-storage-prices-roundtrip`() {
    var config = net.getConfig();
    var prices = config.getStoragePrices();
    var initial = prices.getInitial();

    initial.bitPrice += 11;
    initial.cellPrice += 22;
    val expectedBitPrice = initial.bitPrice;
    val expectedCellPrice = initial.cellPrice;

    prices.setInitial(initial);
    config.setStoragePrices(prices);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getStoragePrices().getInitial();
    expect(updated.bitPrice).toEqual(expectedBitPrice);
    expect(updated.cellPrice).toEqual(expectedCellPrice);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_storage_prices_roundtrip_updates_initial_entry.stdout.txt",
    );
}

#[test]
fn config_gas_prices_update_basechain_and_masterchain_independently() {
    run_config_success_case(
        "ai-stdlib-config-gas-prices-roundtrip",
        r#"
get fun `test-ai-stdlib-config-gas-prices-roundtrip`() {
    var config = net.getConfig();

    var basechain = config.getGasPrices(BASECHAIN);
    var masterchain = config.getGasPrices(MASTERCHAIN);

    val baseBefore = basechain.flatGasPrice;
    val masterBefore = masterchain.flatGasPrice;

    basechain.flatGasPrice += 111;
    masterchain.flatGasPrice += 222;

    val expectedBase = basechain.flatGasPrice;
    val expectedMaster = masterchain.flatGasPrice;

    config.setGasPrices(basechain, BASECHAIN);
    config.setGasPrices(masterchain, MASTERCHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig();
    val actualBase = updated.getGasPrices(BASECHAIN);
    val actualMaster = updated.getGasPrices(MASTERCHAIN);

    expect(actualBase.flatGasPrice).toEqual(expectedBase);
    expect(actualMaster.flatGasPrice).toEqual(expectedMaster);
    expect(actualBase.flatGasPrice).toNotEqual(baseBefore);
    expect(actualMaster.flatGasPrice).toNotEqual(masterBefore);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_gas_prices_update_basechain_and_masterchain_independently.stdout.txt",
    );
}

#[test]
fn config_msg_forward_prices_update_changes_forward_fee_opcode_result() {
    run_config_success_case(
        "ai-stdlib-config-msg-forward-prices",
        r#"
fun calculateForwardFeeLocal(workchain: int8, bits: int, cells: int): coins
    asm(cells bits workchain) "GETFORWARDFEE"

get fun `test-ai-stdlib-config-msg-forward-prices`() {
    val before = calculateForwardFeeLocal(BASECHAIN, 100, 100);

    var config = net.getConfig();
    var basechainPrices = config.getMsgForwardPrices(BASECHAIN);
    basechainPrices.lumpPrice += 2000;
    val expectedLump = basechainPrices.lumpPrice;

    config.setMsgForwardPrices(basechainPrices, BASECHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val updatedPrices = net.getConfig().getMsgForwardPrices(BASECHAIN);
    expect(updatedPrices.lumpPrice).toEqual(expectedLump);

    val after = calculateForwardFeeLocal(BASECHAIN, 100, 100);
    expect(after).toNotEqual(before);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_msg_forward_prices_update_changes_forward_fee_opcode_result.stdout.txt",
    );
}

#[test]
fn config_precompiled_contracts_roundtrip_and_duplicate_insert_guard() {
    run_config_success_case(
        "ai-stdlib-config-precompiled-contracts",
        r#"
get fun `test-ai-stdlib-config-precompiled-contracts`() {
    var precompiled = PrecompiledContractsConfig {
        list: createEmptyMap<uint256, PrecompiledSmartContract>(),
    };

    val hashA: uint256 = 0x1234;
    val hashB: uint256 = 0x5678;

    expect(precompiled.addContractGas(hashA, 777)).toBeTrue();
    expect(precompiled.addContractGas(hashA, 999)).toBeFalse();
    expect(precompiled.addContractGas(hashB, 555)).toBeTrue();

    var config = net.getConfig();
    config.setPrecompiledContractsConfig(precompiled);
    expect(net.setConfig(config)).toBeTrue();

    val updated = net.getConfig().getPrecompiledContractsConfig();
    expect(updated.list).toHaveLength(2);

    val first = updated.list.get(hashA).loadValue();
    val second = updated.list.get(hashB).loadValue();

    expect(first.gasUsage).toEqual(777);
    expect(second.gasUsage).toEqual(555);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_precompiled_contracts_roundtrip_and_duplicate_insert_guard.stdout.txt",
    );
}

#[test]
fn config_gas_price_change_affects_run_get_method_result() {
    run_config_success_case_with_contract(
        "ai-stdlib-config-gas-price-affects-get-method",
        r#"
get fun `test-ai-stdlib-config-gas-price-affects-run-get-method`() {
    val simpleCode = build("simple");
    val autoAddress = AutoDeployAddress {
        stateInit: { code: simpleCode, data: beginCell().endCell() },
    };

    val deployer = net.treasury("ai-config-gas-deployer");
    val deployMsg = createMessage({
        dest: autoAddress,
        bounce: false,
        value: ton("1"),
    });

    val deployResult = net.send(deployer.address, deployMsg);
    expect(deployResult).toHaveSuccessfulDeploy({
        from: deployer.address,
        to: autoAddress.calculateAddress(),
    });

    val before = net.runGetMethod(autoAddress.calculateAddress(), "getParam") as int;

    var config = net.getConfig();
    var basechain = config.getGasPrices(BASECHAIN);
    basechain.flatGasPrice += 100;
    config.setGasPrices(basechain, BASECHAIN);
    expect(net.setConfig(config)).toBeTrue();

    val after = net.runGetMethod(autoAddress.calculateAddress(), "getParam") as int;
    expect(after).toNotEqual(before);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_config_raw_param_roundtrip_for_global_version_cell_tests/config_gas_price_change_affects_run_get_method_result.stdout.txt",
    );
}
