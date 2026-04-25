import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/JettonMinter.gen"
import "utils/common"

fun main() {
    val admin = promptJettonWallet("JETTON_ADMIN", "Select Current Admin ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");
    val nextAdmin = promptJettonAddress(
        "JETTON_NEXT_ADMIN",
        "Enter next admin address",
        admin.address,
    );

    val res = minter.sendChangeMinterAdmin(
        admin.address,
        0,
        nextAdmin,
        { bounce: false, value: ton("0.05") },
    );
    res.waitForFirstTransaction();

    println("JETTON NEXT_ADMIN_REQUESTED={}", nextAdmin);
    printJettonMinterInfo(minter, "JETTON");
}
