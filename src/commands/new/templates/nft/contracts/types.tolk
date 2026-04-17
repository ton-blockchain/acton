struct RoyaltyParams {
    numerator: uint16
    denominator: uint16
    royaltyAddress: address
}

struct NftCollectionStorage {
    adminAddress: address
    nextItemIndex: uint64
    content: Cell<CollectionContent>
    nftItemCode: cell
    royaltyParams: Cell<RoyaltyParams>
}

struct CollectionContent {
    collectionMetadata: cell
    commonContent: string
}

struct NftItemStorage {
    itemIndex: uint64
    collectionAddress: address
    ownerAddress: address
    content: string
}

struct NftItemStorageNotInitialized {
    itemIndex: uint64
    collectionAddress: address
}

struct NftDataReply {
    isInitialized: bool
    itemIndex: int
    collectionAddress: address
    ownerAddress: address? = null
    content: string? = null
}

fun NftCollectionStorage.load(): NftCollectionStorage {
    return NftCollectionStorage.fromCell(contract.getData());
}

fun NftCollectionStorage.save(self) {
    contract.setData(self.toCell());
}

fun NftItemStorage.startLoading(): NftItemStorageLoader {
    return NftItemStorageLoader.fromCell(contract.getData());
}

fun NftItemStorage.save(self) {
    contract.setData(self.toCell());
}

// storage of an NFT item is tricky: it changes its shape after deployment;
// initially, it has only itemIndex and collectionAddress: `NftItemStorageNotInitialized`;
// after initialization, it becomes a fully filled `NftItemStorage`;
// that's why NftItemStorage doesn't have a `load` method: only `startLoading`,
// which helps detect presence of the rest fields, and `endLoading` called if initialized
struct NftItemStorageLoader {
    itemIndex: uint64
    collectionAddress: address
    private rest: RemainingBitsAndRefs
}

fun NftItemStorageLoader.isNotInitialized(self): bool {
    return self.rest.isEmpty();
}

fun NftItemStorageLoader.endLoading(mutate self): NftItemStorage {
    return {
        itemIndex: self.itemIndex,
        collectionAddress: self.collectionAddress,
        ownerAddress: self.rest.loadAny(),
        content: self.rest.loadAny(),
    };
}

fun calcDeployedNftItem(
    itemIndex: uint64,
    collectionAddress: address,
    nftItemCode: cell,
): AutoDeployAddress {
    val emptyNftItemStorage: NftItemStorageNotInitialized = { itemIndex, collectionAddress };

    return { stateInit: { code: nftItemCode, data: emptyNftItemStorage.toCell() } };
}

struct CollectionDataReply {
    nextItemIndex: int
    collectionMetadata: cell
    adminAddress: address
}

struct (0x01) OffchainMetadataReply {
    string: string
}
