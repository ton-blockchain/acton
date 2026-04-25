import "@acton/emulation/scripts"
import "utils/common"

fun main() {
    val deployer = promptJettonWallet("JETTON_DEPLOYER", "Select Jetton Deployer");
    val admin = promptJettonAddress(
        "JETTON_ADMIN_ADDRESS",
        "Enter admin address",
        deployer.address,
    );
    val storage = promptJettonMinterStorage(admin);

    val minter = deployJettonMinter(deployer, storage);

    printJettonMinterInfo(minter, "JETTON");
    printJettonWalletInfo(minter, admin, "JETTON_ADMIN");
}
