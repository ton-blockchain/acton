import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"
import "@contracts/errors"
import "@contracts/messages"
import "@tests/nft-collection.test"
import "@wrappers/NftCollection.gen"
import "@wrappers/NftItem.gen"
import "@wrappers/utils"

get fun `test nft: should transfer ownership`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val nftReceiverAddress = randomAddress();

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val ownerBeforeTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerBeforeTransfer!).toEqual(nftOwner.address);

    val result = nftItem.sendTransferPayloadInline(
        nftOwner.address,
        0,
        nftReceiverAddress,
        null,
        null,
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: nftOwner.address,
        to: nftItem.address,
    });

    // Should NOT have notification to receiver (forwardAmount = 0)
    expect(result).toNotHaveTx<NotificationForNewOwner>({
        from: nftItem.address,
        to: nftReceiverAddress,
    });

    val ownerAfterTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerAfterTransfer!).toEqual(nftReceiverAddress);
}

get fun `test nft: should transfer ownership and return excesses`() {
    val (nftCollection, _, _) = setupTest();

    val nftOwner = testing.treasury("nft-owner");
    val nftReceiverAddress = randomAddress();
    val excessesAddress = randomAddress();

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val ownerBeforeTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerBeforeTransfer!).toEqual(nftOwner.address);

    val result = nftItem.sendTransferPayloadInline(
        nftOwner.address,
        0,
        nftReceiverAddress,
        excessesAddress,
        null,
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: nftOwner.address,
        to: nftItem.address,
    });

    expect(result).toHaveTx<ReturnExcessesBack>({
        from: nftItem.address,
        to: excessesAddress,
        bounce: false,
    });

    val ownerAfterTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerAfterTransfer!).toEqual(nftReceiverAddress);
}

get fun `test nft: should not transfer ownership not from owner`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val notOwner = testing.treasury("not-owner");
    val nftReceiverAddress = randomAddress();

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val ownerBeforeTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerBeforeTransfer!).toEqual(nftOwner.address);

    val result = nftItem.sendTransferPayloadInline(
        notOwner.address,
        0,
        nftReceiverAddress,
        null,
        null,
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveFailedTx<AskToChangeOwnership>({
        from: notOwner.address,
        to: nftItem.address,
        exitCode: Errors.NotFromOwner as int32,
    });

    // Owner should NOT have changed
    val ownerAfterTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerAfterTransfer!).toEqual(ownerBeforeTransfer!);
}

get fun `test nft: should transfer ownership with notification`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val nftReceiverAddress = randomAddress();

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val ownerBeforeTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerBeforeTransfer!).toEqual(nftOwner.address);

    val forwardAmount = ton("0.01");
    val forwardBody = beginCell().storeUint(0, 32).storeString("Hello!").endCell();
    val queryId = 42;

    val result = nftItem.sendTransferPayloadInRef(
        nftOwner.address,
        queryId,
        nftReceiverAddress,
        null,
        null,
        forwardAmount,
        forwardBody,
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: nftOwner.address,
        to: nftItem.address,
    });

    expect(result).toHaveTx<NotificationForNewOwner>({
        from: nftItem.address,
        to: nftReceiverAddress,
        value: forwardAmount,
        bounce: false,
        body: NotificationForNewOwner {
            queryId,
            oldOwnerAddress: nftOwner.address,
            payload: PayloadInRef { value: forwardBody as Cell<RemainingBitsAndRefs> },
        }.toCell(),
    });

    val ownerAfterTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerAfterTransfer!).toEqual(nftReceiverAddress);
}

get fun `test nft: should get static data`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val randomTreasury = testing.treasury("random-treasury");

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val queryId = 42;
    val nftData = nftItem.getNftData();

    val result = nftItem.sendRequestStaticData(
        randomTreasury.address,
        queryId,
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<RequestStaticData>({
        from: randomTreasury.address,
        to: nftItem.address,
    });

    expect(result).toHaveTx<ResponseStaticData>({
        from: nftItem.address,
        to: randomTreasury.address,
        bounce: false,
        body: ResponseStaticData {
            queryId,
            itemIndex: nftData.itemIndex,
            collectionAddress: nftData.collectionAddress,
        }.toCell(),
    });
}

get fun `test nft: should fail when receiver in different workchain`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val nftReceiverAddress = randomAddress("random_user", -1);

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val result = nftItem.sendTransferPayloadInline(
        nftOwner.address,
        0,
        nftReceiverAddress,
        null,
        null,
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveFailedTx<AskToChangeOwnership>({
        from: nftOwner.address,
        to: nftItem.address,
        exitCode: Errors.InvalidWorkchain as int32,
    });
}

get fun `test nft: should fail when op not on list`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val randomTreasury = testing.treasury("random-treasury");

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val queryId = 42;
    val invalidOpcode = 0x123459;

    val result = nftItem.sendAny(
        randomTreasury.address,
        beginCell().storeUint(invalidOpcode, 32).storeUint(queryId, 64).endCell(),
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveTx({
        from: randomTreasury.address,
        to: nftItem.address,
        exitCode: 0xFFFF,
    });
}

get fun `test nft: should fail when response address in different workchain`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val nftReceiverAddress = randomAddress();
    val excessesAddress = randomAddress("excesses", MASTERCHAIN);

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val result = nftItem.sendTransferPayloadInline(
        nftOwner.address,
        0,
        nftReceiverAddress,
        excessesAddress,
        null,
        0,
        createEmptySlice(),
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveFailedTx<AskToChangeOwnership>({
        from: nftOwner.address,
        to: nftItem.address,
        exitCode: Errors.InvalidWorkchain as int32,
    });
}

get fun `test nft: should transfer ownership with forward payload in slice`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");
    val nftReceiverAddress = randomAddress();

    val nftItem = nftFixture(nftCollection, nftOwner.address);
    val queryId = 42;
    val forwardAmount = ton("0.01");

    // forward_payload as inline slice (not ref)
    val forwardPayloadSlice = beginCell().storeUint(0xDEADBEEF, 32).toSlice();

    val result = nftItem.sendTransferPayloadInline(
        nftOwner.address,
        queryId,
        nftReceiverAddress,
        null,
        null,
        forwardAmount,
        forwardPayloadSlice,
        { value: ton("0.05"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: nftOwner.address,
        to: nftItem.address,
    });

    expect(result).toHaveTx<NotificationForNewOwner>({
        from: nftItem.address,
        to: nftReceiverAddress,
        bounce: false,
    });

    val ownerAfterTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerAfterTransfer!).toEqual(nftReceiverAddress);
}

get fun `test nft: should fail with incomplete transfer body`() {
    val (nftCollection, _, _) = setupTest();
    val nftOwner = testing.treasury("nft-owner");

    val nftItem = nftFixture(nftCollection, nftOwner.address);

    val ownerBeforeTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerBeforeTransfer!).toEqual(nftOwner.address);

    // Send transfer op with only queryId (missing new_owner and rest)
    val body = beginCell()
        .storeUint(0x5fcc3d14, 32) // op::transfer
        .storeUint(42, 64) // queryId
        // missing new_owner and rest!
        .endCell();

    val result = nftItem.sendAny(nftOwner.address, body, { value: ton("0.05"), bounce: true });

    expect(result).toHaveFailedTx({
        from: nftOwner.address,
        to: nftItem.address,
        body,
        exitCode: 9, // cell underflow
    });

    val ownerAfterTransfer = nftItem.getNftData().ownerAddress;
    expect(ownerAfterTransfer!).toEqual(ownerBeforeTransfer!);
}

get fun `test nft: bench should deploy 100 nfts via batch deploy`() {
    val (nftCollection, deployer, _) = setupTest();
    val ownerAddress = randomAddress();
    val content = "Content";

    var deployList: map<uint64, BatchDeployDictItem> = [];

    var i = 0;
    while (i < 100) {
        deployList.set(
            i,
            BatchDeployDictItem {
                attachTonAmount: ton("0.02"),
                initParams: NftItemInitAtDeployment { ownerAddress, content }.toCell(),
            },
        );
        i += 1;
    }

    val nftDeployResult = nftCollection.sendBatchDeployNfts(
        deployer.address,
        0,
        deployList,
        {
            value: ton("5"), // more TON for 100 NFTs
            bounce: true,
        },
    );

    expect(nftDeployResult).toHaveSuccessfulTx<BatchDeployNfts>({
        from: deployer.address,
        to: nftCollection.address,
    });

    // Check first and last NFT were deployed
    val firstNftAddress = nftCollection.getNftAddressByIndex(0);
    expect(nftDeployResult).toHaveSuccessfulDeploy({
        from: nftCollection.address,
        to: firstNftAddress,
    });

    val lastNftAddress = nftCollection.getNftAddressByIndex(99);
    expect(nftDeployResult).toHaveSuccessfulDeploy({
        from: nftCollection.address,
        to: lastNftAddress,
    });

    // Verify collection state
    val newNextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    expect(newNextItemIndex).toEqual(100);
}
