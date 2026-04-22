import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@wrappers/NftCollectionContract"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val collection = promptNftCollectionContract("Enter NFT Collection address");
    val newAdmin = promptNftAddress(
        "Enter new admin address",
        testing.treasury("nft-new-admin").address,
    );

    val res = collection.sendChangeCollectionAdmin(
        deployer.address,
        0,
        newAdmin,
        { value: ton("0.05"), bounce: true },
    );
    res.waitForFirstTransaction();

    printNftCollectionInfo(collection, "NFT");
}
