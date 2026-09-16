import "@acton/fmt"
import "@acton/io"
import "@acton/prompts"
import "@acton/emulation/network"
import "@acton/testing/assert"
import "@contracts/fees-management"
import "utils/common"

fun main() {
    val deployer = promptNftWallet("NFT_DEPLOYER", "Select NFT Deployer");
    val collection = promptNftCollection("Enter NFT Collection address");
    val batchOwner = promptNftAddress("Enter NFT batch owner", deployer.address);
    val batchCount = promptInt("Enter batch size", NFT_DEFAULT_BATCH_COUNT);
    if (batchCount <= 0 || batchCount >= BATCH_SIZE_LIMIT) {
        Assert.fail(format("Batch size must be in range 1..{}", BATCH_SIZE_LIMIT - 1));
    }

    val res = deployNftBatch(collection, deployer, batchOwner, batchCount);
    println(res.waitForTrace());

    printNftCollectionInfo(collection, "NFT");
}
