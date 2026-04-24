import "@acton/prompts"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val collection = promptNftCollection("Enter NFT Collection address");
    val itemContent = prompt("Enter item content path", "", NFT_DEFAULT_ITEM_CONTENT);
    val owner = promptNftAddress("Enter NFT Item owner", deployer.address);

    val item = deployNftItem(collection, deployer, owner, itemContent);

    printNftItemInfo(item, "NFT");
}
