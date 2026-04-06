import "@acton/io"
import "@acton/emulation/network"
import "@contracts/types"

import "@wrappers/Counter"

fun main() {
    val deployer = net.wallet("deployer");

    val counter = Counter.fromStorage(Storage {
        id: 0,
        counter: 0,
    });
    val res = counter.deploy(deployer.address, { value: ton("0.005") });
    if (!res.wait()) {
        return;
    }

    println1("Deployed counter to {}", counter.address);
}
