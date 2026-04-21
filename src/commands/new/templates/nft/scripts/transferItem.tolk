import "@stdlib/tvm-dicts"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@wrappers/NftItemContract"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val item = promptNftItemContract("Enter NFT Item address");
    val newOwner = promptNftAddress(
        "Enter new owner address",
        testing.treasury("nft-new-owner").address,
    );

    val res = item.sendAskToChangeOwnership(
        deployer.address,
        0,
        newOwner,
        null,
        createEmptyDict(),
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );
    res.waitForFirstTransaction();

    printNftItemInfo(item, "NFT");
}
