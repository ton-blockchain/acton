import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"

import "@contracts/types"
import "@wrappers/Empty"

fun main() {
    val deployer = scripts.wallet("deployer");

    val contract = Empty.fromStorage(Storage { owner: deployer.address });
    val res = contract.deploy(deployer.address, { value: ton("0.05") });
    if (res.waitForFirstTransaction() == null) {
        return;
    }

    println("Deployed starter contract to {}", contract.address);
    println("Owner is {}", contract.owner());
}
