import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../types/nft_v1_collection.types"
import "../types/nft_v1_item.types"
import "../wrappers/NftV1Collection.gen"
import "../wrappers/NftV1Item.gen"

const RECIPIENT = "0:1111111111111111111111111111111111111111111111111111111111111111"

fun inlineEmptyForwardPayload(): RemainingBitsAndRefs {
    return beginCell().storeUint(0, 1).endCell().beginParse();
}

fun waitForTransaction(result: SendResultList) {
    if (result.waitForFirstTransaction() == null) {
        throw 1000;
    }
}

fun main() {
    val owner = scripts.wallet("deployer");
    val recipient = address(RECIPIENT);
    val collection = NftV1Collection.fromStorage({
        adminAddress: owner.address,
        nextItemIndex: 0,
        content: NftV1CollectionContent {
            collectionMetadata: createEmptyCell(),
            commonContent: createEmptyCell(),
        }.toCell(),
        nftItemCode: build("nftv1item"),
        royaltyParams: NftV1RoyaltyParams {
            numerator: 5,
            denominator: 100,
            royaltyAddress: owner.address,
        }.toCell(),
    });
    waitForTransaction(collection.deploy(owner.address, { value: ton("1"), bounce: false }));
    waitForTransaction(collection.sendNftV1DeployItem(
        owner.address,
        200,
        0,
        ton("0.2"),
        {
            ownerAddress: owner.address,
            content: createEmptyCell(),
        },
        { value: ton("0.4"), bounce: false },
    ));

    val item = NftV1Item.fromAddress(collection.getNftAddressByIndex(0));
    waitForTransaction(item.sendNftV1Transfer(
        owner.address,
        201,
        recipient,
        owner.address,
        null,
        ton("0.01"),
        inlineEmptyForwardPayload(),
        { value: ton("0.2"), bounce: false },
    ));
    waitForTransaction(item.sendNftV1Transfer(
        owner.address,
        203,
        owner.address,
        owner.address,
        null,
        ton("0.01"),
        inlineEmptyForwardPayload(),
        { value: ton("0.2"), bounce: false },
    ));

    println("OWNER={}", owner.address);
    println("RECIPIENT={}", recipient);
    println("NFT_COLLECTION={}", collection.address);
    println("NFT_ITEM={}", item.address);
}
