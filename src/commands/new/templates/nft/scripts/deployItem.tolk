import "@acton/promts/prompts"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val collection = promptNftCollectionContract("Enter NFT Collection address");
    val itemContent = prompt("Enter item content path", "", NFT_DEFAULT_ITEM_CONTENT);
    val owner = promptNftAddress("Enter NFT Item owner", deployer.address);

    val item = deployNftItemContract(collection, deployer, owner, itemContent);

    printNftItemInfo(item, "NFT");
}
