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

    val priceBefore = ((testing.getC7OutsideContract().get(0) as tuple).get(9) as BlockchainConfigMap)
        .getMsgForwardPrices(BASECHAIN)
        .lumpPrice;
    var config = testing.getConfig();

    var storagePricesBefore = config.getMsgForwardPrices(BASECHAIN);

    storagePricesBefore.lumpPrice += 2000;

    config.setMsgForwardPrices(storagePricesBefore, BASECHAIN);
    val result = testing.setConfig(config);
    expect(result).toBeTrue();

    val priceAfter = ((testing.getC7OutsideContract().get(0) as tuple).get(9) as BlockchainConfigMap)
        .getMsgForwardPrices(BASECHAIN)
        .lumpPrice;
    expect(priceBefore).toNotEqual(priceAfter);

    val after = calculateForwardFeeLocal(BASECHAIN, gas, gas);

    expect(before).toNotEqual(after);
}
