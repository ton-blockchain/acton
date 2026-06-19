import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/JettonMinter.gen"
import "utils/common"

fun main() {
    val claimer = promptJettonWallet("JETTON_PENDING_ADMIN", "Select Pending Admin ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");

    val res = minter.sendClaimMinterAdmin(
        claimer.address,
        0,
        { bounce: false, value: grams("0.05") },
    );
    println(res.waitForTrace());

    println("JETTON CLAIM_SENDER={}", claimer.address);
    printJettonMinterInfo(minter, "JETTON");
}
