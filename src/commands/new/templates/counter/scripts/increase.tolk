import "@acton/env"
import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"

import "@wrappers/Counter.gen"

import "utils/common"

fun main() {
    val delta = env<int>("COUNTER_DELTA") ?? 1;
    val deployer = scripts.wallet("deployer");
    val deployValue = counterDeployValue();
    val counterAddress = explicitCounterAddress();
    val counter = targetCounter(deployer.address);
    println("Deploy value: {}", deployValue);
    if (!scripts.isContractDeployed(counter.address)) {
        if (counterAddress != null) {
            println("Counter is not deployed at {}", counter.address);
            return;
        }
        counter.deploy(deployer.address, { value: deployValue }).waitForFirstTransaction();
    }

    println("Increasing counter by {}", delta);
    val res = counter.sendIncreaseCounter(deployer.address, delta);
    res.waitForFirstTransaction();

    println("Counter address: {}", counter.address);
    println("COUNTER_ADDRESS={}", counter.address);
    println("Counter owner: {}", counter.owner());
    println("Current counter value: {}", counter.currentCounter());
}
