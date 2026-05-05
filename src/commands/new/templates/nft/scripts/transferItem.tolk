import "@acton/io"
import "@acton/emulation/network"
import "@wrappers/utils"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val item = promptNftItem("Enter NFT Item address");
    val newOwner = promptNftAddress("Enter new owner address");

    val res = item.sendTransferPayloadInline(
        deployer.address,
        0,
        newOwner,
        null,
        null,
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );
    println(res.waitForTrace());

    printNftItemInfo(item, "NFT");
}
