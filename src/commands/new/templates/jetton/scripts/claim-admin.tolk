import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/utils"
import "utils/common"

fun main() {
    val claimer = promptJettonWallet("JETTON_PENDING_ADMIN", "Select Pending Admin ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");

    val res = minter.sendClaimAdmin(claimer.address);
    println(res.waitForTrace());

    println("JETTON CLAIM_SENDER={}", claimer.address);
    printJettonMinterInfo(minter, "JETTON");
}
