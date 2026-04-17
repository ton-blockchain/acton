import "@acton/build/build"
import "@acton/emulation/network"
import "@acton/testing/assert"
import "@contracts/messages"
import "@contracts/types"

struct NftCollectionContract {
    address: address
    init: ContractState? = null
}

/// Creates a contract wrapper instance from the storage data
fun NftCollectionContract.fromStorage(storage: NftCollectionStorage): NftCollectionContract {
    val init = ContractState {
        code: build("NftCollection"),
        data: storage.toCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return NftCollectionContract { address, init };
}

/// Creates a contract wrapper instance from the address
fun NftCollectionContract.fromAddress(address: address): NftCollectionContract {
    return NftCollectionContract { address };
}

/// Deploys the contract to the blockchain
fun NftCollectionContract.deploy(self, from: address, config: SendParams = {}): SendResultList {
    if (self.init == null) {
        Assert.fail("Cannot deploy a contract created with 'fromAddress' because it lacks state init for deployment");
    }
    val deployMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: { stateInit: self.init },
    });
    return net.send(from, deployMessage);
}

fun NftCollectionContract.sendDeployNft(
    self,
    from: address,
    queryId: uint64,
    itemIndex: uint64,
    attachTonAmount: coins,
    initParams: NftItemInitAtDeployment,
    config: SendParams = {},
): SendResultList {
    val deployNftMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: DeployNft {
            queryId,
            itemIndex,
            attachTonAmount,
            initParams: initParams.toCell(),
        },
    });
    return net.send(from, deployNftMessage);
}

fun NftCollectionContract.sendRequestRoyaltyParams(
    self,
    from: address,
    queryId: uint64,
    config: SendParams = {},
): SendResultList {
    val requestRoyaltyParamsMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: RequestRoyaltyParams { queryId },
    });
    return net.send(from, requestRoyaltyParamsMessage);
}

fun NftCollectionContract.sendBatchDeployNfts(
    self,
    from: address,
    queryId: uint64,
    deployList: map<uint64, BatchDeployDictItem>,
    config: SendParams = {},
): SendResultList {
    val batchDeployNftsMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: BatchDeployNfts { queryId, deployList },
    });
    return net.send(from, batchDeployNftsMessage);
}

fun NftCollectionContract.sendChangeCollectionAdmin(
    self,
    from: address,
    queryId: uint64,
    newAdminAddress: address,
    config: SendParams = {},
): SendResultList {
    val changeCollectionAdminMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body: ChangeCollectionAdmin { queryId, newAdminAddress },
    });
    return net.send(from, changeCollectionAdminMessage);
}

/// Send message to the contract with a custom body cell
fun NftCollectionContract.sendAny(
    self,
    from: address,
    body: cell,
    config: SendParams = {},
): SendResultList {
    val outgoingMessage = createMessage({
        bounce: config.bounce,
        value: config.value,
        dest: self.address,
        body,
    });
    return net.send(from, outgoingMessage);
}

fun NftCollectionContract.getCollectionData(self): CollectionDataReply {
    return net.runGetMethod(self.address, "get_collection_data");
}

fun NftCollectionContract.getNftAddressByIndex(self, itemIndex: int): address {
    return net.runGetMethod(self.address, "get_nft_address_by_index", itemIndex);
}

fun NftCollectionContract.royaltyParams(self): RoyaltyParams {
    return net.runGetMethod(self.address, "royalty_params");
}

fun NftCollectionContract.getNftContent(
    self,
    itemIndex: int,
    individualNftContent: string,
): Cell<OffchainMetadataReply> {
    return net.runGetMethod(
        self.address,
        "get_nft_content",
        [itemIndex, individualNftContent] as tuple,
    );
}
