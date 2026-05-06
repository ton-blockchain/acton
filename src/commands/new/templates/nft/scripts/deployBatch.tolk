import "@acton/fmt"
import "@acton/io"
import "@acton/prompts"
import "@acton/emulation/network"
import "@acton/testing/assert"
import "@contracts/fees-management"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("Select NFT Deployer");
    val collection = promptNftCollection("Enter NFT Collection address");
    val batchOwner = promptNftAddress("Enter NFT batch owner", deployer.address);
    val batchCount = promptInt("Enter batch size", NFT_DEFAULT_BATCH_COUNT);
    if (batchCount > BATCH_SIZE_LIMIT) {
        Assert.fail(format("Batch size can't be higher than {}", BATCH_SIZE_LIMIT));
    }

    val res = deployNftBatch(collection, deployer, batchOwner, batchCount);
    println(res.waitForTrace());

    printNftCollectionInfo(collection, "NFT");
}
