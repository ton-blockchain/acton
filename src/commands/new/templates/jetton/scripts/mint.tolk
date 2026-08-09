import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/utils"
import "utils/common"

fun main() {
    val admin = promptJettonWallet("JETTON_ADMIN", "Select Jetton Admin ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");
    val recipient = promptJettonAddress(
        "JETTON_MINT_RECIPIENT",
        "Enter mint recipient",
        admin.address,
    );
    val mintAmount = promptJettonCoins(
        "JETTON_MINT_AMOUNT",
        "Enter mint amount",
        JETTON_DEFAULT_MINT_AMOUNT,
    );

    val res = minter.sendMint(admin.address, recipient, mintAmount, 1, grams("0.08"));
    println(res.waitForTrace());

    printJettonMinterInfo(minter, "JETTON");
    printJettonWalletInfo(minter, recipient, "JETTON_RECIPIENT");
}
