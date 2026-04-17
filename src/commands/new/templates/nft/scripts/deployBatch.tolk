import "@acton/emulation/network"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val collection = promptNftCollectionContract("Enter NFT Collection address");
    val batchOwner = promptNftAddress("Enter NFT batch owner", deployer.address);
    val batchCount = promptInt("Enter batch size", NFT_DEFAULT_BATCH_COUNT);

    val res = deployNftBatch(collection, deployer, batchOwner, batchCount);
    res.wait();

    printNftCollectionInfo(collection, "NFT");
}
