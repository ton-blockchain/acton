import "@stdlib/strings"
import "types"

// Warning - this is an expensive function. It is used only in get-methods
fun string.concat(self, other: string): string {
    var b: StringBuilder = StringBuilder.create();
    b.append(self);
    b.append(other);
    return b.build();
}

struct NftItemInitAtDeployment {
    ownerAddress: address
    content: string
}

struct (0x693d3950) RequestRoyaltyParams {
    queryId: uint64
}

struct (0xa8cb00ad) ResponseRoyaltyParams {
    queryId: uint64
    royaltyParams: RoyaltyParams
}

struct (0x00000001) DeployNft {
    queryId: uint64
    itemIndex: uint64
    attachTonAmount: coins
    initParams: Cell<NftItemInitAtDeployment>
}

struct (0x00000002) BatchDeployNfts {
    queryId: uint64
    deployList: map<uint64, BatchDeployDictItem>
}

struct BatchDeployDictItem {
    attachTonAmount: coins
    initParams: Cell<NftItemInitAtDeployment>
}

struct (0x00000003) ChangeCollectionAdmin {
    queryId: uint64
    newAdminAddress: address
}

struct (0x2fcb26a2) RequestStaticData {
    queryId: uint64
}

struct (0x8b771735) ResponseStaticData {
    queryId: uint64
    itemIndex: uint256
    collectionAddress: address
}

struct (0x05138d91) NotificationForNewOwner {
    queryId: uint64
    oldOwnerAddress: address
    payload: Payload
}

struct (0xd53276db) ReturnExcessesBack {
    queryId: uint64
}

struct (0b0) PayloadInline {
    value: RemainingBitsAndRefs
}

struct (0b1) PayloadInRef {
    value: Cell<RemainingBitsAndRefs>
}

type Payload = PayloadInline | PayloadInRef

struct (0x5fcc3d14) AskToChangeOwnership {
    queryId: uint64
    newOwnerAddress: address
    sendExcessesTo: address?
    customPayload: dict
    forwardTonAmount: coins
    forwardPayload: Payload
}
