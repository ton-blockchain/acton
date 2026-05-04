import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@contracts/types"
import "@wrappers/Counter.gen"

fun main() {
    val deployer = scripts.wallet("deployer");

    val counter = Counter.fromStorage(Storage {
        id: 0,
        owner: deployer.address,
        counter: 0,
    });
    val res = counter.deploy(deployer.address, { value: ton("0.005") });
    println(res.waitForTrace());

    println("Deployed counter to {}", counter.address);
    println("Counter owner is {}", counter.owner());
    println("Counter value is {}", counter.currentCounter());
}
