import "@acton/env"
import "@acton/prompts"
import "utils/common"

fun main() {
    val minter = promptJettonMinter("Enter Jetton Minter address");
    printJettonMinterInfo(minter, "JETTON");

    val owner = env<address>("JETTON_OWNER_ADDRESS");
    if (owner != null) {
        printJettonWalletInfo(minter, owner, "JETTON_OWNER");
        return;
    }

    if (
        !confirm(
            "Show holder wallet info?",
            false,
            "Prints wallet address and balance for an owner",
        )
    ) {
        return;
    }
    val promptedOwner = promptJettonAddress("JETTON_OWNER_ADDRESS", "Enter owner address");
    printJettonWalletInfo(minter, promptedOwner, "JETTON_OWNER");
}
