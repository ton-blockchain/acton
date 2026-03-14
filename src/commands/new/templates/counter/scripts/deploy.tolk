import "@acton/io"
import "@acton/build/build"
import "@acton/emulation/network"
import "@contracts/types"

import "@tests/wrappers/Counter"

fun main() {
    val deployer = net.wallet("deployer");

    val counter = Counter.fromStorage(Storage {
        id: 0,
        counter: 0,
    });
    val res = counter.deploy(deployer.address, { value: ton("0.005") });
    res.wait();

    println1("Deployed counter to {}", counter.address);
}
