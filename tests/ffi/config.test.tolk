import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

get fun `test get config`() {
    val config = net.getConfig();

    expect(config.getGlobalVersion().version).toBeGreaterOrEqual(12);
}

get fun `test set config`() {
    var config = net.getConfig();

    var storagePricesBefore = config.getStoragePrices();
    var baseStorage = storagePricesBefore.getInitial();
    baseStorage.bitPrice += 10;

    val savedPrice = baseStorage.bitPrice;
    storagePricesBefore.setInitial(baseStorage);
    config.setStoragePrices(storagePricesBefore);

    val result = net.setConfig(config);
    expect(result).toBeTrue();

    val updatedConfig = net.getConfig();
    var pricesAfter = updatedConfig.getStoragePrices();
    var actual = pricesAfter.getInitial();

    expect(savedPrice).toEqual(actual.bitPrice);
}

get fun `test bad config`() {
    var config = net.getConfig();

    config.setParamRaw(8, beginCell().endCell());

    val result = net.setConfig(config);
    expect(result).toBeFalse();
}
