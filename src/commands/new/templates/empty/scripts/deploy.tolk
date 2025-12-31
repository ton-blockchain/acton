import "../tests/wrappers/Counter"
import "../contracts/types"

import "../.acton/io"
import "../.acton/emulation/network"
import "../.acton/build/build"

fun main() {
    val deployer = net.wallet("deployer");

    // When your contract is ready, uncomment these lines
    // val counter = Contract.fromStorage(Storage { id: 0, counter: 0 });
    // val res = counter.deploy(deployer.address, { value: ton("0.005") });
    // res.wait();
    //
    // println1("Deployed counter to {}", counter.address);
}
