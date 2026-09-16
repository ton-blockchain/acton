import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/utils"
import "utils/common"

fun main() {
    val admin = promptJettonWallet("JETTON_ADMIN", "Select Current Admin ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");
    val nextAdmin = promptJettonAddress(
        "JETTON_NEXT_ADMIN",
        "Enter next admin address",
        admin.address,
    );

    val res = minter.sendChangeAdmin(admin.address, nextAdmin);
    println(res.waitForTrace());

    println("JETTON NEXT_ADMIN_REQUESTED={}", nextAdmin);
    printJettonMinterInfo(minter, "JETTON");
}
