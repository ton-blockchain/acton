import "@stdlib/gas-payments"
import "@acton/emulation/config"
import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/assert"
import "@acton/testing/expect"
import "@acton/types/transaction"
import "@contracts/errors"
import "@contracts/fees-management"
import "@contracts/messages"
import "@contracts/types"
import "@tests/nft-collection.test"
import "@wrappers/NftCollection.gen"
import "@wrappers/NftItem.gen"

const MIN_EDGE_DELTA = 1

fun activeShardAccount(addr: address): TlbShardAccount {
    val shard = testing.getShardAccount(addr);
    if (shard != null) {
        return shard;
    }
    Assert.fail("Expected TlbShardAccount when setting data");
}

fun setActiveDataCell(addr: address, newData: cell) {
    var updatedShard = activeShardAccount(addr);
    val account = updatedShard.account.load();
    if (account !is TlbAccountInfo) {
        Assert.fail("Expected TlbAccountInfo when setting data");
    }
    var info = account;
    if (info.storage.state !is TlbAccountStateActive) {
        Assert.fail("Expected TlbAccountStateActive when setting data");
    }

    var active = info.storage.state;
    active.stateInit.data = newData;
    info.storage.state = active;
    updatedShard.account = (info as TlbAccount).toCell();
    testing.setShardAccount(addr, updatedShard);
}

fun forceItemUninitialized(item: NftItem, collection: NftCollection) {
    val data = item.getNftData();
    setActiveDataCell(
        item.address,
        NftItemStorageNotInitialized {
            itemIndex: data.itemIndex as uint64,
            collectionAddress: collection.address,
        }.toCell(),
    );
}

fun makeBatchDeployList(
    indexes: array<int>,
    ownerAddress: address,
    content: string,
    attachTonAmount: coins = grams("0.02"),
): map<uint64, BatchDeployDictItem> {
    var deployList: map<uint64, BatchDeployDictItem> = [];
    var i = 0;
    while (i < indexes.size()) {
        deployList.set(
            indexes.get(i),
            BatchDeployDictItem {
                attachTonAmount,
                initParams: NftItemInitAtDeployment { ownerAddress, content }.toCell(),
            },
        );
        i += 1;
    }
    return deployList;
}

fun estimateMessageForwardFee<T>(body: T, dest: address, bounce: bool, value: coins): coins {
    val outgoingMessage = createMessage({
        body,
        dest,
        bounce,
        value,
    });
    val totalForwardFee = outgoingMessage.estimateFeeWithoutSending(SEND_MODE_REGULAR);
    val prices = testing.getConfig().getMsgForwardPrices(BASECHAIN);
    val firstPart = (totalForwardFee * prices.firstFrac) / 65536;
    val remainingForwardFee = totalForwardFee - firstPart;

    // check-disable-next-line deprecated-symbol-use
    return calculateOriginalForwardFee(BASECHAIN, remainingForwardFee);
}

struct OwnershipTransferBody {
    queryId: uint64
    newOwnerAddress: address
    sendExcessesTo: address?
    customPayload: cell?
    forwardTonAmount: coins
    forwardPayload: PayloadInline | PayloadInRef
}

fun OwnershipTransferBody.toCell(self): cell {
    return beginCell()
        .storeUint(0x5fcc3d14, 32)
        .storeAny(self.queryId)
        .storeAny(self.newOwnerAddress)
        .storeAny(self.sendExcessesTo)
        .storeAny(self.customPayload)
        .storeAny(self.forwardTonAmount)
        .storeAny(self.forwardPayload)
        .endCell();
}

fun makeOwnershipTransferBody(
    newOwnerAddress: address,
    sendExcessesTo: address?,
    forwardTonAmount: coins,
    forwardPayload: slice,
): OwnershipTransferBody {
    val (bodyBits, bodyRefs) = OwnershipTransferBody {
        queryId: 0,
        newOwnerAddress,
        sendExcessesTo,
        customPayload: null,
        forwardTonAmount,
        forwardPayload: PayloadInline { value: createEmptySlice() },
    }
    .toCell()
    .beginParse()
    .remainingBitsAndRefsCount();

    val payload: PayloadInline | PayloadInRef = (1023 - bodyBits) >
        forwardPayload.remainingBitsCount() &&
        (4 - bodyRefs) > forwardPayload.remainingRefsCount()
        ? PayloadInline { value: forwardPayload }
        : PayloadInRef { value: forwardPayload.toCell() };

    return {
        queryId: 0,
        newOwnerAddress,
        sendExcessesTo,
        customPayload: null,
        forwardTonAmount,
        forwardPayload: payload,
    };
}

fun calculateMinimalTransferValue(
    itemAddress: address,
    body: OwnershipTransferBody,
    balanceBefore: coins,
): coins {
    var value = 0;
    var previous = -1;
    val bodyCell = body.toCell();

    while (value != previous) {
        previous = value;

        val fwdFee = estimateMessageForwardFee(bodyCell, itemAddress, true, value);
        var required = MIN_GRAMS_FOR_STORAGE - balanceBefore;
        if (required < 0) {
            required = 0;
        }
        if (body.forwardTonAmount != 0) {
            required += body.forwardTonAmount + fwdFee;
        }
        if (body.sendExcessesTo != null) {
            required += fwdFee;
        }

        value = required;
    }

    return value;
}

fun calculateRestAmount(
    balanceBefore: coins,
    msgValue: coins,
    body: OwnershipTransferBody,
    fwdFee: coins,
): coins {
    var restAmount = balanceBefore + msgValue - MIN_GRAMS_FOR_STORAGE;
    if (body.forwardTonAmount != 0) {
        restAmount -= (body.forwardTonAmount + fwdFee);
    }
    if (body.sendExcessesTo != null) {
        restAmount -= fwdFee;
    }
    return restAmount;
}

get fun `test mutation: batch deploy should reject action-list overflow and preserve collection state`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;

    var indexes: array<int> = [];
    var i = 0;
    while (i < 250) {
        indexes.push(nextItemIndex + i);
        i += 1;
    }

    val deployList = makeBatchDeployList(indexes, randomAddress(), "batch-overflow");
    val firstNftAddress = nftCollection.getNftAddressByIndex(nextItemIndex);
    val result = nftCollection.sendBatchDeployNfts(
        deployer.address,
        0,
        deployList,
        { value: grams("20"), bounce: true },
    );

    expect(result).toHaveFailedTx<BatchDeployNfts>({
        from: deployer.address,
        to: nftCollection.address,
        exitCode: Errors.BatchLimitExceeded,
    });
    expect(result).toNotHaveTx({ from: nftCollection.address, deploy: true });
    expect(nftCollection.getCollectionData().nextItemIndex).toEqual(nextItemIndex);
    expect(testing.isDeployed(firstNftAddress)).toBeFalse();
}

get fun `test mutation: batch deploy should report exact failing counter for skipped indexes`() {
    val (nftCollection, deployer, _) = setupTest();
    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    val deployList = makeBatchDeployList(
        [nextItemIndex, nextItemIndex + 2],
        randomAddress(),
        "gap",
    );
    val skippedAddress = nftCollection.getNftAddressByIndex(nextItemIndex);
    val result = nftCollection.sendBatchDeployNfts(
        deployer.address,
        0,
        deployList,
        { value: grams("0.5"), bounce: true },
    );

    expect(result).toHaveFailedTx<BatchDeployNfts>({
        from: deployer.address,
        to: nftCollection.address,
        exitCode: (Errors.NotFromAdmin as int) + 2,
    });
    expect(result).toNotHaveTx({ from: nftCollection.address, deploy: true });
    expect(nftCollection.getCollectionData().nextItemIndex).toEqual(nextItemIndex);
    expect(testing.isDeployed(skippedAddress)).toBeFalse();
}

get fun `test mutation: batch redeploy should not over-increment next item index`() {
    val (nftCollection, deployer, _) = setupTest();
    val owner = testing.treasury("mutation-batch-redeploy-owner");

    nftFixture(nftCollection, owner.address);
    nftFixture(nftCollection, owner.address);

    val nextItemIndex = nftCollection.getCollectionData().nextItemIndex;
    expect(nextItemIndex).toEqual(2);

    val result = nftCollection.sendBatchDeployNfts(
        deployer.address,
        0,
        makeBatchDeployList([0, nextItemIndex], owner.address, "redeploy"),
        { value: grams("0.2"), bounce: true },
    );

    expect(result).toHaveSuccessfulTx<BatchDeployNfts>({
        from: deployer.address,
        to: nftCollection.address,
    });
    expect(nftCollection.getCollectionData().nextItemIndex).toEqual(nextItemIndex + 1);

    val freshNftAddress = nftCollection.getNftAddressByIndex(nextItemIndex);
    expect(result).toHaveSuccessfulDeploy({ from: nftCollection.address, to: freshNftAddress });
}

get fun `test mutation: uninitialized item getter should expose deployment skeleton only`() {
    val (nftCollection, _, _) = setupTest();
    val owner = testing.treasury("mutation-uninitialized-owner");
    val nftItem = nftFixture(nftCollection, owner.address);
    val initialData = nftItem.getNftData();

    forceItemUninitialized(nftItem, nftCollection);

    val data = nftItem.getNftData();
    expect(data.isInitialized).toBeFalse();
    expect(data.itemIndex).toEqual(initialData.itemIndex);
    expect(data.collectionAddress).toEqual(nftCollection.address);
    expect(data.ownerAddress == null).toBeTrue();
    expect(data.content == null).toBeTrue();
}

get fun `test mutation: uninitialized item should reject initialization from non-collection sender`() {
    val (nftCollection, _, _) = setupTest();
    val owner = testing.treasury("mutation-uninitialized-owner");
    val outsider = testing.treasury("mutation-uninitialized-outsider");
    val nftItem = nftFixture(nftCollection, owner.address);

    forceItemUninitialized(nftItem, nftCollection);

    val result = nftItem.sendAny(
        outsider.address,
        NftItemInitAtDeployment {
            ownerAddress: outsider.address,
            content: "forbidden",
        }.toCell(),
        { value: grams("0.05"), bounce: true },
    );

    expect(result).toHaveFailedTx({
        from: outsider.address,
        to: nftItem.address,
        exitCode: Errors.NotFromCollection,
    });

    val data = nftItem.getNftData();
    expect(data.isInitialized).toBeFalse();
    expect(data.collectionAddress).toEqual(nftCollection.address);
    expect(data.ownerAddress == null).toBeTrue();
    expect(data.content == null).toBeTrue();
}

get fun `test mutation: ownership transfer should fail one nanotone below forward-only threshold`() {
    val (nftCollection, _, _) = setupTest();
    val owner = testing.treasury("mutation-threshold-owner");
    val nftItem = nftFixture(nftCollection, owner.address);
    val nextOwner = randomAddress();
    val transferBody = makeOwnershipTransferBody(
        nextOwner,
        null,
        grams("0.01"),
        createEmptySlice(),
    );

    val balanceBefore = testing.getAccountBalance(nftItem.address);
    val minimalValue = calculateMinimalTransferValue(nftItem.address, transferBody, balanceBefore);
    val result = nftItem.sendAskToChangeOwnership(
        owner.address,
        transferBody.queryId,
        transferBody.newOwnerAddress,
        transferBody.sendExcessesTo,
        transferBody.customPayload,
        transferBody.forwardTonAmount,
        transferBody.forwardPayload,
        { value: minimalValue - MIN_EDGE_DELTA, bounce: true },
    );

    expect(result).toHaveFailedTx<AskToChangeOwnership>({
        from: owner.address,
        to: nftItem.address,
        exitCode: Errors.TooSmallRestAmount,
    });
    expect(nftItem.getNftData().ownerAddress!).toEqual(owner.address);
}

get fun `test mutation: ownership transfer with zero forward amount should exhaust rest exactly`() {
    val (nftCollection, _, _) = setupTest();
    val owner = testing.treasury("mutation-zero-forward-owner");
    val nftItem = nftFixture(nftCollection, owner.address);
    val nextOwner = randomAddress();
    val excessesAddress = randomAddress();
    val transferBody = makeOwnershipTransferBody(nextOwner, excessesAddress, 0, createEmptySlice());

    val balanceBefore = testing.getAccountBalance(nftItem.address);
    val exactValue = calculateMinimalTransferValue(nftItem.address, transferBody, balanceBefore);
    val fwdFee = estimateMessageForwardFee(
        transferBody.toCell(),
        nftItem.address,
        true,
        exactValue,
    );
    val expectedRestAmount = calculateRestAmount(balanceBefore, exactValue, transferBody, fwdFee);
    val result = nftItem.sendAskToChangeOwnership(
        owner.address,
        transferBody.queryId,
        transferBody.newOwnerAddress,
        transferBody.sendExcessesTo,
        transferBody.customPayload,
        transferBody.forwardTonAmount,
        transferBody.forwardPayload,
        { value: exactValue, bounce: true },
    );

    expect(expectedRestAmount).toEqual(0);
    expect(result).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: owner.address,
        to: nftItem.address,
    });
    expect(result).toNotHaveTx<NotificationForNewOwner>({ from: nftItem.address, to: nextOwner });
    expect(result).toHaveTx<ReturnExcessesBack>({
        from: nftItem.address,
        to: excessesAddress,
        bounce: false,
        value: expectedRestAmount,
    });
    expect(nftItem.getNftData().ownerAddress!).toEqual(nextOwner);
}

get fun `test mutation: ownership transfer with forward amount should emit exact zero-rest payouts`() {
    val (nftCollection, _, _) = setupTest();
    val owner = testing.treasury("mutation-forward-owner");
    val nftItem = nftFixture(nftCollection, owner.address);
    val nextOwner = randomAddress();
    val excessesAddress = randomAddress();
    val forwardAmount = grams("0.01");
    val transferBody = makeOwnershipTransferBody(
        nextOwner,
        excessesAddress,
        forwardAmount,
        createEmptySlice(),
    );

    val balanceBefore = testing.getAccountBalance(nftItem.address);
    val exactValue = calculateMinimalTransferValue(nftItem.address, transferBody, balanceBefore);
    val fwdFee = estimateMessageForwardFee(
        transferBody.toCell(),
        nftItem.address,
        true,
        exactValue,
    );
    val expectedRestAmount = calculateRestAmount(balanceBefore, exactValue, transferBody, fwdFee);
    val result = nftItem.sendAskToChangeOwnership(
        owner.address,
        transferBody.queryId,
        transferBody.newOwnerAddress,
        transferBody.sendExcessesTo,
        transferBody.customPayload,
        transferBody.forwardTonAmount,
        transferBody.forwardPayload,
        { value: exactValue, bounce: true },
    );

    expect(expectedRestAmount).toEqual(0);
    expect(result).toHaveSuccessfulTx<AskToChangeOwnership>({
        from: owner.address,
        to: nftItem.address,
    });
    expect(result).toHaveTx<NotificationForNewOwner>({
        from: nftItem.address,
        to: nextOwner,
        bounce: false,
        value: forwardAmount,
    });
    expect(result).toHaveTx<ReturnExcessesBack>({
        from: nftItem.address,
        to: excessesAddress,
        bounce: false,
        value: expectedRestAmount,
    });
    expect(nftItem.getNftData().ownerAddress!).toEqual(nextOwner);
}
