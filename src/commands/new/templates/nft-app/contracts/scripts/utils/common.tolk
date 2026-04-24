import "@stdlib/strings"
import "@acton/build"
import "@acton/fmt"
import "@acton/io"
import "@acton/prompts"
import "@acton/emulation/network"
import "@acton/emulation/scripts"
import "@contracts/messages"
import "@contracts/types"
import "@wrappers/NftCollection.gen"
import "@wrappers/NftItem.gen"

const ONCHAIN_CONTENT_PREFIX = 0x00
const SNAKE_PREFIX = 0x00

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

fun promptNftWallet(message: string): ActonWallet {
    return scripts.wallet(promptWallet(message));
}

fun promptNftAddress(message: string, default: address? = null): address {
    val s = default == null ? prompt(message, "") : prompt(message, "", format("{}", default));
    return parseAddress(s);
}

fun promptNftCollectionInfo(): NftCollectionInfo {
    return {
        name: prompt("Enter collection name", "", NFT_DEFAULT_COLLECTION_NAME),
        description: prompt("Enter collection description", "", NFT_DEFAULT_COLLECTION_DESCRIPTION),
        image: prompt("Enter collection image URL", "", NFT_DEFAULT_COLLECTION_IMAGE),
    };
}

fun promptNftCollection(message: string): NftCollection {
    val s = prompt(message, "EQ... / kQ...", "");
    return NftCollection.fromAddress(parseAddress(s));
}

fun promptNftItem(message: string): NftItem {
    val s = prompt(message, "EQ... / kQ...", "");
    return NftItem.fromAddress(parseAddress(s));
}

fun promptInt(message: string, default: string): int {
    return parseInt(prompt(message, "", default));
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

fun deployNftCollection(deployer: ActonWallet, info: NftCollectionInfo): NftCollection {
    val collection = NftCollection.fromStorage(
        makeNftCollectionStorage(deployer.address, info),
    );
    val res = collection.deploy(deployer.address, { value: ton("0.05"), bounce: false });
    res.waitForFirstTransaction();
    return collection;
}

fun deployNftItem(
    collection: NftCollection,
    deployer: ActonWallet,
    owner: address,
    itemContent: string,
): NftItem {
    val itemIndex = collection.getCollectionData().nextItemIndex;
    val res = collection.sendDeployNft(
        deployer.address,
        0,
        itemIndex,
        ton("0.03"),
        { ownerAddress: owner, content: itemContent },
        { value: ton("0.05"), bounce: true },
    );
    res.waitForFirstTransaction();
    return NftItem.fromAddress(collection.getNftAddressByIndex(itemIndex));
}

fun deployNftBatch(
    collection: NftCollection,
    deployer: ActonWallet,
    owner: address,
    count: int,
): SendResultList {
    var deployList: map<uint64, BatchDeployDictItem> = [];
    var i = 0;
    val nextItemIndex = collection.getCollectionData().nextItemIndex;

    while (i < count) {
        val index = (nextItemIndex + i);
        val content = StringBuilder
            .create()
            .append(NFT_DEFAULT_BATCH_CONTENT_PREFIX)
            .append(index.toDecimalString())
            .append(".json")
            .build();

        deployList.set(
            index,
            BatchDeployDictItem {
                attachTonAmount: ton("0.03"),
                initParams: NftItemInitAtDeployment {
                    ownerAddress: owner,
                    content,
                }.toCell(),
            },
        );
        i += 1;
    }

    return collection.sendBatchDeployNfts(
        deployer.address,
        0,
        deployList,
        { value: ton("0.15"), bounce: true },
    );
}

fun printNftCollectionInfo(collection: NftCollection, label: string) {
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

fun printNftItemInfo(item: NftItem, label: string) {
    val data = item.getNftData();
    println("{} ITEM_ADDRESS={}", label, item.address);
    println("{} INITIALIZED={}", label, data.isInitialized);
    println("{} INDEX={}", label, data.itemIndex);
    println("{} COLLECTION_ADDRESS={}", label, data.collectionAddress);
    println("{} OWNER_ADDRESS={}", label, data.ownerAddress);
    println("{} CONTENT={}", label, data.content);
}
