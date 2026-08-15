import "../../lib/build"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"

get fun `test get config`() {
    val config = testing.getConfig();

    expect(config.getGlobalVersion().version).toBeGreaterOrEqual(12);
}

get fun `test set config`() {
    var config = testing.getConfig();

    var storagePricesBefore = config.getStoragePrices();
    var baseStorage = storagePricesBefore.getInitial();
    baseStorage.bitPrice += 10;

    val savedPrice = baseStorage.bitPrice;
    storagePricesBefore.setInitial(baseStorage);
    config.setStoragePrices(storagePricesBefore);

    val result = testing.setConfig(config);
    expect(result).toBeTrue();

    val updatedConfig = testing.getConfig();
    val pricesAfter = updatedConfig.getStoragePrices();
    val actual = pricesAfter.getInitial();

    expect(savedPrice).toEqual(actual.bitPrice);
}

get fun `test bad config`() {
    var config = testing.getConfig();

    config.setParamRaw(8, beginCell().endCell());

    val result = testing.setConfig(config);
    expect(result).toBeFalse();
}

get fun `test set negative config param`() {
    var config = testing.getConfig();

    val negPayload = beginCell().storeUint(0xABCD1234, 32).endCell();
    // Before the fix this call threw "Range check error": the config map used
    // uint32 keys, so a signed id like -137 could not be encoded. TON's config
    // dict is `Hashmap 32 ^Cell` keyed by the *signed* 32-bit index.
    config.setParamRaw(-137, negPayload);

    // the negative id round-trips locally
    expect(config.getParamRaw(-137)).toEqual(negPayload);

    // and does not collide with its positive twin (+137)
    val posPayload = beginCell().storeUint(0x5555AAAA, 32).endCell();
    config.setParamRaw(137, posPayload);
    expect(config.getParamRaw(-137)).toEqual(negPayload);
    expect(config.getParamRaw(137)).toEqual(posPayload);
}

get fun `test negative config param read by contract`() {
    // Deploy a contract that reads an out-of-consensus param via CONFIGOPTPARAM.
    val readerCode = build("config_reader");
    val readerInit = AutoDeployAddress {
        stateInit: { code: readerCode, data: beginCell().endCell() },
    };
    val readerAddr = readerInit.calculateAddress();

    val deployMsg = createMessage({
        dest: readerInit,
        bounce: false,
        value: ton("1"),
    });
    val deployer = testing.treasury("deployer");
    val deployRes = net.send(deployer.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({
        from: deployer.address,
        to: readerAddr,
    });

    // Set the out-of-consensus negative param and commit it.
    var config = testing.getConfig();
    val payload = beginCell().storeUint(0x00C0FFEE, 32).endCell();
    config.setParamRaw(-137, payload);
    expect(testing.setConfig(config)).toBeTrue();

    // It survives the round-trip through the raw config dict...
    expect(testing.getConfig().getParamRaw(-137)).toEqual(payload);

    // ...and the deployed contract reads back the same value via
    // `blockchain.configParam(-137)` (CONFIGOPTPARAM), proving the 32-bit
    // signed key bit-pattern matches what the VM looks up on-chain.
    val readBack = net.runGetMethod(readerAddr, "readNegParam") as int;
    expect(readBack).toEqual(0x00C0FFEE);
}

get fun `test get executor config`() {
    val simpleCode = build("simple");
    val getAddress = AutoDeployAddress {
        stateInit: { code: simpleCode, data: beginCell().endCell() },
    };

    val deployMsg = createMessage({
        dest: getAddress,
        bounce: false,
        value: ton("1"),
    });

    val deployer = testing.treasury("deployer");
    val res = net.send(deployer.address, deployMsg);

    expect(res).toHaveSuccessfulDeploy({
        from: deployer.address,
        to: getAddress.calculateAddress(),
    });

    val firstRes = net.runGetMethod(getAddress.calculateAddress(), "getParam") as int;

    var config = testing.getConfig();

    var storagePricesBefore = config.getGasPrices(BASECHAIN);
    storagePricesBefore.flatGasPrice += 100;
    config.setGasPrices(storagePricesBefore, BASECHAIN);

    val result = testing.setConfig(config);
    expect(result).toBeTrue();

    val secondRes = net.runGetMethod(getAddress.calculateAddress(), "getParam") as int;
    expect(firstRes).toNotEqual(secondRes);
}

fun calculateForwardFeeLocal(workchain: int8, bits: int, cells: int): coins
    asm(cells bits workchain) "GETFORWARDFEE"

get fun `test current config`() {
    val gas = 100;
    val before = calculateForwardFeeLocal(BASECHAIN, gas, gas);

    val priceBefore = ((testing
        .getC7OutsideContract()
        .get(0) as tuple).get(9) as BlockchainConfigMap)
        .getMsgForwardPrices(BASECHAIN)
        .lumpPrice;
    var config = testing.getConfig();

    var storagePricesBefore = config.getMsgForwardPrices(BASECHAIN);

    storagePricesBefore.lumpPrice += 2000;

    config.setMsgForwardPrices(storagePricesBefore, BASECHAIN);
    val result = testing.setConfig(config);
    expect(result).toBeTrue();

    val priceAfter = ((testing
        .getC7OutsideContract()
        .get(0) as tuple).get(9) as BlockchainConfigMap)
        .getMsgForwardPrices(BASECHAIN)
        .lumpPrice;
    expect(priceBefore).toNotEqual(priceAfter);

    val after = calculateForwardFeeLocal(BASECHAIN, gas, gas);

    expect(before).toNotEqual(after);
}
