import "@local-acton/emulation/config"
import "@local-acton/emulation/network"
import "@local-acton/build/build"
import "@local-acton/testing/expect"

struct ConfigReadContract {
    address: address
}

fun ConfigReadContract.deploy(): ConfigReadContract {
    val init = ContractState {
        code: build("ConfigReader", "tests/acton-stdlib/contracts/config_reader.tolk"),
        data: beginCell().endCell(),
    };
    val address = AutoDeployAddress { stateInit: init, workchain: MASTERCHAIN }.calculateAddress();

    val deployer = net.treasury("deployer");
    net.send(deployer.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: AutoDeployAddress { stateInit: init, workchain: MASTERCHAIN },
    }));

    return ConfigReadContract { address };
}

fun ConfigReadContract.readConfigParam(self, idx: int): int {
    return net.runGetMethod(self.address, "read_config_param", idx);
}

/// Test that deleting a config param via map.delete + net.setConfig
/// actually removes it from blockchain.configParam() inside a contract
get fun `test config delete param`() {
    val contract = ConfigReadContract.deploy();

    // Verify param 0 exists
    val before = contract.readConfigParam(0);
    expect(before).toEqual(1); // 1 = exists

    // Delete param 0 from config
    var config = net.getConfig();
    config.delete(0);
    expect(config.exists(0)).toEqual(false); // deleted from map
    net.setConfig(config);

    // Check if the contract sees it as deleted
    val after = contract.readConfigParam(0);
    expect(after).toEqual(0); // 0 = null / not found
}
