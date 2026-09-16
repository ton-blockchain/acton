import "@acton/build"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/messages"
import "@contracts/types"
import "@wrappers/NftCollection.gen"
import "@wrappers/NftItem.gen"
import "@wrappers/utils"

const NFT_E2E_NOW = 1000
const NFT_E2E_ROYALTY_QUERY_ID = 7301
const NFT_E2E_DEPLOY_QUERY_ID = 7302
const NFT_E2E_TRANSFER_QUERY_ID = 7303
const NFT_E2E_STATIC_QUERY_ID = 7304
const NFT_E2E_BATCH_QUERY_ID = 7305
const NFT_E2E_CHANGE_ADMIN_QUERY_ID = 7306

const NFT_E2E_DEPLOY_ATTACH = grams("0.02")
const NFT_E2E_SINGLE_DEPLOY_VALUE = grams("0.05")
const NFT_E2E_TRANSFER_VALUE = grams("0.12")
const NFT_E2E_FORWARD_AMOUNT = grams("0.01")
const NFT_E2E_BATCH_COUNT = 4
const NFT_E2E_FORWARD_MARKER = 0x4e465431

const NFT_E2E_COMMON_CONTENT = "ipfs://nft-e2e/"
const NFT_E2E_SINGLE_CONTENT = "single.json"
const NFT_E2E_BATCH_CONTENT = "batch.json"

struct NftE2eContext {
    collection: NftCollection
    deployer: Treasury
    royaltyReader: Treasury
    royaltyRecipient: Treasury
    itemOwner: Treasury
    itemReceiver: Treasury
    excessesReceiver: Treasury
    staticReader: Treasury
    batchOwner: Treasury
    newAdmin: Treasury
    royaltyParams: RoyaltyParams
}

struct NftE2eItemFixture {
    ctx: NftE2eContext
    item: NftItem
    itemIndex: int
}

fun makeE2eCollectionMetadata(): cell {
    return beginCell().storeSlice("nft-e2e-collection".beginParse()).endCell();
}

fun deployCollectionStage(): NftE2eContext {
    testing.setNow(NFT_E2E_NOW);

    val deployer = testing.treasury("nft-e2e-deployer");
    val royaltyReader = testing.treasury("nft-e2e-royalty-reader");
    val royaltyRecipient = testing.treasury("nft-e2e-royalty-recipient");
    val itemOwner = testing.treasury("nft-e2e-item-owner");
    val itemReceiver = testing.treasury("nft-e2e-item-receiver");
    val excessesReceiver = testing.treasury("nft-e2e-excesses-receiver");
    val staticReader = testing.treasury("nft-e2e-static-reader");
    val batchOwner = testing.treasury("nft-e2e-batch-owner");
    val newAdmin = testing.treasury("nft-e2e-new-admin");

    val royaltyParams = RoyaltyParams {
        numerator: 5,
        denominator: 100,
        royaltyAddress: royaltyRecipient.address,
    };
    val collection = NftCollection.fromStorage({
        adminAddress: deployer.address,
        nextItemIndex: 0,
        content: CollectionContent {
            collectionMetadata: makeE2eCollectionMetadata(),
            commonContent: NFT_E2E_COMMON_CONTENT,
        }.toCell(),
        nftItemCode: build("NftItem"),
        royaltyParams: royaltyParams.toCell(),
    });

    val deploy = collection.deploy(deployer.address, { value: grams("1"), bounce: false });
    expect(deploy).toHaveSuccessfulDeploy({ to: collection.address });

    val data = collection.getCollectionData();
    expect(data.nextItemIndex).toEqual(0);
    expect(data.adminAddress).toEqual(deployer.address);

    return {
        collection,
        deployer,
        royaltyReader,
        royaltyRecipient,
        itemOwner,
        itemReceiver,
        excessesReceiver,
        staticReader,
        batchOwner,
        newAdmin,
        royaltyParams,
    };
}

fun reportRoyaltyParamsStage(ctx: NftE2eContext) {
    val report = ctx
        .collection
        .sendRequestRoyaltyParams(
            ctx.royaltyReader.address,
            NFT_E2E_ROYALTY_QUERY_ID,
            { value: grams("0.05"), bounce: true },
        );

    expect(report).toHaveSuccessfulTx<RequestRoyaltyParams>({
        from: ctx.royaltyReader.address,
        to: ctx.collection.address,
    });
    expect(report).toHaveTx<ResponseRoyaltyParams>({
        from: ctx.collection.address,
        to: ctx.royaltyReader.address,
        bounce: false,
        body: ResponseRoyaltyParams {
            queryId: NFT_E2E_ROYALTY_QUERY_ID,
            royaltyParams: ctx.royaltyParams,
        }.toCell(),
    });
    expect(ctx.collection.royaltyParams()).toEqual(ctx.royaltyParams);
}

fun deploySingleNftStage(ctx: NftE2eContext): NftE2eItemFixture {
    val nextItemIndex = ctx.collection.getCollectionData().nextItemIndex;
    expect(nextItemIndex).toEqual(0);

    val deploy = ctx
        .collection
        .sendDeployNft(
            ctx.deployer.address,
            NFT_E2E_DEPLOY_QUERY_ID,
            nextItemIndex,
            NFT_E2E_DEPLOY_ATTACH,
            { ownerAddress: ctx.itemOwner.address, content: NFT_E2E_SINGLE_CONTENT },
            { value: NFT_E2E_SINGLE_DEPLOY_VALUE, bounce: true },
        );
    expect(deploy).toHaveSuccessfulTx<DeployNft>({
        from: ctx.deployer.address,
        to: ctx.collection.address,
    });

    val nftAddress = ctx.collection.getNftAddressByIndex(nextItemIndex);
    expect(deploy).toHaveSuccessfulDeploy({
        from: ctx.collection.address,
        to: nftAddress,
    });

    val item = NftItem.fromAddress(nftAddress);
    val data = item.getNftData();
    expect(data.isInitialized).toBeTrue();
    expect(data.itemIndex).toEqual(nextItemIndex);
    expect(data.ownerAddress!).toEqual(ctx.itemOwner.address);
    expect(data.collectionAddress).toEqual(ctx.collection.address);
    expect(data.content).toEqual(NFT_E2E_SINGLE_CONTENT);
    expect(ctx.collection.getCollectionData().nextItemIndex).toEqual(nextItemIndex + 1);

    return {
        ctx,
        item,
        itemIndex: nextItemIndex,
    };
}

fun transferNftStage(fixture: NftE2eItemFixture) {
    val forwardPayload = beginCell()
        .storeUint(NFT_E2E_FORWARD_MARKER, 32)
        .storeSlice("nft-e2e-transfer".beginParse())
        .endCell();
    val transfer = fixture
        .item
        .sendTransferPayloadInRef(
            fixture.ctx.itemOwner.address,
            NFT_E2E_TRANSFER_QUERY_ID,
            fixture.ctx.itemReceiver.address,
            fixture.ctx.excessesReceiver.address,
            null,
            NFT_E2E_FORWARD_AMOUNT,
            forwardPayload,
            { value: NFT_E2E_TRANSFER_VALUE, bounce: true },
        );

    expect(transfer).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: fixture.ctx.itemOwner.address,
        to: fixture.item.address,
    });
    expect(transfer).toHaveTx<NotificationForNewOwner>({
        from: fixture.item.address,
        to: fixture.ctx.itemReceiver.address,
        value: NFT_E2E_FORWARD_AMOUNT,
        bounce: false,
    });
    expect(transfer).toHaveTx<ReturnExcessesBack>({
        from: fixture.item.address,
        to: fixture.ctx.excessesReceiver.address,
        bounce: false,
    });

    val data = fixture.item.getNftData();
    expect(data.ownerAddress!).toEqual(fixture.ctx.itemReceiver.address);
}

fun requestStaticDataStage(fixture: NftE2eItemFixture) {
    val data = fixture.item.getNftData();
    val report = fixture
        .item
        .sendRequestStaticData(
            fixture.ctx.staticReader.address,
            NFT_E2E_STATIC_QUERY_ID,
            { value: grams("0.05"), bounce: true },
        );

    expect(report).toHaveSuccessfulTx<RequestStaticData>({
        from: fixture.ctx.staticReader.address,
        to: fixture.item.address,
    });
    expect(report).toHaveTx<ResponseStaticData>({
        from: fixture.item.address,
        to: fixture.ctx.staticReader.address,
        bounce: false,
        body: ResponseStaticData {
            queryId: NFT_E2E_STATIC_QUERY_ID,
            itemIndex: data.itemIndex,
            collectionAddress: data.collectionAddress,
        }.toCell(),
    });
}

fun batchDeployAndChangeAdminStage(ctx: NftE2eContext) {
    val firstBatchIndex = ctx.collection.getCollectionData().nextItemIndex;
    expect(firstBatchIndex).toEqual(1);

    var deployList: map<uint64, BatchDeployDictItem> = [];
    var i = 0;
    while (i < NFT_E2E_BATCH_COUNT) {
        deployList.set(
            firstBatchIndex + i,
            BatchDeployDictItem {
                attachTonAmount: NFT_E2E_DEPLOY_ATTACH,
                initParams: NftItemInitAtDeployment {
                    ownerAddress: ctx.batchOwner.address,
                    content: NFT_E2E_BATCH_CONTENT,
                }.toCell(),
            },
        );
        i += 1;
    }

    val batch = ctx
        .collection
        .sendBatchDeployNfts(
            ctx.deployer.address,
            NFT_E2E_BATCH_QUERY_ID,
            deployList,
            { value: grams("0.5"), bounce: true },
        );
    expect(batch).toHaveSuccessfulTx<BatchDeployNfts>({
        from: ctx.deployer.address,
        to: ctx.collection.address,
    });

    val firstAddress = ctx.collection.getNftAddressByIndex(firstBatchIndex);
    val lastIndex = firstBatchIndex + NFT_E2E_BATCH_COUNT - 1;
    val lastAddress = ctx.collection.getNftAddressByIndex(lastIndex);
    expect(batch).toHaveSuccessfulDeploy({ from: ctx.collection.address, to: firstAddress });
    expect(batch).toHaveSuccessfulDeploy({ from: ctx.collection.address, to: lastAddress });

    val firstItem = NftItem.fromAddress(firstAddress);
    val lastItem = NftItem.fromAddress(lastAddress);
    expect(firstItem.getNftData().ownerAddress!).toEqual(ctx.batchOwner.address);
    expect(lastItem.getNftData().ownerAddress!).toEqual(ctx.batchOwner.address);
    expect(ctx.collection.getCollectionData().nextItemIndex).toEqual(
        firstBatchIndex + NFT_E2E_BATCH_COUNT,
    );

    val changeAdmin = ctx
        .collection
        .sendChangeCollectionAdmin(
            ctx.deployer.address,
            NFT_E2E_CHANGE_ADMIN_QUERY_ID,
            ctx.newAdmin.address,
            { value: grams("0.05"), bounce: true },
        );
    expect(changeAdmin).toHaveSuccessfulTx<ChangeCollectionAdmin>({
        from: ctx.deployer.address,
        to: ctx.collection.address,
    });
    expect(ctx.collection.getCollectionData().adminAddress).toEqual(ctx.newAdmin.address);
}

get fun `test nft e2e bench: stage 1 deploy collection`() {
    deployCollectionStage();
}

get fun `test nft e2e bench: stage 2 report royalty params`() {
    val ctx = deployCollectionStage();
    reportRoyaltyParamsStage(ctx);
}

get fun `test nft e2e bench: stage 3 deploy nft item`() {
    val ctx = deployCollectionStage();
    reportRoyaltyParamsStage(ctx);
    deploySingleNftStage(ctx);
}

get fun `test nft e2e bench: stage 4 transfer nft item and request static data`() {
    val ctx = deployCollectionStage();
    reportRoyaltyParamsStage(ctx);
    val fixture = deploySingleNftStage(ctx);
    transferNftStage(fixture);
    requestStaticDataStage(fixture);
}

get fun `test nft e2e bench: stage 5 full collection and item flow`() {
    val ctx = deployCollectionStage();
    reportRoyaltyParamsStage(ctx);
    val fixture = deploySingleNftStage(ctx);
    transferNftStage(fixture);
    requestStaticDataStage(fixture);
    batchDeployAndChangeAdminStage(ctx);
}
