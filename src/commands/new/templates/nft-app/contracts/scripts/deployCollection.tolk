import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val info = promptNftCollectionInfo();

    val collection = deployNftCollection(deployer, info);

    printNftCollectionInfo(collection, "NFT");
}
