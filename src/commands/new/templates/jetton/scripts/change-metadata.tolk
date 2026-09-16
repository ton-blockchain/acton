import "@acton/io"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@wrappers/utils"
import "utils/common"

fun main() {
    val admin = promptJettonWallet("JETTON_ADMIN", "Select Jetton Admin ActonWallet");
    val minter = promptJettonMinter("Enter Jetton Minter address");
    val metadata = promptJettonMetadata(JETTON_DEFAULT_UPDATED_DESCRIPTION);

    val res = minter.sendChangeMetadata(admin.address, buildJettonMetadataCell(metadata));
    println(res.waitForTrace());

    printJettonMinterInfo(minter, "JETTON");
}
