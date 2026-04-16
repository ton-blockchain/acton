import "@acton/build/build"
import "@acton/emulation/network"
import "@acton/testing/expect"
import "@acton/testing/transaction_expect"
import "../contracts/messages"
import "../contracts/types"
import "../wrappers/NftCollectionContract"
import "../wrappers/NftItemContract"

struct Royalty {
    numerator: int
    denominator: int
    royaltyAddress: address
}

get fun `test nft: report royalty params`() {
    val (nftCollection, _, royaltyParams) = setupTest();
    val randomTreasury = net.treasury("random treasury");
    val queryId = 42;
    val result = nftCollection.sendRequestRoyaltyParams(
        randomTreasury.address,
        queryId,
        { value: ton("0.05") },
    );

    expect(result).toHaveSuccessfulTx<RequestRoyaltyParams>({
        from: randomTreasury.address,
        to: nftCollection.address,
    });

    expect(result).toHaveTx<ResponseRoyaltyParams>({
        from: nftCollection.address,
        to: randomTreasury.address,
        bounce: false,
        body: ResponseRoyaltyParams { queryId, royaltyParams }.toCell(),
    });
}

fun shouldNotAllowAnyOtherOpcodeNotFromOwner(opcode: int) {
    val randomTreasury = net.treasury("random-treasury");

    val (nftCollection, _, _) = setupTest();

    val result = nftCollection.sendAny(
        randomTreasury.address,
        beginCell().storeUint(opcode, 32).storeUint(42, 64).endCell(),
        { value: ton("0.05") },
    );

    val exitCode = opcode == 0x042 ? 0xFFFF : 401;

    expect(result).toHaveTx({
        from: randomTreasury.address,
        to: nftCollection.address,
        opcode,
        success: false,
        // exitCode: 401,       // this changed against FunC: on invalid opcode, we always return 0xFFFF
        exitCode,
    });
}

get fun `test nft: should not allow any other opcode not from owner`() {
    shouldNotAllowAnyOtherOpcodeNotFromOwner(1);
    shouldNotAllowAnyOtherOpcodeNotFromOwner(2);
    shouldNotAllowAnyOtherOpcodeNotFromOwner(3);
    shouldNotAllowAnyOtherOpcodeNotFromOwner(0x42);
}

fun nftFixture(nftCollection: NftCollectionContract, nftOwnerAddress: address) {
    val deployer = net.treasury("deployer");
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    val nftDeployResult = nftCollection.sendDeployNft(
        deployer.address,
        0,
        nextItemIndex,
        ton("0.02"),
        { ownerAddress: nftOwnerAddress, content: "" },
        { value: ton("0.05"), bounce: true },
    );

    expect(nftDeployResult).toHaveSuccessfulTx({
        from: deployer.address,
        to: nftCollection.address,
    });

    val nftAddress = nftCollection.getNftAddressByIndex(nextItemIndex);

    expect(nftDeployResult).toHaveSuccessfulDeploy({ from: nftCollection.address, to: nftAddress });

    return NftItemContract.fromAddress(nftAddress);
}

/// Initializes the test environment, creating a fresh instance of the contract.
/// Returns the contract wrapper and two treasury accounts (`deployer` and `not_deployer`).
fun setupTest() {
    val deployer = net.treasury("deployer");

    val royaltyParams = RoyaltyParams {
        numerator: 16,
        denominator: 2,
        royaltyAddress: net.randomAddress(),
    };

    // Initialize and deploy the contract with default values
    val nftCollection = NftCollectionContract.fromStorage({
        adminAddress: deployer.address,
        nextItemIndex: 0,
        content: CollectionContent {
            collectionMetadata: createEmptyCell(),
            commonContent: "",
        }.toCell(),
        nftItemCode: build("NftItem"),
        royaltyParams: royaltyParams.toCell(),
    });

    val deployRes = nftCollection.deploy(deployer.address, { value: ton("0.05"), bounce: false });

    expect(deployRes).toHaveSuccessfulDeploy({ to: nftCollection.address });

    return (nftCollection, deployer, royaltyParams);
}

get fun `test nft: should deploy nft`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    val nftOwnerAddress = net.randomAddress();
    val content: string = "Content";

    val nftDeployResult = nftCollection.sendDeployNft(
        deployer.address,
        0, // queryId
        nextItemIndex,
        ton("0.02"),
        { ownerAddress: nftOwnerAddress, content },
        { value: ton("0.05"), bounce: true },
    );

    expect(nftDeployResult).toHaveSuccessfulTx<DeployNft>({
        from: deployer.address,
        to: nftCollection.address,
    });

    val nftAddress = nftCollection.getNftAddressByIndex(nextItemIndex);

    expect(nftDeployResult).toHaveSuccessfulDeploy({
        from: nftCollection.address,
        to: nftAddress,
        // TODO initCode: nftItemCode,
    });

    val nftItem = NftItemContract.fromAddress(nftAddress);
    val data = nftItem.getNftData();

    expect(data.isInitialized).toBeTrue();
    expect(data.itemIndex).toEqual(nextItemIndex);
    expect(data.ownerAddress!).toEqual(nftOwnerAddress);
    expect(data.collectionAddress).toEqual(nftCollection.address);
    expect(data.content).toEqual(content);
}

get fun `test nft: should not deploy nft with invalid item index`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;

    // Trying to deploy with invalid index (nextItemIndex + 1 instead of nextItemIndex)
    val nftDeployResult = nftCollection.sendDeployNft(
        deployer.address,
        0,                 // queryId
        nextItemIndex + 1, // invalid index!
        ton("0.02"),
        { ownerAddress: deployer.address, content: "" },
        { value: ton("0.05"), bounce: true },
    );

    expect(nftDeployResult).toHaveFailedTx<DeployNft>({
        from: deployer.address,
        to: nftCollection.address,
        exitCode: 402,
    });
}

get fun `test nft: should increment item index if last`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;

    val nftDeployResult = nftCollection.sendDeployNft(
        deployer.address,
        0, // queryId
        nextItemIndex,
        ton("0.02"),
        { ownerAddress: deployer.address, content: "" },
        { value: ton("0.05"), bounce: true },
    );

    expect(nftDeployResult).toHaveSuccessfulTx<DeployNft>({
        from: deployer.address,
        to: nftCollection.address,
    });

    val newNextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    expect(newNextItemIndex).toEqual(nextItemIndex + 1);
}

get fun `test nft: should not increment item index if not last`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;

    // Deploy first NFT at index 0
    nftCollection.sendDeployNft(
        deployer.address,
        0,
        nextItemIndex,
        ton("0.02"),
        { ownerAddress: deployer.address, content: "" },
        { value: ton("0.05"), bounce: true },
    );

    val newNextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    expect(newNextItemIndex).toEqual(nextItemIndex + 1);

    // Deploy again at index 0 (re-deploy, not last)
    val nftDeployResult = nftCollection.sendDeployNft(
        deployer.address,
        0,
        0, // index 0, not the last one
        ton("0.02"),
        { ownerAddress: deployer.address, content: "" },
        { value: ton("0.05"), bounce: true },
    );

    expect(nftDeployResult).toHaveSuccessfulTx<DeployNft>({
        from: deployer.address,
        to: nftCollection.address,
    });

    // Index should NOT have changed
    val newNextItemIndexAfterSecondDeploy = nftCollection.getCollectionData().nextItemIndex;
    expect(newNextItemIndexAfterSecondDeploy).toEqual(newNextItemIndex);
}

get fun `test nft: should deploy some nfts via batch deploy`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    val nftOwnerAddress = net.randomAddress();
    val content: string = "Content";

    var deployList: map<uint64, BatchDeployDictItem> = [];
    val indexes: array<int> = [nextItemIndex, nextItemIndex + 1, nextItemIndex + 2];

    var i = 0;
    while (i < 3) {
        deployList.set(
            indexes.get(i),
            BatchDeployDictItem {
                attachTonAmount: ton("0.02"),
                initParams: NftItemInitAtDeployment {
                    ownerAddress: nftOwnerAddress,
                    content,
                }.toCell(),
            },
        );
        i += 1;
    }

    val nftDeployResult = nftCollection.sendBatchDeployNfts(
        deployer.address,
        0, // queryId
        deployList,
        { value: ton("0.5"), bounce: true },
    );

    expect(nftDeployResult).toHaveSuccessfulTx<BatchDeployNfts>({
        from: deployer.address,
        to: nftCollection.address,
    });

    // Check all NFTs were deployed
    i = 0;
    while (i < 3) {
        val index = indexes.get(i);
        val nftAddress = nftCollection.getNftAddressByIndex(index);

        expect(nftDeployResult).toHaveSuccessfulDeploy({
            from: nftCollection.address,
            to: nftAddress,
        });

        val nftItem = NftItemContract.fromAddress(nftAddress);
        val data = nftItem.getNftData();

        expect(data.isInitialized).toBeTrue();
        expect(data.itemIndex).toEqual(index);
        expect(data.ownerAddress!).toEqual(nftOwnerAddress);
        expect(data.collectionAddress).toEqual(nftCollection.address);
        expect(data.content).toEqual(content);

        i += 1;
    }
}

get fun `test nft: should change owner`() {
    val (nftCollection, deployer, _) = setupTest();
    val oldOwnerAddress = nftCollection.getCollectionData().adminAddress;
    val newOwner = net.treasury("new-owner");

    val result = nftCollection.sendChangeCollectionAdmin(
        deployer.address,
        0, // queryId
        newOwner.address,
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<ChangeCollectionAdmin>({
        from: deployer.address,
        to: nftCollection.address,
    });

    val newOwnerAddress = nftCollection.getCollectionData().adminAddress;

    expect(oldOwnerAddress).toNotEqual(newOwnerAddress);
    expect(newOwnerAddress).toEqual(newOwner.address);
}
