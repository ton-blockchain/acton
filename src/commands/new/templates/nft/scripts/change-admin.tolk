import "@acton/io"
import "@acton/emulation/network"
import "@wrappers/NftCollection.gen"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("NFT_DEPLOYER", "Select NFT Deployer");
    val collection = promptNftCollection("Enter NFT Collection address");
    val newAdmin = promptNftAddress("Enter new admin address");

    val res = collection.sendChangeCollectionAdmin(
        deployer.address,
        0,
        newAdmin,
        { value: grams("0.05"), bounce: true },
    );
    println(res.waitForTrace());

    printNftCollectionInfo(collection, "NFT");
}
