import "@acton/io"
import "@acton/emulation/network"

import "@contracts/types"
import "@wrappers/Empty"

fun main() {
    val deployer = net.wallet("deployer");

    val contract = Empty.fromStorage(Storage { owner: deployer.address });
    val res = contract.deploy(deployer.address, { value: ton("0.05") });
    if (!res.wait()) {
        return;
    }

    println1("Deployed starter contract to {}", contract.address);
    println1("Owner is {}", contract.owner());
}
