import "@stdlib/strings"
import "@acton/fmt"
import "@acton/io"
import "@acton/build/build"
import "@acton/emulation/network"
import "@acton/promts/prompts"
import "@acton/testing/assert"
import "@acton/vm/vm"
import "../../contracts/messages"
import "../../contracts/types"
import "../../wrappers/NftCollectionContract"
import "../../wrappers/NftItemContract"

const ONCHAIN_CONTENT_PREFIX = 0x00
const SNAKE_PREFIX = 0x00

// Matches the `counter < 250` guard in NftCollection.tolk BatchDeployNfts handler.
const NFT_MAX_BATCH_SIZE = 249

const NFT_DEFAULT_COLLECTION_NAME = "SomeNFT"
const NFT_DEFAULT_COLLECTION_DESCRIPTION = "Some NFT collection"
const NFT_DEFAULT_COLLECTION_IMAGE = "https://ton.org/download/ton_symbol.svg"
const NFT_DEFAULT_COMMON_CONTENT = "https://example.com/nft/"
const NFT_DEFAULT_ITEM_CONTENT = "item-0.json"
const NFT_DEFAULT_BATCH_CONTENT_PREFIX = "batch-"
const NFT_DEFAULT_BATCH_COUNT = "3"

struct NftCollectionInfo {
    name: string
    description: string
    image: string
}

fun string.toSnakeString(self): cell {
    return beginCell().storeUint(SNAKE_PREFIX, 8).storeSlice(self.beginParse()).endCell();
}

fun buildOnchainMetadata(info: NftCollectionInfo): cell {
    var dict: map<uint256, cell> = [];
    dict.set("name".sha256(), info.name.toSnakeString());
    dict.set("description".sha256(), info.description.toSnakeString());
    dict.set("image".sha256(), info.image.toSnakeString());
    return beginCell()
        .storeUint(ONCHAIN_CONTENT_PREFIX, 8)
        .storeDict(dict.toLowLevelDict())
        .endCell();
}

fun promptNftWallet(message: string): Wallet {
    return net.wallet(promptWallet(message));
}

fun promptNftAddress(message: string, default: address): address {
    val s = prompt(message, "", format("{}", default));
    return vm.convertAddress(s);
}

fun promptNftCollectionInfo(): NftCollectionInfo {
    return {
        name: prompt("Enter collection name", "", NFT_DEFAULT_COLLECTION_NAME),
        description: prompt("Enter collection description", "", NFT_DEFAULT_COLLECTION_DESCRIPTION),
        image: prompt("Enter collection image URL", "", NFT_DEFAULT_COLLECTION_IMAGE),
    };
}

fun promptNftCollectionContract(message: string): NftCollectionContract {
    val s = prompt(message, "EQ... / kQ...", "");
    return NftCollectionContract.fromAddress(vm.convertAddress(s));
}

fun promptNftItemContract(message: string): NftItemContract {
    val s = prompt(message, "EQ... / kQ...", "");
    return NftItemContract.fromAddress(vm.convertAddress(s));
}

fun promptInt(message: string, default: string): int {
    return vm.parseInt(prompt(message, "", default));
}

fun makeNftCollectionStorage(admin: address, info: NftCollectionInfo): NftCollectionStorage {
    return {
        adminAddress: admin,
        nextItemIndex: 0,
        content: CollectionContent {
            collectionMetadata: buildOnchainMetadata(info),
            commonContent: NFT_DEFAULT_COMMON_CONTENT,
        }.toCell(),
        nftItemCode: build("NftItem"),
        royaltyParams: RoyaltyParams {
            numerator: 5,
            denominator: 100,
            royaltyAddress: admin,
        }.toCell(),
    };
}

fun deployNftCollectionContract(deployer: Wallet, info: NftCollectionInfo): NftCollectionContract {
    val collection = NftCollectionContract.fromStorage(
        makeNftCollectionStorage(deployer.address, info),
    );
    val res = collection.deploy(deployer.address, { value: ton("0.05"), bounce: false });
    res.wait();
    return collection;
}

fun deployNftItemContract(
    collection: NftCollectionContract,
    deployer: Wallet,
    owner: address,
    itemContent: string,
): NftItemContract {
    val itemIndex = collection.getCollectionData().nextItemIndex;
    val res = collection.sendDeployNft(
        deployer.address,
        0,
        itemIndex,
        ton("0.03"),
        { ownerAddress: owner, content: itemContent },
        { value: ton("0.05"), bounce: true },
    );
    res.wait();
    return NftItemContract.fromAddress(collection.getNftAddressByIndex(itemIndex));
}

fun deployNftBatch(
    collection: NftCollectionContract,
    deployer: Wallet,
    owner: address,
    count: int,
): SendResultList {
    if (count <= 0) {
        Assert.fail(format("Batch size must be positive, got {}", count));
    }
    if (count > NFT_MAX_BATCH_SIZE) {
        Assert.fail(
            format(
                "Batch size {} exceeds NftCollection limit of {}; the on-chain transaction would fail and burn fees.",
                count,
                NFT_MAX_BATCH_SIZE,
            ),
        );
    }

    var deployList: map<uint64, BatchDeployDictItem> = [];
    var i = 0;
    val nextItemIndex = collection.getCollectionData().nextItemIndex;
    val perItemAmount = ton("0.03");

    while (i < count) {
        val index = (nextItemIndex + i);
        deployList.set(
            index,
            BatchDeployDictItem {
                attachTonAmount: perItemAmount,
                initParams: NftItemInitAtDeployment {
                    ownerAddress: owner,
                    content: NFT_DEFAULT_BATCH_CONTENT_PREFIX
                        .concat(index.toDecimalString())
                        .concat(".json"),
                }.toCell(),
            },
        );
        i += 1;
    }

    val messageValue = perItemAmount * count + ton("0.1");
    return collection.sendBatchDeployNfts(
        deployer.address,
        0,
        deployList,
        { value: messageValue, bounce: true },
    );
}

fun printNftCollectionInfo(collection: NftCollectionContract, label: string) {
    val data = collection.getCollectionData();
    val royalty = collection.royaltyParams();
    println("{} COLLECTION_ADDRESS={}", label, collection.address);
    println("{} ADMIN_ADDRESS={}", label, data.adminAddress);
    println("{} NEXT_ITEM_INDEX={}", label, data.nextItemIndex);
    println("{} COLLECTION_METADATA_HASH={}", label, data.collectionMetadata.hash());
    println("{} ROYALTY_ADDRESS={}", label, royalty.royaltyAddress);
    println("{} ROYALTY_NUMERATOR={}", label, royalty.numerator);
    println("{} ROYALTY_DENOMINATOR={}", label, royalty.denominator);
    println(
        "{} NFT_CONTENT_HASH={}",
        label,
        collection.getNftContent(0, NFT_DEFAULT_ITEM_CONTENT).hash(),
    );
}

fun printNftItemInfo(item: NftItemContract, label: string) {
    val data = item.getNftData();
    println("{} ITEM_ADDRESS={}", label, item.address);
    println("{} INITIALIZED={}", label, data.isInitialized);
    println("{} INDEX={}", label, data.itemIndex);
    println("{} COLLECTION_ADDRESS={}", label, data.collectionAddress);
    println("{} OWNER_ADDRESS={}", label, data.ownerAddress);
    println("{} CONTENT={}", label, data.content);
}
