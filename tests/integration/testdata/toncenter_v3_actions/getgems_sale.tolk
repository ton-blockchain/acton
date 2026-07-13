import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/scripts"
import "../../lib/io"
import "../types/getgems_nft_fixprice_sale_v3.types"
import "../types/nft_v1_collection.types"
import "../types/nft_v1_item.types"
import "../wrappers/GetgemsNftFixpriceSaleV3.gen"
import "../wrappers/NftV1Collection.gen"
import "../wrappers/NftV1Item.gen"

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
    val sale = GetgemsNftFixpriceSaleV3.fromStorage({
        isComplete: false,
        createdAt: 1700000000,
        marketplaceAddress: owner.address,
        nftAddress: item.address,
        nftOwnerAddress: null,
        fullPrice: ton("2"),
        fees: GetgemsNftFixpriceSaleV3Fees {
            marketplaceFeeAddress: owner.address,
            marketplaceFee: ton("0.1"),
            royaltyAddress: owner.address,
            royaltyAmount: ton("0.2"),
        }.toCell(),
        canDeployByExternal: false,
    });
    waitForTransaction(sale.deploy(owner.address, { value: ton("1"), bounce: false }));
    waitForTransaction(item.sendNftV1Transfer(
        owner.address,
        202,
        sale.address,
        owner.address,
        null,
        ton("0.05"),
        inlineEmptyForwardPayload(),
        { value: ton("0.3"), bounce: false },
    ));

    println("OWNER={}", owner.address);
    println("NFT_COLLECTION={}", collection.address);
    println("NFT_ITEM={}", item.address);
    println("NFT_SALE={}", sale.address);
}
