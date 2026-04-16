import "@acton/emulation/network"
import "@acton/promts/prompts"
import "utils/common"
import "../wrappers/NftCollectionContract"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val collection = promptNftCollectionContract("Enter NFT Collection address");
    val newAdmin = promptNftAddress(
        "Enter new admin address",
        net.treasury("nft-new-admin").address,
    );

    val res = collection.sendChangeCollectionAdmin(
        deployer.address,
        0,
        newAdmin,
        { value: ton("0.05"), bounce: true },
    );
    res.wait();

    printNftCollectionInfo(collection, "NFT");
}
