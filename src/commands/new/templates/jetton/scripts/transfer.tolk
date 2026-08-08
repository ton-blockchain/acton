import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/JettonMinter.gen"
import "@wrappers/JettonWallet.gen"
import "@wrappers/utils"
import "utils/common"

fun main() {
    val sender = promptJettonWallet("JETTON_SENDER", "Select Jetton Sender ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");
    val recipient = promptJettonAddress(
        "JETTON_TRANSFER_RECIPIENT",
        "Enter transfer recipient",
    );
    val transferAmount = promptJettonCoins(
        "JETTON_TRANSFER_AMOUNT",
        "Enter transfer amount",
        JETTON_DEFAULT_TRANSFER_AMOUNT,
    );

    val senderWallet = JettonWallet { address: minter.getWalletAddress(sender.address) };
    val res = senderWallet.sendTransfer(
        sender.address,
        grams("0.08"),
        transferAmount,
        recipient,
        sender.address,
        null,
        1,
        null,
    );
    println(res.waitForTrace());

    printJettonMinterInfo(minter, "JETTON");
    printJettonWalletInfo(minter, sender.address, "JETTON_SENDER");
    printJettonWalletInfo(minter, recipient, "JETTON_RECIPIENT");
}
